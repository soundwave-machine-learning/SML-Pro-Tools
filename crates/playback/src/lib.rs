//! Realtime playback through the system audio device (cpal: CoreAudio, WASAPI, ALSA/Pulse/
//! PipeWire via ALSA, JACK, WebAudio).
//!
//! The UI thread owns a [`Player`]; the audio callback owns the [`MixEngine`] and a snapshot of
//! the session (`Arc<Session>`) that the UI swaps whenever the document changes. Commands go to
//! the callback through a channel; position and meters come back through atomics and a
//! `try_lock`ed snapshot, so the callback never blocks. When no device is available the player
//! falls back to a silent clock thread so the transport, meters and automation still run.
//!
//! Surround: the device is opened with as many output channels as it offers, up to the width of
//! the session's main format. Main channels (SMPTE/WAV order) go to device channels in order; when
//! the device has fewer channels than the main mix, the mix is folded down to stereo (ITU-R
//! BS.775) on the first two device channels.
//!
//! Plugins and threads: hosted third-party plugins (CLAP/VST3) are created, given their stored
//! state and prepared on the thread that calls [`Player::update_session`] (the UI thread), and
//! travel to the audio thread together with the session snapshot that needs them; the mix
//! engine adopts them (see `soundcraft_mix`'s crate docs). Instances the engine retires come back
//! on a return channel and are dropped on the UI thread ([`Player::idle`]). Plugin states for
//! saving are read on request ([`Player::capture_states`]: a command with a reply; the audio
//! thread answers between two blocks, never blocking). Plugin editors are driven on the UI thread
//! through handles taken from each instance before it leaves ([`Player::open_editor`]).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod capture;
pub mod clock;
pub mod device;
pub mod ptt;
pub mod record;

use soundcraft_dsp::{Plugin, PluginEditor};
use soundcraft_mix::{GraphPrep, MAX_CHANNELS, MixEngine, PreparedInstance, StripMeter, itu_stereo_fold, main_channels};
use soundcraft_model::{Session, TrackId};
use soundcraft_time::{Range, Samples};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};

const BLOCK: usize = 512;

/// The realtime audio thread marker lives in the mix engine, which carries it into the strips it
/// renders on worker threads; the cpal callbacks here set it.
pub use soundcraft_mix::{mark_audio_thread, on_audio_thread};

/// Meter values for the UI.
#[derive(Debug, Clone, Default)]
pub struct MeterSnapshot {
    pub tracks: HashMap<TrackId, StripMeter>,
    pub main: StripMeter,
}

/// Plugin states read from the live instances: `(track, slot or INSTRUMENT_SLOT, state)`.
pub type PluginStates = Vec<(TrackId, usize, Vec<u8>)>;

/// A parameter edit made in a plugin's own editor: `(track, slot, param id, value)`.
pub type EditorEdit = (TrackId, usize, String, f32);

enum Cmd {
    /// A new snapshot plus its prepared graph (B01: strips, buffers, order and the third-party
    /// instances it needs, all created on the UI thread). A fresh engine comes along when the
    /// sample rate changed (the old one is dropped on the UI thread).
    Session(Arc<Session>, Box<GraphPrep>, Option<Box<MixEngine>>),
    /// Read every plugin state and reply.
    CaptureStates(Sender<PluginStates>),
    /// Count Off: click before this session position even with the click off.
    ClickUntil(Samples),
    Input(Arc<record::InputRing>),
    Recording(bool),
    Play {
        from: Samples,
        end: Option<Samples>,
        looped: Option<Range>,
    },
    Stop,
    Locate(Samples),
}

struct Shared {
    position: AtomicI64,
    playing: AtomicBool,
    /// Speed × 1000.
    speed: AtomicU32,
    meters: Mutex<MeterSnapshot>,
    /// Stream clock telemetry (written by the callback with atomics only).
    stream_generation: std::sync::atomic::AtomicU64,
    stream_frames: std::sync::atomic::AtomicU64,
    last_callback_frames: AtomicU32,
    /// Set by the audio thread when compensation latency changed during recording.
    latency_alarm: AtomicBool,
    /// Output stream failure: 0 none, 1 device not available, 2 other backend error (set from
    /// the stream's error callback; first one wins).
    output_lost: std::sync::atomic::AtomicU8,
    /// Latest (presentation instant, session sample presented then) while playing (CL04).
    play_anchor: Mutex<Option<(cpal::StreamInstant, Samples)>>,
    /// Monitor-only controls (R37): applied to the device output after fold-down, never to a
    /// render or bounce. Volume in milli-dB (0 = unity), dim (−20 dB) and mono sum.
    monitor_db_milli: std::sync::atomic::AtomicI32,
    monitor_dim: AtomicBool,
    monitor_mono: AtomicBool,
    /// Monitor-only auxiliary audio (assistant speech, mastering reference), see [`AuxMonitor`].
    aux: AuxMonitor,
}

/// Audio only the listener hears (assistant speech, the mastering reference): a single-producer
/// ring filled from the UI thread at the device rate and summed into the device output after
/// the program and before the monitor stage. It is not part of the rendered block, so no
/// bounce (`render_range`, a separate code path) and no capture (the device INPUT ring) can
/// contain it. Speech is dropped while recording when `mute_while_recording` is set.
pub struct AuxMonitor {
    buf: Vec<std::sync::atomic::AtomicU32>,
    head: std::sync::atomic::AtomicUsize,
    tail: std::sync::atomic::AtomicUsize,
    /// Linear gain × 1000 on the way out.
    gain_milli: AtomicU32,
    /// Mute the program on the device output while this plays (reference A/B); never a render.
    pub solo: AtomicBool,
    /// Drop instead of play while recording (assistant speech policy).
    pub mute_while_recording: AtomicBool,
}

/// Capacity of the auxiliary monitor ring (stereo frames at the device rate).
pub const AUX_RING_FRAMES: usize = 1 << 17;

impl AuxMonitor {
    fn new() -> AuxMonitor {
        AuxMonitor {
            buf: (0..AUX_RING_FRAMES * 2).map(|_| std::sync::atomic::AtomicU32::new(0)).collect(),
            head: std::sync::atomic::AtomicUsize::new(0),
            tail: std::sync::atomic::AtomicUsize::new(0),
            gain_milli: AtomicU32::new(1000),
            solo: AtomicBool::new(false),
            mute_while_recording: AtomicBool::new(true),
        }
    }

    /// UI thread: queues stereo interleaved frames (mono callers duplicate); returns how many
    /// frames were accepted (the rest wait for room).
    pub fn push(&self, interleaved: &[f32]) -> usize {
        let frames = interleaved.len() / 2;
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        let free = AUX_RING_FRAMES - 1 - head.wrapping_sub(tail) % AUX_RING_FRAMES;
        let n = frames.min(free);
        for i in 0..n {
            let slot = (head + i) % AUX_RING_FRAMES;
            self.buf[slot * 2].store(interleaved[i * 2].to_bits(), Ordering::Relaxed);
            self.buf[slot * 2 + 1].store(interleaved[i * 2 + 1].to_bits(), Ordering::Relaxed);
        }
        self.head.store((head + n) % AUX_RING_FRAMES, Ordering::Release);
        n
    }

    /// UI thread: queues mono speech at `rate` for the device running at `device_rate`
    /// (windowed-sinc conversion, both channels); returns the frames accepted. Assistant speech
    /// takes this path only, so it is heard and never rendered, bounced or captured.
    pub fn push_speech(&self, mono: &[f32], rate: u32, device_rate: u32) -> usize {
        let converted = soundcraft_dsp::offline::resample(&[mono.to_vec()], rate, device_rate);
        let ch = converted.into_iter().next().unwrap_or_default();
        let interleaved: Vec<f32> = ch.iter().flat_map(|&x| [x, x]).collect();
        self.push(&interleaved)
    }

    pub fn set_gain_db(&self, db: f32) {
        let g = 10f32.powf(db.clamp(-60.0, 12.0) / 20.0);
        self.gain_milli.store((g * 1000.0) as u32, Ordering::Relaxed);
    }

    pub fn queued_frames(&self) -> usize {
        self.head.load(Ordering::Acquire).wrapping_sub(self.tail.load(Ordering::Acquire)) % AUX_RING_FRAMES
    }

    pub fn clear(&self) {
        self.tail.store(self.head.load(Ordering::Acquire), Ordering::Release);
    }

    /// Audio thread: sums (or drops) one frame into `frame`'s first two channels.
    fn pop_into(&self, frame: &mut [f32], recording: bool) {
        let tail = self.tail.load(Ordering::Acquire);
        if tail == self.head.load(Ordering::Acquire) {
            return;
        }
        let l = f32::from_bits(self.buf[tail * 2].load(Ordering::Relaxed));
        let r = f32::from_bits(self.buf[tail * 2 + 1].load(Ordering::Relaxed));
        self.tail.store((tail + 1) % AUX_RING_FRAMES, Ordering::Release);
        if recording && self.mute_while_recording.load(Ordering::Relaxed) {
            return;
        }
        let g = self.gain_milli.load(Ordering::Relaxed) as f32 / 1000.0;
        if let Some(x) = frame.get_mut(0) {
            *x += l * g;
        }
        if let Some(x) = frame.get_mut(1) {
            *x += r * g;
        }
    }
}

impl Shared {
    fn new() -> Shared {
        Shared {
            position: AtomicI64::new(0),
            playing: AtomicBool::new(false),
            speed: AtomicU32::new(1000),
            meters: Mutex::new(MeterSnapshot::default()),
            stream_generation: std::sync::atomic::AtomicU64::new(0),
            stream_frames: std::sync::atomic::AtomicU64::new(0),
            last_callback_frames: AtomicU32::new(0),
            latency_alarm: AtomicBool::new(false),
            output_lost: std::sync::atomic::AtomicU8::new(0),
            monitor_db_milli: std::sync::atomic::AtomicI32::new(0),
            monitor_dim: AtomicBool::new(false),
            monitor_mono: AtomicBool::new(false),
            aux: AuxMonitor::new(),
            play_anchor: Mutex::new(None),
        }
    }
}

/// State owned by the audio callback.
struct AudioState {
    rx: Receiver<Cmd>,
    shared: Arc<Shared>,
    session: Arc<Session>,
    mix: MixEngine,
    pos: Samples,
    playing: bool,
    end: Option<Samples>,
    looped: Option<Range>,
    block: Vec<Vec<f32>>,
    /// Rendered frames waiting to be consumed (planar, main channels) and the fractional read index.
    pending: Vec<Vec<f32>>,
    /// Stereo fold-down gains per main channel (used when the device has fewer channels).
    fold: [(f32, f32); MAX_CHANNELS],
    pending_len: usize,
    read: f64,
    device_rate: f64,
    input: Option<Arc<record::InputRing>>,
    recording: bool,
    /// Retired plugin instances and replaced sessions go back to the UI thread to be dropped
    /// there (CL05: freeing a whole session or plugin is not audio-thread work).
    garbage: Sender<Garbage>,
    /// Device-frame clock of this stream (CL01).
    clock: clock::StreamClock,
    /// A Count Off arrived for the next Play.
    countoff_armed: bool,
}

/// What the audio thread hands back to the UI thread to drop.
pub(crate) struct Garbage {
    plugins: Vec<Box<dyn Plugin>>,
    /// Held only so it is dropped by the receiver.
    _session: Option<Arc<Session>>,
    /// Leftovers of an applied graph preparation (old maps, unused strips, retired instances).
    graph: Option<Box<GraphPrep>>,
    /// A replaced engine (sample-rate change), dropped here.
    engine: Option<Box<MixEngine>>,
}

impl Garbage {
    fn plugins(plugins: Vec<Box<dyn Plugin>>) -> Garbage {
        Garbage { plugins, _session: None, graph: None, engine: None }
    }
}

/// Stream failure code kept by the error callbacks: 1 device gone, 2 other backend error.
pub(crate) fn stream_error_code(e: &cpal::StreamError) -> u8 {
    if matches!(e, cpal::StreamError::DeviceNotAvailable) { 1 } else { 2 }
}

fn lost_reason(code: u8) -> Option<&'static str> {
    match code {
        0 => None,
        1 => Some("the output device is no longer available"),
        _ => Some("the output driver reported an error"),
    }
}

fn new_engine(sr: f32) -> MixEngine {
    let mut m = MixEngine::new(sr, BLOCK);
    m.set_external_instances(true);
    // Every session hand-off below announces itself (`Cmd::Session`).
    m.set_session_changes_signalled(true);
    m.realtime_overrides = true;
    m.metronome = true;
    m
}

impl AudioState {
    fn new(rx: Receiver<Cmd>, shared: Arc<Shared>, session: Arc<Session>, device_rate: f64, garbage: Sender<Garbage>) -> Self {
        let sr = session.sample_rate.as_f64() as f32;
        let nch = main_channels(&session);
        AudioState {
            garbage,
            fold: fold_gains(&session),
            rx,
            shared,
            mix: new_engine(sr),
            session,
            pos: 0,
            playing: false,
            end: None,
            looped: None,
            block: vec![vec![0.0; BLOCK]; nch],
            pending: vec![vec![0.0; BLOCK]; nch],
            pending_len: 0,
            read: 0.0,
            device_rate,
            input: None,
            recording: false,
            clock: clock::StreamClock::new(device_rate as u32),
            countoff_armed: false,
        }
    }

    fn drain(&mut self) {
        let mut changed = false;
        while let Ok(c) = self.rx.try_recv() {
            match c {
                Cmd::Input(r) => self.input = Some(r),
                Cmd::Recording(r) => {
                    self.recording = r;
                    // A crashed sandboxed processor is never silently resumed into a take.
                    #[cfg(not(target_arch = "wasm32"))]
                    soundcraft_mix::sandbox::set_auto_restart(!r);
                }
                Cmd::ClickUntil(at) => {
                    self.mix.click_until = at;
                    self.countoff_armed = true;
                }
                Cmd::CaptureStates(reply) => {
                    self.mix.ensure_synced(&self.session);
                    let _ = reply.send(self.mix.capture_states());
                }
                Cmd::Session(s, prep, fresh) => {
                    changed = true;
                    let mut old_engine = None;
                    if let Some(fresh) = fresh {
                        // The rate changed: the UI thread prepared a whole new engine.
                        old_engine = Some(Box::new(std::mem::replace(&mut self.mix, *fresh)));
                    }
                    // Structure, buffers and instances were prepared off this thread (B01).
                    let leftovers = self.mix.apply_prepared(&s, *prep);
                    let _ = self.garbage.send(Garbage { plugins: Vec::new(), _session: None, graph: Some(Box::new(leftovers)), engine: old_engine });
                    // The main format changed (a document edit, not steady state): resize.
                    let nch = main_channels(&s);
                    if self.block.len() != nch {
                        self.block = vec![vec![0.0; BLOCK]; nch];
                        self.pending = vec![vec![0.0; BLOCK]; nch];
                        self.pending_len = 0;
                        self.read = 0.0;
                    }
                    self.fold = fold_gains(&s);
                    // Temporary audition overrides are realtime-only (offline exports ignore them).
                    self.mix.audition = s.audition;
                    self.mix.listen = s.listen.mask;
                    let old = std::mem::replace(&mut self.session, s);
                    let _ = self.garbage.send(Garbage { plugins: Vec::new(), _session: Some(old), graph: None, engine: None });
                }
                Cmd::Play { from, end, looped } => {
                    // A count-off applies to the one Play that follows it, never to a later one.
                    if !std::mem::take(&mut self.countoff_armed) {
                        self.mix.click_until = Samples::MIN;
                    }
                    self.pos = from;
                    self.end = end;
                    self.looped = looped.filter(|r| r.len() > 64);
                    self.playing = true;
                    self.pending_len = 0;
                    self.read = 0.0;
                    self.mix.reset();
                }
                Cmd::Stop => {
                    self.mix.click_until = Samples::MIN;
                    self.playing = false;
                    self.mix.reset();
                }
                Cmd::Locate(at) => {
                    self.pos = at;
                    self.pending_len = 0;
                    self.read = 0.0;
                }
            }
        }
        if changed {
            // Place adopted instances now (not only once playback starts), so state captures and
            // editors see them; `sync` creates no third-party plugins here.
            self.mix.ensure_synced(&self.session);
        }
        self.send_garbage();
        self.shared.playing.store(self.playing, Ordering::Relaxed);
    }

    fn send_garbage(&mut self) {
        let g = self.mix.take_retired();
        if !g.is_empty() {
            let _ = self.garbage.send(Garbage::plugins(g));
        }
    }

    /// Render the next block (at most `want` ≤ BLOCK frames) at the session rate into `pending`.
    fn render_next(&mut self, want: usize) {
        let mut n = want.clamp(1, BLOCK);
        if let Some(l) = self.looped
            && self.pos >= l.end
        {
            self.pos = l.start;
        }
        if let Some(l) = self.looped {
            n = n.min(usize::try_from((l.end - self.pos).max(1)).unwrap_or(BLOCK));
        } else if let Some(e) = self.end {
            if self.pos >= e {
                self.playing = false;
                self.shared.playing.store(false, Ordering::Relaxed);
                n = 0;
            } else {
                n = n.min(usize::try_from(e - self.pos).unwrap_or(BLOCK));
            }
        }
        if n == 0 {
            for c in &mut self.pending {
                c.iter_mut().for_each(|x| *x = 0.0);
            }
            self.pending_len = BLOCK;
            return;
        }
        if let Some(ring) = &self.input {
            let ch = ring.channels.load(Ordering::Relaxed).max(1);
            if self.mix.input.len() != ch {
                self.mix.input = vec![vec![0.0; BLOCK]; ch];
            }
            ring.pop_into(&mut self.mix.input, n, self.session.sample_rate.hz());
        }
        self.mix.monitor_only = !self.playing;
        self.mix.recording = self.recording;
        self.mix.render(&self.session, self.pos, n, &mut self.block);
        if self.mix.take_latency_event() {
            // CL04: a latency change while recording is a capture-integrity event.
            self.shared.latency_alarm.store(true, Ordering::Release);
        }
        self.send_garbage();
        for (d, s) in self.pending.iter_mut().zip(self.block.iter()) {
            if let (Some(dd), Some(ss)) = (d.get_mut(..n), s.get(..n)) {
                dd.copy_from_slice(ss);
            }
        }
        self.pending_len = n;
        if !self.playing {
            return;
        }
        self.pos += n as i64;
        self.shared.position.store(self.pos, Ordering::Relaxed);
        if let Ok(mut m) = self.shared.meters.try_lock() {
            m.main.copy_from(&self.mix.main_meter);
            for (k, v) in &self.mix.meters {
                match m.tracks.get_mut(k) {
                    Some(d) => d.copy_from(v),
                    None => {
                        m.tracks.insert(*k, v.clone());
                    }
                }
            }
        }
    }

    /// [`AudioState::fill`] with the backend's presentation instant for the buffer: records the
    /// (instant, session position) anchor used to place recordings by timestamp. `try_lock` only.
    fn fill_stamped(&mut self, out: &mut [f32], channels: usize, playback: cpal::StreamInstant) {
        self.drain();
        if self.playing {
            let queued = (self.pending_len as f64 - self.read).max(0.0) as i64;
            if let Ok(mut a) = self.shared.play_anchor.try_lock() {
                *a = Some((playback, self.pos - queued));
            }
        }
        self.fill(out, channels);
    }

    /// Fill an interleaved device buffer.
    fn fill(&mut self, out: &mut [f32], channels: usize) {
        self.drain();
        let channels = channels.max(1);
        let total = out.len() / channels;
        let stamp = self.clock.advance(total);
        self.shared.stream_generation.store(stamp.generation, Ordering::Relaxed);
        self.shared.stream_frames.store(self.clock.frames(), Ordering::Relaxed);
        self.shared.last_callback_frames.store(stamp.frames, Ordering::Relaxed);
        let monitoring = self.input.is_some() && self.session.tracks.iter().any(|t| t.mixer.input_monitor || t.mixer.record_arm);
        if !self.playing && !monitoring {
            out.iter_mut().for_each(|x| *x = 0.0);
            if let Ok(mut m) = self.shared.meters.try_lock() {
                m.main = StripMeter::default();
                m.tracks.clear();
            }
            return;
        }
        let speed = f64::from(self.shared.speed.load(Ordering::Relaxed)) / 1000.0;
        let step = self.session.sample_rate.as_f64() / self.device_rate.max(1.0) * speed.clamp(0.1, 4.0);
        let mon_gain = monitor_gain(self.shared.monitor_db_milli.load(Ordering::Relaxed), self.shared.monitor_dim.load(Ordering::Relaxed));
        let mon_mono = self.shared.monitor_mono.load(Ordering::Relaxed);
        // Same rate, normal speed: render exactly what this callback still needs, so no audio is
        // queued past the callback (CL02); otherwise fixed blocks feed the interpolating reader.
        let exact = step == 1.0;
        for (f, frame) in out.chunks_mut(channels).enumerate() {
            while self.read >= self.pending_len as f64 {
                self.read -= self.pending_len as f64;
                if !self.playing && !monitoring {
                    break;
                }
                self.render_next(if exact { total - f } else { BLOCK });
                if self.pending_len == 0 {
                    break;
                }
            }
            let i0 = self.read.floor() as usize;
            let fr = (self.read - i0 as f64) as f32;
            let sounding = self.playing || monitoring;
            let nch = self.pending.len();
            let sample = |c: usize| {
                let ch = self.pending.get(c);
                let a = ch.and_then(|v| v.get(i0)).copied().unwrap_or(0.0);
                let b = ch.and_then(|v| v.get(i0 + 1)).copied().unwrap_or(a);
                a + (b - a) * fr
            };
            if nch > 2 && channels < nch {
                // Fewer device channels than the main mix: ITU fold-down to stereo.
                let (mut l, mut r) = (0.0f32, 0.0f32);
                if sounding {
                    for (c, (gl, gr)) in self.fold.iter().enumerate().take(nch) {
                        let v = sample(c);
                        l += v * gl;
                        r += v * gr;
                    }
                }
                for (c, o) in frame.iter_mut().enumerate() {
                    *o = match c {
                        0 => l.clamp(-1.0, 1.0),
                        1 => r.clamp(-1.0, 1.0),
                        _ => 0.0,
                    };
                }
            } else {
                for (c, o) in frame.iter_mut().enumerate() {
                    // Stereo mixes on a mono device play the left channel (as before).
                    *o = if c < nch.max(1) && sounding { sample(c).clamp(-1.0, 1.0) } else { 0.0 };
                }
            }
            // Monitor-only auxiliary audio (speech, reference): after the program, before the
            // monitor stage; never in a render or a capture.
            if self.shared.aux.solo.load(Ordering::Relaxed) {
                frame.iter_mut().for_each(|x| *x = 0.0);
            }
            self.shared.aux.pop_into(frame, self.recording);
            apply_monitor(frame, mon_gain, mon_mono);
            self.read += step;
        }
    }
}

/// Monitor-only stage (R37): a gain (volume × dim) and an optional mono sum of the first two
/// channels, on the device output only. Pure, so it is unit-tested and provably never part of
/// an offline render (which does not go through `fill`).
pub fn apply_monitor(frame: &mut [f32], gain: f32, mono: bool) {
    if mono && frame.len() >= 2 {
        let m = (frame[0] + frame[1]) * 0.5;
        frame[0] = m;
        frame[1] = m;
    }
    if (gain - 1.0).abs() > 1e-9 {
        for x in frame.iter_mut() {
            *x = (*x * gain).clamp(-1.0, 1.0);
        }
    }
}

/// Linear monitor gain from the shared controls.
fn monitor_gain(db_milli: i32, dim: bool) -> f32 {
    let db = (db_milli as f32 / 1000.0).clamp(-90.0, 0.0) + if dim { -20.0 } else { 0.0 };
    if db <= -90.0 { 0.0 } else { 10f32.powf(db / 20.0) }
}

/// UI-thread bookkeeping of the third-party instances the audio thread holds.
struct Side {
    /// What was last created for each slot: (plugin id, channels, created ok).
    shadow: HashMap<(TrackId, usize), (String, usize, bool)>,
    /// Session sample rate the instances were prepared for.
    rate: f32,
    /// Editor handles of the live instances.
    editors: HashMap<(TrackId, usize), Box<dyn PluginEditor>>,
    garbage: Receiver<Garbage>,
    /// Track count of the last session sent (bounds the retire capacity of the next preparation).
    prev_tracks: usize,
}

impl Side {
    /// Drops retired instances (closing their editors first, inside the plugin's drop).
    fn collect_garbage(&mut self) {
        while let Ok(mut g) = self.garbage.try_recv() {
            // Instances first: a plugin's drop may still close an editor.
            drop(g.plugins);
            if let Some(mut p) = g.graph.take() {
                drop(p.drain_plugins());
                drop(p);
            }
            if let Some(mut e) = g.engine.take() {
                drop(e.drain_plugins());
                drop(e);
            }
            drop(g._session);
        }
    }

    fn forget(&mut self, key: (TrackId, usize)) {
        self.shadow.remove(&key);
        if let Some(mut e) = self.editors.remove(&key) {
            e.close();
        }
    }
}

/// The transport and audio device.
pub struct Player {
    shared: Arc<Shared>,
    tx: Sender<Cmd>,
    side: Mutex<Side>,
    #[cfg(not(target_arch = "wasm32"))]
    _stream: Option<cpal::Stream>,
    #[cfg(target_arch = "wasm32")]
    _stream: Option<cpal::Stream>,
    pub device_name: String,
    pub device_rate: u32,
    /// Output channels the device was opened with (0 for the silent clock).
    pub device_channels: u16,
    /// True when no audio device could be opened (silent clock).
    pub silent: bool,
    /// The negotiated configuration when a specific device/rate/buffer was requested.
    pub negotiated: Option<device::NegotiatedConfig>,
}

impl Player {
    /// Open the default output device. Never fails: without a device it runs a silent clock.
    pub fn new(session: Arc<Session>) -> Player {
        let p = Player::open(Arc::clone(&session));
        // Hand the initial session's third-party plugins over.
        p.update_session(session);
        p
    }

    fn side(&self) -> std::sync::MutexGuard<'_, Side> {
        self.side.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Opens exactly the requested output host/device/rate/buffer (CL02). A request the device
    /// cannot honour is an error — never a silent substitution — and the caller keeps its
    /// current player. Change devices only while stopped.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_requested(session: Arc<Session>, req: &device::DeviceRequest) -> Result<Player, String> {
        let caps = device::enumerate(device::Direction::Output);
        let neg = device::negotiate(req, &caps).map_err(|e| e.to_string())?;
        let dev = device::find_device(&neg, device::Direction::Output)?;
        let cfg = device::stream_config(&neg);
        let mut p = Player::open_from(Arc::clone(&session), Ok((dev, cfg, neg.device.clone())));
        if p.silent {
            return Err(format!("`{}` could not start a stream with the negotiated settings", neg.device));
        }
        p.negotiated = Some(neg);
        p.update_session(session);
        Ok(p)
    }

    fn open(session: Arc<Session>) -> Player {
        let want = session.sample_rate.hz();
        let want_ch = u16::try_from(main_channels(&session)).unwrap_or(2);
        Player::open_from(session, open_device(want, want_ch))
    }

    fn open_from(session: Arc<Session>, dev: Result<(cpal::Device, cpal::StreamConfig, String), String>) -> Player {
        let (gtx, grx) = channel();
        let side = Mutex::new(Side {
            shadow: HashMap::new(),
            rate: session.sample_rate.as_f64() as f32,
            editors: HashMap::new(),
            garbage: grx,
            prev_tracks: 0,
        });
        let shared = Arc::new(Shared::new());
        let (tx, rx) = channel();
        let want = session.sample_rate.hz();
        match dev {
            Ok((device, config, name)) => {
                let rate = config.sample_rate.0;
                let channels = usize::from(config.channels);
                let mut state = AudioState::new(rx, Arc::clone(&shared), session, f64::from(rate), gtx);
                // cpal may call this on the audio thread (on ALSA in a loop while the device
                // stays broken): marked, so the logger hands the record off instead of writing it.
                let lost = Arc::clone(&shared);
                let err_fn = move |e: cpal::StreamError| {
                    mark_audio_thread();
                    let _ = lost.output_lost.compare_exchange(0, stream_error_code(&e), Ordering::AcqRel, Ordering::Acquire);
                    log::warn!("audio stream error: {e}");
                };
                use cpal::traits::{DeviceTrait, StreamTrait};
                match device.build_output_stream(
                    &config,
                    move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                        mark_audio_thread();
                        state.fill_stamped(data, channels, info.timestamp().playback);
                    },
                    err_fn,
                    None,
                ) {
                    Ok(stream) => {
                        if let Err(e) = stream.play() {
                            log::warn!("audio stream failed to start: {e}");
                        }
                        log::info!("audio device: {name} @ {rate} Hz, {channels} ch");
                        Player {
                            shared,
                            tx,
                            side,
                            _stream: Some(stream),
                            device_name: name,
                            device_rate: rate,
                            device_channels: config.channels,
                            silent: false,
                            negotiated: None,
                        }
                    }
                    Err(e) => {
                        log::warn!("cannot open audio stream: {e}; using a silent clock");
                        let (tx2, rx2) = channel();
                        let (gtx2, grx2) = channel();
                        // The state (and its receivers) moved into the failed closure; rebuild.
                        let mut side = side;
                        side.get_mut().unwrap_or_else(PoisonError::into_inner).garbage = grx2;
                        Player::silent_clock(shared, tx2, rx2, want, side, gtx2)
                    }
                }
            }
            Err(e) => {
                log::warn!("no audio output: {e}; using a silent clock");
                let s2 = Arc::clone(&shared);
                Player::silent_clock_with(s2, tx, rx, session, side, gtx)
            }
        }
    }

    fn silent_clock(shared: Arc<Shared>, tx: Sender<Cmd>, rx: Receiver<Cmd>, rate: u32, side: Mutex<Side>, gtx: Sender<Garbage>) -> Player {
        let s = Arc::new(Session::new("Untitled", soundcraft_time::SampleRate::new(rate).unwrap_or_default()));
        Player::silent_clock_with(shared, tx, rx, s, side, gtx)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn silent_clock_with(
        shared: Arc<Shared>,
        tx: Sender<Cmd>,
        rx: Receiver<Cmd>,
        session: Arc<Session>,
        side: Mutex<Side>,
        gtx: Sender<Garbage>,
    ) -> Player {
        let rate = session.sample_rate.hz();
        let mut state = AudioState::new(rx, Arc::clone(&shared), session, f64::from(rate), gtx);
        let spawn = std::thread::Builder::new().name("soundcraft-clock".into()).spawn(move || {
            let mut buf = vec![0.0f32; 4096];
            let mut last = std::time::Instant::now();
            loop {
                std::thread::sleep(std::time::Duration::from_millis(10));
                let now = std::time::Instant::now();
                let frames = ((now - last).as_secs_f64() * state.device_rate) as usize;
                last = now;
                let n = (frames * 2).min(buf.len());
                if let Some(b) = buf.get_mut(..n) {
                    state.fill(b, 2);
                }
                if Arc::strong_count(&state.shared) <= 1 {
                    break;
                }
            }
        });
        if let Err(e) = spawn {
            log::warn!("clock thread failed: {e}");
        }
        Player {
            shared,
            tx,
            side,
            _stream: None,
            device_name: "No audio device (silent)".into(),
            device_rate: rate,
            device_channels: 0,
            silent: true,
            negotiated: None,
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn silent_clock_with(
        shared: Arc<Shared>,
        tx: Sender<Cmd>,
        _rx: Receiver<Cmd>,
        session: Arc<Session>,
        side: Mutex<Side>,
        _gtx: Sender<Garbage>,
    ) -> Player {
        Player {
            shared,
            tx,
            side,
            _stream: None,
            device_name: "No audio device".into(),
            device_rate: session.sample_rate.hz(),
            device_channels: 0,
            silent: true,
            negotiated: None,
        }
    }

    /// Sends a new session snapshot to the audio thread, with freshly created (state-restored,
    /// prepared) instances of any third-party plugin it newly needs. Call on the UI thread.
    pub fn update_session(&self, s: Arc<Session>) {
        let mut side = self.side();
        let sr = s.sample_rate.as_f64() as f32;
        let rate_changed = (sr - side.rate).abs() > 0.5;
        let instances = side.prepare(&s);
        let prep = GraphPrep::build(&s, sr, BLOCK, instances, side.prev_tracks);
        side.prev_tracks = s.tracks.len();
        let fresh = rate_changed.then(|| Box::new(new_engine(sr)));
        drop(side);
        let _ = self.tx.send(Cmd::Session(s, Box::new(prep), fresh));
    }

    /// Reads the state of every live third-party plugin (for saving the session). Waits briefly
    /// for the audio thread to answer between two blocks; empty when it does not answer.
    pub fn capture_states(&self) -> PluginStates {
        self.side().collect_garbage();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (tx, rx) = channel();
            if self.tx.send(Cmd::CaptureStates(tx)).is_err() {
                return Vec::new();
            }
            rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap_or_default()
        }
        #[cfg(target_arch = "wasm32")]
        {
            Vec::new()
        }
    }

    /// Per-UI-frame housekeeping: drops retired instances and services plugin editors. Returns
    /// the parameter edits made in open editors since the last call.
    pub fn idle(&self) -> Vec<EditorEdit> {
        let mut side = self.side();
        side.collect_garbage();
        let mut out = Vec::new();
        for (&(t, slot), e) in side.editors.iter_mut() {
            for (id, v) in e.idle() {
                out.push((t, slot, id, v));
            }
        }
        out
    }

    /// True when the plugin in this slot has its own editor.
    pub fn has_editor(&self, track: TrackId, slot: usize) -> bool {
        self.side().editors.contains_key(&(track, slot))
    }

    /// Opens the plugin's own editor window. Main thread only.
    pub fn open_editor(&self, track: TrackId, slot: usize) -> Result<(), String> {
        match self.side().editors.get_mut(&(track, slot)) {
            Some(e) => e.open(),
            None => Err("this plugin has no editor (or it is not loaded)".into()),
        }
    }

    pub fn close_editor(&self, track: TrackId, slot: usize) {
        if let Some(e) = self.side().editors.get_mut(&(track, slot)) {
            e.close();
        }
    }

    pub fn editor_open(&self, track: TrackId, slot: usize) -> bool {
        self.side().editors.get(&(track, slot)).is_some_and(|e| e.is_open())
    }

    /// Shows a parameter change made elsewhere in the plugin's open editor.
    pub fn editor_set_param(&self, track: TrackId, slot: usize, id: &str, value: f32) {
        if let Some(e) = self.side().editors.get_mut(&(track, slot)) {
            e.set_param(id, value);
        }
    }

    /// Start playback at `from`, stopping at `end`, or looping `looped`.
    pub fn play(&self, from: Samples, end: Option<Samples>, looped: Option<Range>) {
        self.shared.position.store(from, Ordering::Relaxed);
        self.shared.playing.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Cmd::Play { from, end, looped });
    }

    /// Count Off: the click sounds on beats before `at` (None = off). Realtime only.
    pub fn set_click_until(&self, at: Option<Samples>) {
        let _ = self.tx.send(Cmd::ClickUntil(at.unwrap_or(Samples::MIN)));
    }

    /// Feed live input (from a `record::Recorder`) into the mix for input monitoring.
    pub fn set_input(&self, ring: Arc<record::InputRing>) {
        let _ = self.tx.send(Cmd::Input(ring));
    }

    pub fn set_recording(&self, on: bool) {
        let _ = self.tx.send(Cmd::Recording(on));
    }

    pub fn stop(&self) {
        self.shared.playing.store(false, Ordering::Relaxed);
        let _ = self.tx.send(Cmd::Stop);
    }

    pub fn locate(&self, at: Samples) {
        self.shared.position.store(at, Ordering::Relaxed);
        let _ = self.tx.send(Cmd::Locate(at));
    }

    /// Monitor-only volume/dim/mono (R37): the speaker path, never a bounce.
    /// The monitor-only auxiliary ring (assistant speech, mastering reference).
    pub fn aux(&self) -> &AuxMonitor {
        &self.shared.aux
    }

    pub fn set_monitor(&self, volume_db: f32, dim: bool, mono: bool) {
        let v = if volume_db.is_finite() { (volume_db.clamp(-90.0, 0.0) * 1000.0) as i32 } else { 0 };
        self.shared.monitor_db_milli.store(v, Ordering::Relaxed);
        self.shared.monitor_dim.store(dim, Ordering::Relaxed);
        self.shared.monitor_mono.store(mono, Ordering::Relaxed);
    }

    pub fn set_speed(&self, speed: f32) {
        let v = if speed.is_finite() { (speed.clamp(0.1, 4.0) * 1000.0) as u32 } else { 1000 };
        self.shared.speed.store(v, Ordering::Relaxed);
    }

    pub fn position(&self) -> Samples {
        self.shared.position.load(Ordering::Relaxed)
    }

    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Ordering::Relaxed)
    }

    /// The latest playback anchor: the backend instant at which session sample `pos` is presented.
    pub fn play_anchor(&self) -> Option<(cpal::StreamInstant, Samples)> {
        self.shared.play_anchor.lock().map(|a| *a).unwrap_or(None)
    }

    /// True once after the audio thread saw the compensation latency change while recording.
    /// Why the output stream stopped (device unplugged / driver error), if it did. The
    /// transport cannot advance on this stream; reopen the device (Playback Engine).
    pub fn output_lost(&self) -> Option<&'static str> {
        lost_reason(self.shared.output_lost.load(Ordering::Acquire))
    }

    pub fn take_latency_alarm(&self) -> bool {
        self.shared.latency_alarm.swap(false, Ordering::AcqRel)
    }

    /// Stream clock telemetry: (generation, device frames serviced, last callback size).
    pub fn stream_clock(&self) -> (u64, u64, u32) {
        (
            self.shared.stream_generation.load(Ordering::Relaxed),
            self.shared.stream_frames.load(Ordering::Relaxed),
            self.shared.last_callback_frames.load(Ordering::Relaxed),
        )
    }

    pub fn meters(&self) -> MeterSnapshot {
        self.shared.meters.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

impl Side {
    /// Creates the third-party instances `s` needs that the audio thread does not have yet.
    fn prepare(&mut self, s: &Session) -> Vec<PreparedInstance> {
        self.collect_garbage();
        let sr = s.sample_rate.as_f64() as f32;
        if (sr - self.rate).abs() > 0.5 {
            // The audio thread rebuilds its engine: everything is created anew.
            let keys: Vec<(TrackId, usize)> = self.shadow.keys().copied().collect();
            for k in keys {
                self.forget(k);
            }
            self.rate = sr;
        }
        let specs = soundcraft_mix::instance_specs(s);
        let stale: Vec<(TrackId, usize)> = self.shadow.keys().filter(|k| !specs.iter().any(|x| (x.track, x.slot) == **k)).copied().collect();
        for k in stale {
            self.forget(k);
        }
        let mut out = Vec::new();
        for spec in specs {
            let key = (spec.track, spec.slot);
            if self.shadow.get(&key).is_some_and(|(id, ch, _)| *id == spec.id && *ch == spec.ch) {
                continue;
            }
            self.forget(key);
            let made = soundcraft_mix::create_instance(&spec.id, spec.state.as_deref(), sr, BLOCK, spec.ch);
            self.shadow.insert(key, (spec.id.clone(), spec.ch, made.is_some()));
            if let Some(mut plugin) = made {
                if let Some(e) = plugin.editor() {
                    self.editors.insert(key, e);
                }
                out.push(PreparedInstance {
                    track: spec.track,
                    slot: spec.slot,
                    id: spec.id,
                    ch: spec.ch,
                    plugin,
                    applied: vec![f32::NAN; spec.params],
                });
            }
        }
        out
    }
}

/// ITU stereo fold-down gains for each main channel of a session.
fn fold_gains(s: &Session) -> [(f32, f32); MAX_CHANNELS] {
    let mut g = [(0.0, 0.0); MAX_CHANNELS];
    let f = s.main_format();
    let sp = f.speakers();
    if sp.len() == f.channels() {
        for (o, x) in g.iter_mut().zip(sp.iter()) {
            *o = itu_stereo_fold(*x);
        }
    } else {
        // Ambisonics: W only, at -3 dB per side.
        g[0] = (std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2);
    }
    g
}

/// Open the default output device: f32 at the session rate if possible, with as many channels as
/// it offers up to `want_ch` (at least stereo when available).
fn open_device(want_rate: u32, want_ch: u16) -> Result<(cpal::Device, cpal::StreamConfig, String), String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or_else(|| "no default output device".to_string())?;
    #[allow(deprecated)]
    let name = device.name().unwrap_or_else(|_| "Audio device".into());
    // Prefer an f32 config at the session rate; else the device default (we resample).
    let want_ch = want_ch.max(2);
    let mut chosen: Option<cpal::StreamConfig> = None;
    if let Ok(configs) = device.supported_output_configs() {
        for c in configs {
            if c.sample_format() == cpal::SampleFormat::F32
                && c.min_sample_rate().0 <= want_rate
                && c.max_sample_rate().0 >= want_rate
                && c.channels() >= 2
                && c.channels() <= want_ch
                && chosen.as_ref().is_none_or(|x| c.channels() > x.channels)
            {
                chosen = Some(c.with_sample_rate(cpal::SampleRate(want_rate)).config());
            }
        }
    }
    let config = match chosen {
        Some(c) => c,
        None => {
            let d = device.default_output_config().map_err(|e| e.to_string())?;
            if d.sample_format() != cpal::SampleFormat::F32 {
                return Err(format!("unsupported device sample format {:?}", d.sample_format()));
            }
            d.config()
        }
    };
    Ok((device, config, name))
}

#[cfg(test)]
mod monitor_tests {
    use super::{apply_monitor, monitor_gain};

    /// R37: volume, dim and mono act on the output frame only; unity leaves samples untouched.
    #[test]
    fn monitor_stage_math() {
        let mut f = [0.5f32, -0.25, 0.1];
        apply_monitor(&mut f, 1.0, false);
        assert_eq!(f, [0.5, -0.25, 0.1], "unity is bit-exact");
        apply_monitor(&mut f, monitor_gain(-6_000, false), false);
        assert!((f[0] - 0.5 * 0.501_187).abs() < 1e-4);
        let mut g = [0.8f32, -0.4];
        apply_monitor(&mut g, 1.0, true);
        assert_eq!(g, [0.2, 0.2], "mono sums the first two channels");
        assert!((monitor_gain(0, true) - 0.1).abs() < 1e-6, "dim is −20 dB");
        assert_eq!(monitor_gain(-90_000, false), 0.0);
        assert_eq!(monitor_gain(5_000, false), 1.0, "never above unity");
    }
}

#[cfg(test)]
mod tests {
    /// A session command with its graph prepared the way `update_session` does it.
    fn session_cmd(s: Arc<soundcraft_model::Session>, instances: Vec<PreparedInstance>) -> Cmd {
        let prep = GraphPrep::build(&s, s.sample_rate.as_f64() as f32, BLOCK, instances, 0);
        Cmd::Session(s, Box::new(prep), None)
    }

    /// Like `update_session` when the sample rate changed: a fresh engine comes along.
    fn session_cmd_new_rate(s: Arc<soundcraft_model::Session>, instances: Vec<PreparedInstance>) -> Cmd {
        let sr = s.sample_rate.as_f64() as f32;
        let prep = GraphPrep::build(&s, sr, BLOCK, instances, 0);
        Cmd::Session(s, Box::new(prep), Some(Box::new(new_engine(sr))))
    }
    use super::*;

    #[test]
    fn audio_state_plays_and_stops_at_end() {
        let s = Arc::new(soundcraft_model::Session::default());
        let shared = Arc::new(Shared::new());
        let (tx, rx) = channel();
        let mut st = AudioState::new(rx, Arc::clone(&shared), s, 48_000.0, channel().0);
        tx.send(Cmd::Play { from: 0, end: Some(1000), looped: None }).unwrap();
        let mut out = vec![0.0f32; 4096];
        st.fill(&mut out, 2);
        assert!(shared.position.load(Ordering::Relaxed) >= 1000);
        assert!(!shared.playing.load(Ordering::Relaxed));
    }

    #[test]
    fn surround_main_maps_to_device_channels_or_folds_down() {
        use soundcraft_model::{ChannelFormat, Clip, SourceAudio, SourceId, TrackKind};
        let mut s = soundcraft_model::Session::default();
        s.outputs[0].format = ChannelFormat::Surround51;
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Surround51, None);
        let chans: Vec<Vec<f32>> = (0..6).map(|c| vec![0.1 * (c + 1) as f32; 48_000]).collect();
        s.pool.insert(SourceId(5), Arc::new(SourceAudio::new(soundcraft_audio_io::AudioBuffer { sample_rate: 48_000, channels: chans })));
        let id = s.new_clip_id();
        s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "x", SourceId(5), 0, 0, 48_000));
        let s = Arc::new(s);
        let shared = Arc::new(Shared::new());
        let (tx, rx) = channel();
        let mut st = AudioState::new(rx, Arc::clone(&shared), Arc::clone(&s), 48_000.0, channel().0);
        tx.send(Cmd::Play { from: 0, end: None, looped: None }).unwrap();
        // An 8-channel device: channels in order, the rest silent.
        let mut out = vec![0.0f32; 8 * 256];
        st.fill(&mut out, 8);
        let f = &out[8 * 100..8 * 101];
        for c in 0..6 {
            assert!((f[c] - 0.1 * (c + 1) as f32).abs() < 1e-5, "{f:?}");
        }
        assert!(f[6] == 0.0 && f[7] == 0.0);
        // A stereo device: ITU fold-down.
        let mut out = vec![0.0f32; 2 * 256];
        st.fill(&mut out, 2);
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let l = (0.1 + h * 0.3 + h * 0.5f32).min(1.0);
        assert!((out[200] - l).abs() < 1e-4, "{} vs {l}", out[200]);
        assert_eq!(shared.meters.lock().unwrap().main.peaks.len(), 6);
    }

    struct Stateful(Box<dyn Plugin>);

    impl Plugin for Stateful {
        fn info(&self) -> &'static soundcraft_dsp::PluginInfo {
            self.0.info()
        }
        fn prepare(&mut self, sr: f32, mb: usize, ch: usize) {
            self.0.prepare(sr, mb, ch);
        }
        fn reset(&mut self) {
            self.0.reset();
        }
        fn set_param(&mut self, id: &str, v: f32) -> bool {
            self.0.set_param(id, v)
        }
        fn param(&self, id: &str) -> Option<f32> {
            self.0.param(id)
        }
        fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
            self.0.process(io, frames);
        }
        fn save_state(&mut self) -> Option<Vec<u8>> {
            Some(b"blob".to_vec())
        }
    }

    #[test]
    fn instances_arrive_with_the_session_and_leave_through_the_return_channel() {
        use soundcraft_model::{ChannelFormat, Insert, TrackKind};
        let mut s = soundcraft_model::Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Stereo, None);
        s.track_mut(t).unwrap().mixer.inserts[1] = Some(Insert::new("clap:test.fake"));
        let s = Arc::new(s);
        let shared = Arc::new(Shared::new());
        let (tx, rx) = channel();
        let (gtx, grx) = channel();
        let mut st = AudioState::new(rx, Arc::clone(&shared), Arc::new(soundcraft_model::Session::default()), 48_000.0, gtx);
        let plugin: Box<dyn Plugin> = Box::new(Stateful(soundcraft_mix::test_providers::create("test:gain:0").unwrap()));
        tx.send(session_cmd(
            Arc::clone(&s),
            vec![PreparedInstance { track: t, slot: 1, id: "clap:test.fake".into(), ch: 2, plugin, applied: Vec::new() }],
        ))
        .unwrap();
        let (rtx, rrx) = channel();
        tx.send(Cmd::CaptureStates(rtx)).unwrap();
        // Stopped: the callback only drains commands (adopting the instance) and answers.
        let mut out = vec![0.0f32; 256];
        st.fill(&mut out, 2);
        assert_eq!(rrx.try_recv().unwrap(), vec![(t, 1, b"blob".to_vec())]);
        // Retired instances travel directly or inside an applied graph's leftovers.
        let retired = |grx: &std::sync::mpsc::Receiver<Garbage>| {
            grx.try_iter()
                .map(|mut g| {
                    g.plugins.len()
                        + g.graph.as_mut().map_or(0, |p| p.drain_plugins().len())
                        + g.engine.as_mut().map_or(0, |e| e.drain_plugins().len())
                })
                .sum::<usize>()
        };
        assert_eq!(retired(&grx), 0, "nothing retired yet");
        // A session without the insert retires the instance to the return channel.
        tx.send(session_cmd(Arc::new(soundcraft_model::Session::default()), Vec::new())).unwrap();
        st.fill(&mut out, 2);
        assert_eq!(retired(&grx), 1);
        // A sample-rate change rebuilds the engine without dropping plugins on this thread.
        tx.send(session_cmd(
            Arc::clone(&s),
            vec![PreparedInstance {
                track: t,
                slot: 1,
                id: "clap:test.fake".into(),
                ch: 2,
                plugin: Box::new(Stateful(soundcraft_mix::test_providers::create("test:gain:0").unwrap())),
                applied: Vec::new(),
            }],
        ))
        .unwrap();
        st.fill(&mut out, 2);
        let mut s44 = (*s).clone();
        s44.sample_rate = soundcraft_time::SampleRate::new(44_100).unwrap();
        tx.send(session_cmd_new_rate(Arc::new(s44), Vec::new())).unwrap();
        st.fill(&mut out, 2);
        assert_eq!(retired(&grx), 1);
    }

    /// Count Off reaches the engine for exactly the next Play; Stop or a later Play without a
    /// new count-off clears it.
    #[test]
    fn countoff_applies_to_the_next_play_only() {
        let shared = Arc::new(Shared::new());
        let (tx, rx) = channel();
        let mut st = AudioState::new(rx, Arc::clone(&shared), Arc::new(soundcraft_model::Session::default()), 48_000.0, channel().0);
        let mut out = vec![0.0f32; 256];
        tx.send(Cmd::ClickUntil(96_000)).unwrap();
        tx.send(Cmd::Play { from: 0, end: None, looped: None }).unwrap();
        st.fill(&mut out, 2);
        assert_eq!(st.mix.click_until, 96_000);
        tx.send(Cmd::Stop).unwrap();
        st.fill(&mut out, 2);
        assert_eq!(st.mix.click_until, Samples::MIN);
        tx.send(Cmd::ClickUntil(96_000)).unwrap();
        tx.send(Cmd::Play { from: 0, end: None, looped: None }).unwrap();
        tx.send(Cmd::Play { from: 0, end: None, looped: None }).unwrap();
        st.fill(&mut out, 2);
        assert_eq!(st.mix.click_until, Samples::MIN, "a later Play without a count-off clears it");
    }

    /// CL-T2 output disconnect: the first stream error's reason is kept and reported.
    #[test]
    fn output_loss_reason_is_reported() {
        let gone = cpal::StreamError::DeviceNotAvailable;
        let other = cpal::StreamError::BackendSpecific { err: cpal::BackendSpecificError { description: "xrun storm".into() } };
        assert_eq!((stream_error_code(&gone), stream_error_code(&other)), (1, 2));
        assert_eq!(lost_reason(0), None);
        assert_eq!(lost_reason(stream_error_code(&gone)), Some("the output device is no longer available"));
        assert_eq!(lost_reason(stream_error_code(&other)), Some("the output driver reported an error"));
        // Input side: the first failure wins.
        let st = record::StreamStats::default();
        st.mark_lost(stream_error_code(&gone));
        st.mark_lost(stream_error_code(&other));
        assert_eq!(st.lost_reason(), Some("the input device is no longer available"));
    }

    /// CL05: a replaced session is handed back to the UI thread, so its last reference is not
    /// released (and the whole document freed) inside the audio callback.
    #[test]
    fn replaced_sessions_are_dropped_off_the_audio_thread() {
        let shared = Arc::new(Shared::new());
        let (tx, rx) = channel();
        let (gtx, grx) = channel();
        let first = Arc::new(soundcraft_model::Session::default());
        let weak = Arc::downgrade(&first);
        let mut st = AudioState::new(rx, Arc::clone(&shared), first, 48_000.0, gtx);
        tx.send(session_cmd(Arc::new(soundcraft_model::Session::default()), Vec::new())).unwrap();
        let mut out = vec![0.0f32; 256];
        st.fill(&mut out, 2);
        assert!(weak.upgrade().is_some(), "the audio thread did not free the old session");
        // One message carries the graph leftovers, another the replaced session.
        let garbage: Vec<Garbage> = grx.try_iter().collect();
        assert!(garbage.iter().all(|g| g.plugins.is_empty()));
        assert!(garbage.iter().any(|g| g._session.is_some()), "the old session travels back");
        drop(garbage);
        assert!(weak.upgrade().is_none(), "freed where the UI thread drops the garbage");
    }

    #[test]
    fn side_creates_each_missing_instance_once() {
        use soundcraft_model::{ChannelFormat, Insert, TrackKind};
        let (_gtx, grx) = channel();
        let mut side = Side { shadow: HashMap::new(), rate: 48_000.0, editors: HashMap::new(), garbage: grx, prev_tracks: 0 };
        let mut s = soundcraft_model::Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        s.track_mut(t).unwrap().mixer.inserts[0] = Some(Insert::new("clap:does.not.exist"));
        s.track_mut(t).unwrap().mixer.inserts[1] = Some(Insert::new("gain"));
        assert!(side.prepare(&s).is_empty(), "the plugin is not installed; built-ins stay with the engine");
        assert_eq!(side.shadow.get(&(t, 0)), Some(&("clap:does.not.exist".to_string(), 1, false)));
        assert!(side.prepare(&s).is_empty());
        assert_eq!(side.shadow.len(), 1, "failures are remembered, not retried every update");
        s.track_mut(t).unwrap().mixer.inserts[0] = None;
        side.prepare(&s);
        assert!(side.shadow.is_empty());
    }

    /// CL02: at equal rates the live path renders exactly each callback's frames: after a 64-frame
    /// callback the transport is at 64 (not a 512-frame block ahead) and nothing stays queued.
    /// Variable callback sizes keep exact frame accounting and a monotonic stream clock.
    #[test]
    fn live_path_renders_exactly_the_callback_size() {
        let s = Arc::new(soundcraft_model::Session::default());
        let shared = Arc::new(Shared::new());
        let (tx, rx) = channel();
        let mut st = AudioState::new(rx, Arc::clone(&shared), s, 48_000.0, channel().0);
        tx.send(Cmd::Play { from: 1000, end: None, looped: None }).unwrap();
        let mut total = 0i64;
        let g = st.clock.generation();
        for n in [64usize, 64, 127, 128, 32, 256, 1024, 7] {
            let mut out = vec![0.0f32; n * 2];
            st.fill(&mut out, 2);
            total += n as i64;
            assert_eq!(shared.position.load(Ordering::Relaxed), 1000 + total, "after a {n}-frame callback");
            assert_eq!(st.pending_len as f64 - st.read, 0.0, "no audio queued past the callback");
        }
        assert_eq!(st.clock.frames(), total as u64);
        assert_eq!(st.clock.generation(), g);
        assert_eq!(shared.stream_frames.load(Ordering::Relaxed), total as u64);
    }

    /// CL04 anchor: the output callback records which session sample is presented at the
    /// backend's playback instant (exact rendering → nothing queued, so it is the transport pos).
    #[test]
    fn playback_anchor_maps_instant_to_session_position() {
        let s = Arc::new(soundcraft_model::Session::default());
        let shared = Arc::new(Shared::new());
        let (tx, rx) = channel();
        let mut st = AudioState::new(rx, Arc::clone(&shared), s, 48_000.0, channel().0);
        tx.send(Cmd::Play { from: 5_000, end: None, looped: None }).unwrap();
        let mut out = vec![0.0f32; 256];
        st.fill_stamped(&mut out, 2, cpal::StreamInstant::new(10, 0));
        assert_eq!(*shared.play_anchor.lock().unwrap(), Some((cpal::StreamInstant::new(10, 0), 5_000)));
        st.fill_stamped(&mut out, 2, cpal::StreamInstant::new(10, 2_666_667));
        assert_eq!(shared.play_anchor.lock().unwrap().unwrap().1, 5_128);
    }

    #[test]
    fn looping_wraps() {
        let s = Arc::new(soundcraft_model::Session::default());
        let shared = Arc::new(Shared::new());
        let (tx, rx) = channel();
        let mut st = AudioState::new(rx, Arc::clone(&shared), s, 48_000.0, channel().0);
        tx.send(Cmd::Play { from: 0, end: None, looped: Some(Range::new(0, 1000)) }).unwrap();
        let mut out = vec![0.0f32; 20_000];
        st.fill(&mut out, 2);
        let p = shared.position.load(Ordering::Relaxed);
        assert!(p <= 1000, "position {p} should stay inside the loop");
        assert!(shared.playing.load(Ordering::Relaxed));
    }
}

#[cfg(test)]
mod aux_monitor_tests {
    use super::*;

    /// R33/R34: auxiliary monitor audio reaches the device output only. A render of the same
    /// session is unchanged, nothing enters the capture path, speech is dropped while recording,
    /// and the reference "solo" silences the program on the device output only.
    #[test]
    fn aux_audio_is_monitor_only() {
        let aux = AuxMonitor::new();
        let frames: Vec<f32> = (0..64).flat_map(|i| [0.5, if i % 2 == 0 { 0.25 } else { -0.25 }]).collect();
        assert_eq!(aux.push(&frames), 64);
        assert_eq!(aux.queued_frames(), 64);
        let mut frame = [0.1f32, 0.1];
        aux.pop_into(&mut frame, false);
        assert!((frame[0] - 0.6).abs() < 1e-6 && (frame[1] - 0.35).abs() < 1e-6, "summed after the program: {frame:?}");
        // Recording: dropped, not played.
        let mut frame = [0.1f32, 0.1];
        aux.pop_into(&mut frame, true);
        assert_eq!(frame, [0.1, 0.1]);
        assert_eq!(aux.queued_frames(), 62);
        aux.set_gain_db(-6.0);
        let mut frame = [0.0f32, 0.0];
        aux.pop_into(&mut frame, false);
        assert!((frame[0] - 0.5 * 0.501).abs() < 1e-2, "{frame:?}");
        aux.clear();
        assert_eq!(aux.queued_frames(), 0);
        let mut frame = [0.3f32, 0.3];
        aux.pop_into(&mut frame, false);
        assert_eq!(frame, [0.3, 0.3], "empty ring leaves the program untouched");
        // The ring is a device-output concern: `render_next` / `soundcraft_mix::render_range`
        // never read it (there is no path from AuxMonitor into MixEngine or the capture ring).
        let s = soundcraft_model::Session::default();
        let before = soundcraft_mix::render_range(&s, soundcraft_time::Range::new(0, 256), 128);
        aux.push(&frames);
        let after = soundcraft_mix::render_range(&s, soundcraft_time::Range::new(0, 256), 128);
        assert_eq!(before, after);
    }

    /// RC05 / T02: assistant speech (the built-in synthesizer, 16 kHz) is converted to the device
    /// rate and queued on the monitor ring only: its length matches the rate change, a render is
    /// unchanged, and it is dropped (not played, not delayed) while recording.
    #[test]
    fn assistant_speech_goes_to_the_monitor_ring_only() {
        let aux = AuxMonitor::new();
        let speech = soundcraft_speech::synth::synthesize("kick muted", soundcraft_speech::synth::Voice::default());
        let n = aux.push_speech(&speech, soundcraft_speech::SPEECH_RATE, 48_000);
        let want = speech.len() * 3;
        assert!(n.abs_diff(want) <= 4, "{n} vs {want}");
        assert_eq!(aux.queued_frames(), n);
        let s = soundcraft_model::Session::default();
        let r = soundcraft_mix::render_range(&s, soundcraft_time::Range::new(0, 4_096), 512);
        assert!(r.iter().flatten().all(|x| *x == 0.0), "a render never contains assistant speech");
        let mut heard = 0.0f32;
        for _ in 0..n {
            let mut frame = [0.0f32, 0.0];
            aux.pop_into(&mut frame, true);
            heard = heard.max(frame[0].abs());
        }
        assert_eq!(heard, 0.0, "silent while recording");
        assert_eq!(aux.queued_frames(), 0, "dropped, not kept for later");
        // T03: cancelling queued speech clears it.
        aux.push_speech(&speech, soundcraft_speech::SPEECH_RATE, 44_100);
        aux.clear();
        assert_eq!(aux.queued_frames(), 0);
    }
}
