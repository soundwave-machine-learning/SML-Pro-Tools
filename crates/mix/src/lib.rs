//! The SoundCraft mix engine.
//!
//! [`MixEngine::render`] produces one block of the main output for a session at a timeline
//! position: clips (with clip gain and fades) → trim/phase → inserts → pre-fader sends → fader and
//! mute (with automation) → pan → post-fader sends → busses → aux inputs → main → master faders.
//! The same code drives realtime playback and offline bounces, so what you hear is what you get.
//! Steady-state rendering does not allocate.
//!
//! ## Surround
//! The main mix has the format of the session's main output path ([`Session::main_format`]:
//! stereo by default, up to 16 channels such as 7.1.4 or 9.1.6) and every bus has its own format.
//! Channels are in SMPTE / WAV order (L R C LFE Ls Rs …, see
//! [`ChannelFormat::speakers`](soundcraft_model::ChannelFormat::speakers)). Routing a strip into a
//! destination builds a gain matrix per block:
//! - mono/stereo sources into a stereo destination use the stereo pan exactly as before;
//! - into a multichannel destination they use the track's [`SurroundPan`] (pairwise
//!   constant-power panner over the speaker layout, see `soundcraft_dsp::pan::surround_gains`),
//!   or go to L/R with the stereo pan when the track has no surround pan;
//! - sources whose format equals the destination's pass straight through, channel for channel;
//! - other multichannel sources fold to stereo with ITU-R BS.775 coefficients (C and surrounds at
//!   -3 dB, LFE dropped), or map speaker by speaker into other multichannel layouts.
//!
//! ## Plugin instances and threads
//! [`MixEngine::sync`] brings plugin instances in line with the session. Built-in plugins are
//! cheap (allocation only) and are always created there, on whichever thread renders. Hosted
//! third-party plugins (`clap:` / `vst3:` ids, see [`is_third_party`]) are expensive and have
//! main-thread rules, so a realtime host keeps them off the audio thread:
//! - with [`MixEngine::set_external_instances`]`(true)` (realtime playback), `sync` never creates a
//!   third-party plugin. The host's control (UI) thread creates them with [`create_instance`] from
//!   [`instance_specs`] (restoring each insert's stored [`Insert::state`] and calling `prepare`
//!   there), and hands them over with [`MixEngine::adopt`]; `sync` then moves the matching one
//!   into its slot. A slot whose instance has not arrived yet passes audio through.
//! - every instance `sync` replaces or removes (built-in or not) goes to a retired list instead of
//!   being dropped; the host takes it with [`MixEngine::take_retired`] and drops it on the control
//!   thread, so plugin destruction never runs on the audio thread either.
//! - without external instances (the default: offline [`render_range`], bounces, tests) `sync`
//!   creates third-party plugins itself, synchronously, restores their stored state before the
//!   first `process`, and drops retired instances at the end of `sync`.
//!
//! An instance starts from its stored state; the insert's parameter values are then applied on
//! top before the first block (the session's values are kept in step with the plugin, including
//! edits made in its own editor, so both agree; the state carries everything else).
//! [`MixEngine::capture_states`] reads every live instance's state back (for saving the session);
//! it calls into the plugins and allocates, so a realtime host runs it only on request.
//!
//! [`Insert::state`]: soundcraft_model::Insert::state
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use soundcraft_dsp::pan::{PanLaw, SpeakerPos, SurroundParams, gains};
use soundcraft_dsp::{Plugin, db_to_gain};
use soundcraft_model::{
    AuditionBypass, AutoParam, BusId, ChannelFormat, Clip, ClipContent, DedicatedSlot, ProfileKind, Route, Session, SlotKind, Speaker, StereoPanMode,
    SurroundPan, TapePosition, Track, TrackId, TrackKind,
};
use soundcraft_time::{Range, Samples};
use std::collections::HashMap;

#[cfg(any(test, feature = "test-providers"))]
pub mod test_providers;

#[cfg(not(target_arch = "wasm32"))]
pub mod sandbox;
#[cfg(not(target_arch = "wasm32"))]
pub mod scan_isolated;

/// True when a session insert/slot id cannot be instantiated in this build: a removed upstream
/// built-in, or an external plugin that is not installed. Such entries keep their id, parameters
/// and state and pass audio through (an inactive missing placeholder).
pub fn is_missing_plugin(id: &str) -> bool {
    #[cfg(any(test, feature = "test-providers"))]
    if id.starts_with(test_providers::PREFIX) {
        return false;
    }
    if soundcraft_dsp::is_removed_builtin(id) || !is_third_party(id) {
        return true;
    }
    let id = hosted_id(id);
    soundcraft_clap_host::plugin_info(id).is_none()
        && soundcraft_vst3_host::plugin_info(id).is_none()
        && soundcraft_au_host::plugin_info(id).is_none()
}

/// Default (stereo) main mix width. The actual width is [`main_channels`].
pub const MAIN_CHANNELS: usize = 2;

/// Widest strip, bus or main mix (9.1.6 / 3rd-order Ambisonics). Wider formats are truncated.
pub const MAX_CHANNELS: usize = 16;

/// Number of channels of a session's main mix.
pub fn main_channels(s: &Session) -> usize {
    s.main_format().channels().clamp(MAIN_CHANNELS, MAX_CHANNELS)
}

/// Slot index used for a track's instrument plugin in [`InstanceSpec`], [`PreparedInstance`] and
/// [`MixEngine::capture_states`] (insert slots use their index).
pub const INSTRUMENT_SLOT: usize = usize::MAX;

/// Hosted third-party plugin ids (CLAP or VST3): created off the audio thread in realtime use.
pub fn is_third_party(id: &str) -> bool {
    let id = id.strip_prefix(SANDBOX_PREFIX).unwrap_or(id);
    id.starts_with(soundcraft_clap_host::ID_PREFIX)
        || id.starts_with(soundcraft_vst3_host::ID_PREFIX)
        || id.starts_with(soundcraft_au_host::ID_PREFIX)
}

/// Prefix of ids hosted out of process (`sandbox:clap:…`, see [`sandbox`]).
pub const SANDBOX_PREFIX: &str = "sandbox:";

/// The hosted plugin id without a `sandbox:` prefix.
pub fn hosted_id(id: &str) -> &str {
    id.strip_prefix(SANDBOX_PREFIX).unwrap_or(id)
}

/// Plugins that are created off the audio thread and handed to the engine as prepared
/// instances: hosted plugins, plus the test providers (feature `test-providers`).
fn is_prepared_externally(id: &str) -> bool {
    #[cfg(any(test, feature = "test-providers"))]
    if id.starts_with(test_providers::PREFIX) {
        return true;
    }
    is_third_party(id)
}

/// A third-party plugin instance a session needs: which slot, which plugin, how many channels,
/// and the state to restore.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceSpec {
    pub track: TrackId,
    /// Insert slot index, or [`INSTRUMENT_SLOT`].
    pub slot: usize,
    pub id: String,
    /// Channel count the instance is prepared for.
    pub ch: usize,
    /// Decoded [`Insert::state`](soundcraft_model::Insert::state).
    pub state: Option<Vec<u8>>,
    /// Number of parameter values the insert carries (sizes the applied-values vector).
    pub params: usize,
}

/// A plugin instance created and prepared off the audio thread, handed to [`MixEngine::adopt`].
pub struct PreparedInstance {
    pub track: TrackId,
    pub slot: usize,
    pub id: String,
    pub ch: usize,
    pub plugin: Box<dyn Plugin>,
    /// Last-applied parameter values, sized to the insert's parameter map off the audio thread
    /// (`vec![NAN; n]`); empty when the caller did not size it (then the first block allocates).
    pub applied: Vec<f32>,
}

/// Every third-party plugin slot (inserts and instruments) of a session, with the channel count
/// [`MixEngine::sync`] prepares it for. Inactive and master tracks are included (they have strips).
pub fn instance_specs(s: &Session) -> Vec<InstanceSpec> {
    let main_ch = main_channels(s);
    let mut out = Vec::new();
    for t in &s.tracks {
        let ch = strip_channels(t, main_ch);
        for (i, ins) in t.mixer.inserts.iter().enumerate() {
            if let Some(ins) = ins
                && is_prepared_externally(&ins.plugin)
            {
                out.push(InstanceSpec { track: t.id, slot: i, id: ins.plugin.clone(), ch, state: ins.state_bytes(), params: ins.params.len() });
            }
        }
        if let Some(ins) = &t.instrument
            && is_prepared_externally(&ins.plugin)
        {
            out.push(InstanceSpec {
                track: t.id,
                slot: INSTRUMENT_SLOT,
                id: ins.plugin.clone(),
                ch: ch.max(2),
                state: ins.state_bytes(),
                params: ins.params.len(),
            });
        }
        for k in SlotKind::ALL {
            if let Some(ins) = t.mixer.slots.get(k).and_then(|x| x.provider.as_ref())
                && is_prepared_externally(&ins.plugin)
            {
                out.push(InstanceSpec {
                    track: t.id,
                    slot: k.instance_index(),
                    id: ins.plugin.clone(),
                    ch,
                    state: ins.state_bytes(),
                    params: ins.params.len(),
                });
            }
        }
    }
    out
}

/// Creates a plugin (built-in or hosted), restores `state` (if any) and prepares it. Expensive
/// for third-party plugins: in realtime use, call it on the control thread.
pub fn create_instance(id: &str, state: Option<&[u8]>, sample_rate: f32, max_block: usize, ch: usize) -> Option<Box<dyn Plugin>> {
    let mut p = create_plugin(id)?;
    if let Some(st) = state
        && !p.load_state(st)
    {
        log::warn!("{id}: the plugin did not accept its stored state");
    }
    p.prepare(sample_rate, max_block, ch);
    Some(p)
}

/// Peak meter values for one strip (linear, per channel, max over the last block).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StripMeter {
    /// First two channels (mono strips repeat channel 0), for stereo meters.
    pub peak: [f32; 2],
    /// Every channel of the strip (or of the main mix).
    pub peaks: Vec<f32>,
    /// Gain reduction of the first dynamics insert, dB.
    pub gain_reduction_db: f32,
    /// R23 dedicated slot I/O peaks, indexed Tape, Summing, Master Tape (`SlotKind` order).
    pub slots: [SlotIo; 3],
}

/// Input and output peak (linear, max over channels, last block) of one dedicated slot. Both
/// zero when the slot is not configured on the strip.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SlotIo {
    pub input: f32,
    pub output: f32,
    /// False when the slot is not configured or not in the graph (Tracking deactivation).
    pub active: bool,
}

impl StripMeter {
    /// Copy another meter without reallocating once the channel count is stable.
    pub fn copy_from(&mut self, o: &StripMeter) {
        self.peak = o.peak;
        self.peaks.clone_from(&o.peaks);
        self.gain_reduction_db = o.gain_reduction_db;
        self.slots = o.slots;
    }

    fn measure(&mut self, bufs: &[Vec<f32>], frames: usize, gr: f32) {
        if self.peaks.len() != bufs.len() {
            self.peaks.resize(bufs.len(), 0.0);
        }
        for (p, b) in self.peaks.iter_mut().zip(bufs.iter()) {
            *p = peak(b.get(..frames).unwrap_or(&[]));
        }
        let last = self.peaks.len().saturating_sub(1);
        self.peak = [self.peaks.first().copied().unwrap_or(0.0), self.peaks.get(1.min(last)).copied().unwrap_or(0.0)];
        self.gain_reduction_db = gr;
    }
}

/// A bus: its format and one buffer per channel.
struct BusBuf {
    fmt: ChannelFormat,
    buf: Vec<Vec<f32>>,
    /// f64 accumulator for graph-version-2 sums (M02); flushed into `buf` before it is read.
    acc: Vec<Vec<f64>>,
    /// The previous block's summed signal: what audio/instrument strips hear as a sidechain
    /// (their bus is still being summed while they run). Kept only while a sidechain uses it.
    prev: Vec<Vec<f32>>,
}

impl BusBuf {
    fn flush(&mut self, frames: usize) {
        for (b, a) in self.buf.iter_mut().zip(self.acc.iter()) {
            for (x, y) in b.iter_mut().zip(a.iter()).take(frames) {
                *x = *y as f32;
            }
        }
    }
}

struct PluginSlot {
    id: String,
    /// Channel count the plugin was prepared for.
    ch: usize,
    plugin: Box<dyn Plugin>,
    /// Last values pushed to the plugin, in the insert's parameter-map order.
    applied: Vec<f32>,
    /// Ordinary bypass keeps compensation: the plugin's latency stays as this plain delay
    /// (reserved for the latency at creation; a later increase beyond it is a counted fallback).
    bypass_delay: DelayLine,
}

struct Strip {
    buf: Vec<Vec<f32>>,
    /// Pre-fader copy for pre-fader sends.
    pre: Vec<Vec<f32>>,
    scratch: Vec<f32>,
    plugins: Vec<Option<PluginSlot>>,
    instrument: Option<PluginSlot>,
    /// MIDI notes currently sounding on the instrument (pitch), for note-offs at stops/seek.
    held: Vec<u8>,
    muted: bool,
    gr: f32,
    /// Per-clip effect chains (Clip Effects) and a scratch buffer to render clips into.
    clip_fx: HashMap<u64, ClipFx>,
    clipbuf: Vec<Vec<f32>>,
    /// Total latency of the active inserts (samples).
    latency: usize,
    /// Delay compensation: align this strip with the slowest one (post- and pre-fader paths),
    /// plus extra delay for direct-to-main outputs so they line up with aux returns.
    align: DelayLine,
    align_pre: DelayLine,
    out_extra: DelayLine,
    outbuf: Vec<Vec<f32>>,
    /// Dedicated slot instances (Tape, Summing, Master Tape) and the delay each keeps while
    /// bypassed so compensation stays valid (SL05).
    dedicated: [Option<PluginSlot>; 3],
    slot_delay: [DelayLine; 3],
    /// I/O peaks of the dedicated slots in the last block (published with the strip meter).
    slot_io: [SlotIo; 3],
    /// Last applied native Stereo Width (for the per-block ramp).
    width_prev: f32,
    /// Excluded by solo (gates every contribution, pre-fader sends included).
    soloed_out: bool,
    /// Muted by mute/automation/VCA (gates the final output and post-fader sends).
    mute_only: bool,
    /// Keeps insert latency while inserts are audition-bypassed.
    audition_delay: DelayLine,
}

/// A multichannel delay line (samples), resized only when the delay changes.
#[derive(Default)]
struct DelayLine {
    bufs: Vec<Vec<f32>>,
    idx: usize,
    len: usize,
}

/// Samples of delay-compensation ring reserved per channel for strips prepared off the audio
/// thread (B01): a latency change within the reserve never allocates in the callback.
pub const PDC_RESERVE: usize = 4096;

/// Allocations the audio thread had to make because a prepared resource was missing or too
/// small (B01 fallback). Zero in the structural-mutation guard; logged once at runtime.
static PREPARED_FALLBACKS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn note_fallback() {
    if on_audio_thread() {
        let n = PREPARED_FALLBACKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if n == 0 {
            log::warn!("mix: allocated on the audio thread outside the prepared graph (B01 fallback)");
        }
    }
}

/// Allocations made on the audio thread outside the prepared graph since the last reset.
pub fn prepared_fallbacks() -> u32 {
    PREPARED_FALLBACKS.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn reset_prepared_fallbacks() {
    PREPARED_FALLBACKS.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// Pushes without growing when the prepared capacity suffices; a growth is a counted fallback.
fn push_counted<T>(v: &mut Vec<T>, x: T) {
    if v.len() == v.capacity() {
        note_fallback();
    }
    v.push(x);
}

impl DelayLine {
    /// Allocates `cap` samples per channel ahead of time (off the audio thread).
    fn reserve(&mut self, cap: usize, ch: usize) {
        self.bufs = vec![vec![0.0; cap]; ch];
        self.len = 0;
        self.idx = 0;
    }

    /// Sets the delay. Reuses the ring's capacity when it suffices (clearing the used part);
    /// otherwise reallocates — on the audio thread that is a counted B01 fallback.
    fn set(&mut self, len: usize, ch: usize) {
        let len = len.min(1 << 20);
        if len == self.len && self.bufs.len() == ch {
            return;
        }
        self.len = len;
        self.idx = 0;
        let cap = self.bufs.first().map_or(0, Vec::len);
        if self.bufs.len() == ch && cap >= len {
            for b in &mut self.bufs {
                b.iter_mut().take(len).for_each(|x| *x = 0.0);
            }
        } else {
            note_fallback();
            self.bufs = vec![vec![0.0; len.max(cap)]; ch];
        }
    }
    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        if self.len == 0 {
            return;
        }
        let start = self.idx;
        for (c, ch) in io.iter_mut().enumerate() {
            let Some(ring) = self.bufs.get_mut(c) else { continue };
            let mut i = start;
            for x in ch.iter_mut().take(frames) {
                if let Some(r) = ring.get_mut(i) {
                    std::mem::swap(r, x);
                }
                i += 1;
                if i >= self.len {
                    i = 0;
                }
            }
        }
        self.idx = (start + frames) % self.len.max(1);
    }
}

/// Clip Effects: an EQ and a compressor per clip, configured from `edit.values`
/// (`clip_fx.<clip>.eq.<param>`, `clip_fx.<clip>.comp.<param>`, `clip_fx.<clip>.gain`).
struct ClipFx {
    eq: Option<Box<dyn Plugin>>,
    comp: Option<Box<dyn Plugin>>,
    gain_db: f32,
    sr: f32,
    ch: usize,
}

impl Strip {
    fn new() -> Self {
        Strip {
            buf: Vec::new(),
            pre: Vec::new(),
            scratch: Vec::new(),
            plugins: Vec::new(),
            instrument: None,
            held: Vec::new(),
            muted: false,
            gr: 0.0,
            clip_fx: HashMap::new(),
            clipbuf: Vec::new(),
            latency: 0,
            align: DelayLine::default(),
            align_pre: DelayLine::default(),
            out_extra: DelayLine::default(),
            outbuf: Vec::new(),
            dedicated: [None, None, None],
            slot_delay: [DelayLine::default(), DelayLine::default(), DelayLine::default()],
            slot_io: [SlotIo::default(); 3],
            width_prev: 1.0,
            soloed_out: false,
            mute_only: false,
            audition_delay: DelayLine::default(),
        }
    }
}

impl Strip {
    /// A strip with every buffer the first blocks would otherwise allocate, built off the audio
    /// thread for [`GraphPrep`].
    fn prepared(ch: usize, mb: usize, inserts: usize) -> Strip {
        let mut st = Strip::new();
        st.buf = vec![vec![0.0; mb]; ch];
        st.pre = (0..ch).map(|_| Vec::with_capacity(mb)).collect();
        st.outbuf = (0..ch).map(|_| Vec::with_capacity(mb)).collect();
        st.scratch = vec![0.0; mb];
        st.plugins = Vec::with_capacity(inserts);
        st.held = Vec::with_capacity(128);
        for d in [&mut st.align, &mut st.align_pre, &mut st.out_extra, &mut st.audition_delay].into_iter().chain(st.slot_delay.iter_mut()) {
            d.reserve(PDC_RESERVE, ch);
        }
        st
    }

    fn retire_all(&mut self, into: &mut Vec<Box<dyn Plugin>>) {
        for p in self.plugins.drain(..).flatten() {
            push_counted(into, p.plugin);
        }
        if let Some(p) = self.instrument.take() {
            push_counted(into, p.plugin);
        }
        for d in self.dedicated.iter_mut() {
            if let Some(p) = d.take() {
                push_counted(into, p.plugin);
            }
        }
    }
}

/// Everything a structural session change may need on the audio thread, allocated off it
/// (B01): maps with reserved capacity, a fresh strip/bus/meter per track and bus, the processing
/// order and the plugin instances. [`MixEngine::apply_prepared`] moves what already exists across
/// and fills the rest from here; afterwards the same value holds the leftovers (old maps, unused
/// strips, retired instances) so the UI thread frees them.
pub struct GraphPrep {
    strips: HashMap<TrackId, Strip>,
    fresh: Vec<(TrackId, Strip)>,
    busses: HashMap<BusId, BusBuf>,
    fresh_busses: Vec<(BusId, BusBuf)>,
    meters: HashMap<TrackId, StripMeter>,
    fresh_meters: Vec<(TrackId, StripMeter)>,
    order: Vec<usize>,
    work: Vec<(usize, Strip)>,
    main: Vec<Vec<f32>>,
    main_acc: Vec<Vec<f64>>,
    instances: Vec<PreparedInstance>,
    /// One bypass-compensation ring per prepared instance (same index), reserved off thread.
    bypass_delays: Vec<DelayLine>,
    /// Whole slots retired by `apply_prepared` (plugin, id string, applied values): dropped
    /// off the audio thread.
    retired: Vec<PluginSlot>,
    /// Prepared instances that no slot took (dropped off the audio thread).
    unplaced: Vec<PreparedInstance>,
    orphans: Vec<Strip>,
    orphan_busses: Vec<BusBuf>,
    orphan_meters: Vec<StripMeter>,
    old_order: Vec<usize>,
    old_work: Vec<(usize, Strip)>,
}

impl GraphPrep {
    /// Prepares resources for rendering `s` with an engine of `max_block` frames. `instances`
    /// are the hosted/test instances the session needs (see [`prepare_instances`]);
    /// `previous_tracks` bounds how many strips the engine may retire (its last session size).
    pub fn build(s: &Session, _sample_rate: f32, max_block: usize, instances: Vec<PreparedInstance>, previous_tracks: usize) -> GraphPrep {
        let mb = max_block.clamp(16, 16_384);
        let main_ch = main_channels(s);
        let n = s.tracks.len();
        let slack = 8;
        let fresh: Vec<(TrackId, Strip)> =
            s.tracks.iter().map(|t| (t.id, Strip::prepared(strip_channels(t, main_ch), mb, t.mixer.inserts.len()))).collect();
        let fresh_meters: Vec<(TrackId, StripMeter)> = s
            .tracks
            .iter()
            .map(|t| (t.id, StripMeter { peaks: Vec::with_capacity(strip_channels(t, main_ch).max(2)), ..StripMeter::default() }))
            .collect();
        let fresh_busses: Vec<(BusId, BusBuf)> = s
            .busses
            .iter()
            .map(|b| {
                let c = b.format.channels().clamp(1, MAX_CHANNELS);
                (b.id, BusBuf { fmt: b.format, buf: vec![vec![0.0; mb]; c], acc: vec![vec![0.0; mb]; c], prev: vec![vec![0.0; mb]; c] })
            })
            .collect();
        let n_inst = instances.len();
        let retire_cap = (previous_tracks.max(n) + slack) * (soundcraft_model::INSERT_SLOTS + 4) + n_inst;
        GraphPrep {
            strips: HashMap::with_capacity(n + slack),
            fresh,
            busses: HashMap::with_capacity(s.busses.len() + slack),
            fresh_busses,
            meters: HashMap::with_capacity(n + slack),
            fresh_meters,
            order: processing_order(s),
            work: Vec::with_capacity(n + slack),
            main: vec![vec![0.0; mb]; main_ch],
            main_acc: vec![vec![0.0; mb]; main_ch],
            bypass_delays: instances
                .iter()
                .map(|p| {
                    let mut d = DelayLine::default();
                    d.reserve(p.plugin.latency(), p.ch);
                    d
                })
                .collect(),
            instances,
            retired: Vec::with_capacity(retire_cap),
            unplaced: Vec::with_capacity(n_inst + slack),
            orphans: Vec::with_capacity(previous_tracks + n + slack),
            orphan_busses: Vec::with_capacity(previous_tracks + s.busses.len() + slack),
            orphan_meters: Vec::with_capacity(previous_tracks + n + slack),
            old_order: Vec::new(),
            old_work: Vec::new(),
        }
    }

    /// Plugin instances left over (retired or never placed), for a host that drops them itself.
    pub fn drain_plugins(&mut self) -> Vec<Box<dyn Plugin>> {
        let mut out: Vec<Box<dyn Plugin>> = self.retired.drain(..).map(|s| s.plugin).collect();
        out.extend(self.instances.drain(..).chain(self.unplaced.drain(..)).map(|p| p.plugin));
        for st in self.orphans.iter_mut().chain(self.fresh.iter_mut().map(|(_, s)| s)).chain(self.old_work.iter_mut().map(|(_, s)| s)) {
            st.retire_all(&mut out);
        }
        for st in self.strips.values_mut() {
            st.retire_all(&mut out);
        }
        out
    }

    /// The prepared instance for this slot, if one matches; a mismatched one stays here (dropped
    /// off the audio thread).
    fn take_instance(&mut self, t: TrackId, slot: usize, id: &str, ch: usize) -> Option<PluginSlot> {
        let i = self.instances.iter().position(|p| p.track == t && p.slot == slot && p.id == id && p.ch == ch)?;
        let p = self.instances.swap_remove(i);
        // Same index as `instances`: swap_remove keeps the two vectors aligned.
        let bypass_delay = if i < self.bypass_delays.len() { self.bypass_delays.swap_remove(i) } else { DelayLine::default() };
        Some(PluginSlot { id: p.id, ch, plugin: p.plugin, applied: p.applied, bypass_delay })
    }

    fn retire(&mut self, slot: Option<PluginSlot>) {
        if let Some(s) = slot {
            push_counted(&mut self.retired, s);
        }
    }

    /// Moves every instance of `st` here, whole, without freeing anything.
    fn retire_strip_slots(&mut self, st: &mut Strip) {
        while let Some(p) = st.plugins.pop() {
            self.retire(p);
        }
        let i = st.instrument.take();
        self.retire(i);
        for d in st.dedicated.iter_mut() {
            let p = d.take();
            self.retire(p);
        }
    }
}

/// Creates (state-restored, prepared) instances of every hosted or test-provider plugin `s`
/// needs — off the audio thread, for [`GraphPrep::build`]. Missing plugins yield nothing (their
/// slots pass audio through as placeholders).
pub fn prepare_instances(s: &Session, sample_rate: f32, max_block: usize) -> Vec<PreparedInstance> {
    instance_specs(s)
        .into_iter()
        .filter_map(|spec| {
            let plugin = create_instance(&spec.id, spec.state.as_deref(), sample_rate, max_block, spec.ch)?;
            Some(PreparedInstance { track: spec.track, slot: spec.slot, id: spec.id, ch: spec.ch, plugin, applied: vec![f32::NAN; spec.params] })
        })
        .collect()
}

/// Renders sessions. Keep one per playback stream; plugin state persists across blocks.
pub struct MixEngine {
    sample_rate: f32,
    max_block: usize,
    /// Hosted plugins the last synchronous sync could not create (track, slot, id): their slots
    /// pass audio through. Empty for a realtime engine (instances arrive prepared).
    pub instantiate_failures: Vec<(TrackId, usize, String)>,
    strips: HashMap<TrackId, Strip>,
    busses: HashMap<BusId, BusBuf>,
    main: Vec<Vec<f32>>,
    main_fmt: ChannelFormat,
    pub meters: HashMap<TrackId, StripMeter>,
    pub main_meter: StripMeter,
    pub pan_law: PanLaw,
    /// Last rendered position end (to detect seeks).
    last_end: Samples,
    /// Snapshot key of the session the strips/order were synced to.
    synced: (usize, usize, usize),
    /// CL05: the caller announces every session change ([`MixEngine::session_changed`]), so
    /// `render` skips the per-block structure fingerprint (measured ≈3 µs per block for 21
    /// tracks — about 12 % of a 64-frame callback's work in the headless profile).
    changes_signalled: bool,
    session_dirty: bool,
    order: Vec<usize>,
    /// Current total compensation delay (samples) of the mix.
    pub latency: usize,
    /// Live input for the next block (planar), set by the audio host for input monitoring.
    pub input: Vec<Vec<f32>>,
    /// Transport stopped: only monitored inputs sound (no clips).
    pub monitor_only: bool,
    /// Recording: record-armed tracks hear their input (auto input).
    pub recording: bool,
    /// Mix the metronome (Options > Click) into the output. Playback turns this on; offline renders
    /// (bounces, stems, exports) leave it off so the click is never printed into a file.
    pub metronome: bool,
    /// Third-party instances come from [`MixEngine::adopt`] instead of being created in `sync`.
    external: bool,
    /// Instances handed over by the host, waiting for `sync` to place them.
    adopt: HashMap<(TrackId, usize), PreparedInstance>,
    /// Instances `sync` took out of service, for the host to drop (see [`MixEngine::take_retired`]).
    retired: Vec<Box<dyn Plugin>>,
    /// Temporary audition bypass (SL05). Set by the realtime host only; offline renders and
    /// exports always use the saved normal state.
    pub audition: AuditionBypass,
    /// Listen-tool route mask (T09). Set by the realtime host only; offline renders and
    /// exports keep the default (inactive) so an audition can never reach a bounce.
    pub listen: soundcraft_model::ListenMask,
    /// Realtime host: honour temporary Tracking deactivations (never set for offline renders).
    pub realtime_overrides: bool,
    /// Total compensation latency changed while recording (CL04 capture-integrity event).
    latency_event: bool,
    rec_latency: Option<usize>,
    /// Mix profile and graph version of the session last rendered (set in `render`).
    profile: MixProfileView,
    graph_version: u32,
    /// f64 main-mix accumulator (graph version 2).
    main_acc: Vec<Vec<f64>>,
    /// Reused per-block strip work list (keeps `render` allocation-free in steady state).
    work: Vec<(usize, Strip)>,
    /// Render independent strips on worker threads (default) or serially on the calling thread
    /// (CL05: benchmark single worker vs additional workers).
    pub parallel_strips: bool,
    /// Count Off (realtime only): the click sounds on beats before this session position even
    /// when the click is off. `Samples::MIN` = no count-off.
    pub click_until: Samples,
}

/// The parts of the session mix profile the router needs (Copy, no allocation per block).
#[derive(Debug, Clone, Copy, Default)]
struct MixProfileView {
    logic: bool,
    law: soundcraft_dsp::mixmath::ProfileLaw,
    stereo_pan: bool,
    compensated: bool,
    /// Building an Independent send matrix (mono pan / balance from the send position).
    independent: bool,
}

impl MixProfileView {
    fn of(s: &Session) -> MixProfileView {
        let p = &s.mix_profile;
        MixProfileView {
            logic: p.kind == ProfileKind::LogicStyle,
            law: soundcraft_dsp::mixmath::ProfileLaw::parse(&p.law).unwrap_or_default(),
            stereo_pan: p.stereo_mode == StereoPanMode::StereoPan,
            compensated: p.balance_compensated,
            independent: false,
        }
    }
}

fn s_graph_v2(_t: &Track, version: u32) -> bool {
    version >= 2
}

/// Mono/stereo → stereo routing matrix for the SML profile (M02): mono Pan, stereo Balance or
/// dual stereo Pan with the profile law. Standard-profile sends (v2) use the same functions with
/// the legacy-equivalent -3 law. A neutral bus (Aux at full-width default positions) is an
/// exact identity (B01). Matrix is `m[src][dst]`.
fn profile_matrix(t: &Track, ns: usize, pans: [f32; 2], p: &MixProfileView, legacy: PanLaw) -> Matrix {
    use soundcraft_dsp::mixmath::{ProfileLaw, balance, mono_pan, stereo_pan};
    let law = if p.logic {
        p.law
    } else {
        match legacy {
            PanLaw::Zero => ProfileLaw::Zero,
            _ => ProfileLaw::Minus3,
        }
    };
    let mut m = [[0.0f32; MAX_CHANNELS]; MAX_CHANNELS];
    if ns == 1 {
        let (l, r) = mono_pan(f64::from(pans[0]), law);
        m[0][0] = l as f32;
        m[0][1] = r as f32;
        return m;
    }
    let neutral = !p.independent && matches!(t.kind, TrackKind::Aux | TrackKind::Folder | TrackKind::Master) && pans == [-1.0, 1.0];
    if neutral {
        m[0][0] = 1.0;
        m[1][1] = 1.0;
        return m;
    }
    if p.stereo_pan && !p.independent {
        let sp = stereo_pan(f64::from(pans[0]), f64::from(pans[1]), law);
        m[0][0] = sp[0][0] as f32;
        m[0][1] = sp[0][1] as f32;
        m[1][0] = sp[1][0] as f32;
        m[1][1] = sp[1][1] as f32;
    } else {
        // Balance position: the send's own value, or the centre of the channel's pan pair.
        let pos = if p.independent { pans[0] } else { (pans[0] + pans[1]) / 2.0 };
        let (gl, gr) = balance(f64::from(pos), law, p.compensated);
        m[0][0] = gl as f32;
        m[1][1] = gr as f32;
    }
    m
}

/// What `sync` needs to fill a slot.
struct SlotCtx<'a> {
    sr: f32,
    mb: usize,
    external: bool,
    adopt: &'a mut HashMap<(TrackId, usize), PreparedInstance>,
    retired: &'a mut Vec<Box<dyn Plugin>>,
    /// Hosted plugins that could not be created in this sync (synchronous path): the slot runs
    /// as a pass-through, which an export must not pass off as the mix (R31).
    failed: &'a mut Vec<(TrackId, usize, String)>,
}

impl SlotCtx<'_> {
    fn retire(&mut self, slot: Option<PluginSlot>) {
        if let Some(s) = slot {
            self.retired.push(s.plugin);
        }
    }

    /// A new instance for `ins` on track `t`, slot `idx`, prepared for `ch` channels.
    fn make(&mut self, t: TrackId, idx: usize, ins: &soundcraft_model::Insert, ch: usize) -> Option<PluginSlot> {
        let third = is_prepared_externally(&ins.plugin);
        let plugin = if third && self.external {
            match self.adopt.remove(&(t, idx)) {
                Some(p) if p.id == ins.plugin && p.ch == ch => Some(p.plugin),
                Some(p) => {
                    self.retired.push(p.plugin);
                    None
                }
                None => None,
            }
        } else {
            let state = if third { ins.state_bytes() } else { None };
            let mut made = create_instance(&ins.plugin, state.as_deref(), self.sr, self.mb, ch);
            if made.is_none() && third {
                self.failed.push((t, idx, ins.plugin.clone()));
            }
            // Synchronous creation = an offline engine (realtime hosts adopt prepared
            // instances): an out-of-process instance waits for its replies here.
            if let Some(p) = made.as_mut() {
                p.set_offline(true);
            }
            made
        }?;
        let mut bypass_delay = DelayLine::default();
        bypass_delay.reserve(plugin.latency(), ch);
        Some(PluginSlot { id: ins.plugin.clone(), ch, plugin, applied: Vec::new(), bypass_delay })
    }
}

/// Runs one insert, feeding its sidechain input from the insert's bus when the plugin has one:
/// the current block of the bus (`current`, aux/master strips) or the previous one.
fn run_insert(
    slot: &mut PluginSlot,
    ins: &soundcraft_model::Insert,
    buf: &mut [Vec<f32>],
    busses: &HashMap<BusId, BusBuf>,
    current: bool,
    frames: usize,
) {
    match ins.sidechain.and_then(|b| busses.get(&b)) {
        Some(bus) if slot.plugin.accepts_sidechain() => {
            let side = if current { &bus.buf } else { &bus.prev };
            slot.plugin.process_sidechain(buf, side, frames);
        }
        _ => slot.plugin.process(buf, frames),
    }
}

/// Busses any insert of `t` uses as a sidechain.
fn sidechain_busses(t: &Track) -> impl Iterator<Item = BusId> + '_ {
    t.mixer.inserts.iter().flatten().filter_map(|i| i.sidechain)
}

impl MixEngine {
    pub fn new(sample_rate: f32, max_block: usize) -> Self {
        let max_block = max_block.clamp(16, 16_384);
        MixEngine {
            sample_rate: if sample_rate.is_finite() && sample_rate > 0.0 { sample_rate } else { 48_000.0 },
            max_block,
            instantiate_failures: Vec::new(),
            strips: HashMap::new(),
            busses: HashMap::new(),
            main: vec![vec![0.0; max_block]; MAIN_CHANNELS],
            main_fmt: ChannelFormat::Stereo,
            meters: HashMap::new(),
            main_meter: StripMeter::default(),
            pan_law: PanLaw::Minus3,
            last_end: 0,
            synced: (0, 0, 0),
            changes_signalled: false,
            session_dirty: true,
            order: Vec::new(),
            latency: 0,
            input: Vec::new(),
            monitor_only: false,
            recording: false,
            metronome: false,
            external: false,
            adopt: HashMap::new(),
            retired: Vec::new(),
            audition: AuditionBypass::default(),
            listen: soundcraft_model::ListenMask::default(),
            realtime_overrides: false,
            latency_event: false,
            rec_latency: None,
            profile: MixProfileView::default(),
            graph_version: 1,
            main_acc: Vec::new(),
            work: Vec::new(),
            parallel_strips: true,
            click_until: Samples::MIN,
        }
    }

    /// Realtime mode: third-party plugins are created by the host and handed over with
    /// [`MixEngine::adopt`] instead of being created in `sync` (see the crate docs).
    pub fn set_external_instances(&mut self, on: bool) {
        self.external = on;
    }

    /// Takes instances created off the audio thread; the next render's `sync` places them.
    /// An earlier instance waiting for the same slot is retired.
    pub fn adopt(&mut self, instances: Vec<PreparedInstance>) {
        for p in instances {
            if let Some(old) = self.adopt.insert((p.track, p.slot), p) {
                self.retired.push(old.plugin);
            }
        }
        // Force a re-sync so waiting slots pick their instance up.
        self.synced = (usize::MAX, usize::MAX, usize::MAX);
    }

    /// Applies a session's prepared graph (B01): moves the strips, bus buffers and meters that
    /// still match from the current maps into the prepared ones, takes fresh ones for new
    /// tracks/busses, places the prepared plugin instances and retires the rest — without
    /// allocating or freeing on the calling (audio) thread. Returns the bundle holding everything
    /// to drop elsewhere. Any allocation that was still needed is counted by
    /// [`prepared_fallbacks`].
    pub fn apply_prepared(&mut self, s: &Session, mut prep: GraphPrep) -> GraphPrep {
        let main_ch = main_channels(s);
        self.main_fmt = s.main_format();
        if self.main.len() != main_ch {
            std::mem::swap(&mut self.main, &mut prep.main);
        }
        // `new` leaves the f64 accumulator for `sync` to size: take the prepared one.
        if self.main_acc.len() != main_ch {
            std::mem::swap(&mut self.main_acc, &mut prep.main_acc);
        }
        // Instances waiting from `adopt` (unused in prepared mode) are dropped off-thread too.
        for (_, p) in self.adopt.drain() {
            push_counted(&mut prep.unplaced, p);
        }
        // Strips: keep those whose channel count still matches, retire the others' plugins.
        let mut old = std::mem::replace(&mut self.strips, std::mem::take(&mut prep.strips));
        for (id, mut st) in old.drain() {
            match s.track(id) {
                Some(t) if st.buf.len() == strip_channels(t, main_ch) => {
                    self.strips.insert(id, st);
                }
                _ => {
                    prep.retire_strip_slots(&mut st);
                    push_counted(&mut prep.orphans, st);
                }
            }
        }
        prep.strips = old;
        while let Some((id, st)) = prep.fresh.pop() {
            match self.strips.entry(id) {
                std::collections::hash_map::Entry::Occupied(_) => push_counted(&mut prep.orphans, st),
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(st);
                }
            }
        }
        for t in &s.tracks {
            let ch = strip_channels(t, main_ch);
            let Some(strip) = self.strips.get_mut(&t.id) else { continue };
            while strip.plugins.len() > t.mixer.inserts.len() {
                let p = strip.plugins.pop().flatten();
                prep.retire(p);
            }
            if strip.plugins.capacity() < t.mixer.inserts.len() {
                note_fallback();
            }
            strip.plugins.resize_with(t.mixer.inserts.len(), || None);
            for (idx, (slot, ins)) in strip.plugins.iter_mut().zip(t.mixer.inserts.iter()).enumerate() {
                match ins {
                    Some(i) if slot.as_ref().is_none_or(|p| p.id != i.plugin || p.ch != ch) => {
                        let old = slot.take();
                        prep.retire(old);
                        *slot = prep.take_instance(t.id, idx, &i.plugin, ch);
                    }
                    None => {
                        let old = slot.take();
                        prep.retire(old);
                    }
                    _ => {}
                }
            }
            let want = t.instrument.as_ref();
            let ich = ch.max(2);
            if strip.instrument.as_ref().map(|p| (p.id.as_str(), p.ch)) != want.map(|i| (i.plugin.as_str(), ich)) {
                let old = strip.instrument.take();
                prep.retire(old);
                strip.instrument = want.and_then(|i| prep.take_instance(t.id, INSTRUMENT_SLOT, &i.plugin, ich));
            }
            for (k, inst) in SlotKind::ALL.iter().zip(strip.dedicated.iter_mut()) {
                let want = t.mixer.slots.get(*k).and_then(|x| x.provider.as_ref());
                if inst.as_ref().map(|p| (p.id.as_str(), p.ch)) != want.map(|i| (i.plugin.as_str(), ch)) {
                    let old = inst.take();
                    prep.retire(old);
                    *inst = want.and_then(|i| prep.take_instance(t.id, k.instance_index(), &i.plugin, ch));
                }
            }
        }
        // Busses.
        let mut old_b = std::mem::replace(&mut self.busses, std::mem::take(&mut prep.busses));
        for (id, b) in old_b.drain() {
            match s.bus(id) {
                Some(bus) if b.buf.len() == bus.format.channels().clamp(1, MAX_CHANNELS) => {
                    self.busses.insert(id, b);
                }
                _ => push_counted(&mut prep.orphan_busses, b),
            }
        }
        prep.busses = old_b;
        while let Some((id, mut b)) = prep.fresh_busses.pop() {
            if let Some(cur) = self.busses.get_mut(&id) {
                cur.fmt = b.fmt;
                push_counted(&mut prep.orphan_busses, b);
            } else {
                if let Some(bus) = s.bus(id) {
                    b.fmt = bus.format;
                }
                self.busses.insert(id, b);
            }
        }
        // Meters: keep existing values, pre-place one per track.
        let mut old_m = std::mem::replace(&mut self.meters, std::mem::take(&mut prep.meters));
        for (id, m) in old_m.drain() {
            if s.track(id).is_some() {
                self.meters.insert(id, m);
            } else {
                push_counted(&mut prep.orphan_meters, m);
            }
        }
        prep.meters = old_m;
        while let Some((id, m)) = prep.fresh_meters.pop() {
            match self.meters.entry(id) {
                std::collections::hash_map::Entry::Occupied(_) => push_counted(&mut prep.orphan_meters, m),
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(m);
                }
            }
        }
        // Order and the reusable work list.
        prep.old_order = std::mem::replace(&mut self.order, std::mem::take(&mut prep.order));
        if prep.work.capacity() > self.work.capacity() {
            prep.old_work = std::mem::replace(&mut self.work, std::mem::take(&mut prep.work));
        }
        self.synced = (structure_fingerprint(s), s.tracks.len(), s.busses.len());
        self.session_dirty = false;
        prep
    }

    /// True once if the total compensation latency changed while recording (then cleared).
    pub fn take_latency_event(&mut self) -> bool {
        std::mem::take(&mut self.latency_event)
    }

    /// Instances taken out of service since the last call (drop them off the audio thread).
    /// Does not allocate.
    pub fn take_retired(&mut self) -> Vec<Box<dyn Plugin>> {
        std::mem::take(&mut self.retired)
    }

    /// Every instance this engine holds (strips, instruments, clip effects excluded) plus the
    /// retired and waiting ones, leaving the engine empty: hand them to another thread to drop
    /// before replacing the engine.
    pub fn drain_plugins(&mut self) -> Vec<Box<dyn Plugin>> {
        let mut out = std::mem::take(&mut self.retired);
        for st in self.strips.values_mut() {
            out.extend(st.plugins.iter_mut().filter_map(Option::take).map(|s| s.plugin));
            out.extend(st.instrument.take().map(|s| s.plugin));
            out.extend(st.dedicated.iter_mut().filter_map(Option::take).map(|s| s.plugin));
        }
        out.extend(self.adopt.drain().map(|(_, p)| p.plugin));
        self.synced = (usize::MAX, usize::MAX, usize::MAX);
        out
    }

    /// The state of every live plugin that has one (third-party plugins), as
    /// `(track, slot or INSTRUMENT_SLOT, state)`. Calls into the plugins and allocates.
    pub fn capture_states(&mut self) -> Vec<(TrackId, usize, Vec<u8>)> {
        let mut out = Vec::new();
        for (tid, st) in self.strips.iter_mut() {
            for (i, slot) in st.plugins.iter_mut().enumerate() {
                if let Some(data) = slot.as_mut().and_then(|s| s.plugin.save_state()) {
                    out.push((*tid, i, data));
                }
            }
            if let Some(data) = st.instrument.as_mut().and_then(|s| s.plugin.save_state()) {
                out.push((*tid, INSTRUMENT_SLOT, data));
            }
            for (k, slot) in SlotKind::ALL.iter().zip(st.dedicated.iter_mut()) {
                if let Some(data) = slot.as_mut().and_then(|s| s.plugin.save_state()) {
                    out.push((*tid, k.instance_index(), data));
                }
            }
        }
        out.sort_by_key(|(t, s, _)| (*t, *s));
        out
    }

    pub fn max_block(&self) -> usize {
        self.max_block
    }

    /// Format of the main mix as of the last sync.
    pub fn main_format(&self) -> ChannelFormat {
        self.main_fmt
    }

    /// Channels of the main mix as of the last sync.
    pub fn main_channels(&self) -> usize {
        self.main.len()
    }

    /// Reset all plugin state (on stop / seek).
    pub fn reset(&mut self) {
        for s in self.strips.values_mut() {
            for p in s.plugins.iter_mut().flatten() {
                p.plugin.reset();
            }
            if let Some(i) = &mut s.instrument {
                i.plugin.all_notes_off();
                i.plugin.reset();
            }
            s.held.clear();
        }
        self.meters.clear();
        self.main_meter = StripMeter::default();
    }

    /// Bring plugin instances in line with the session (creates/destroys as needed). Allocates;
    /// call outside the realtime thread when the document changes, or let `render` do it.
    pub fn sync(&mut self, s: &Session) {
        let mb = self.max_block;
        let main_ch = main_channels(s);
        self.main_fmt = s.main_format();
        if self.main.len() != main_ch {
            self.main = vec![vec![0.0; mb]; main_ch];
        }
        if self.main_acc.len() != main_ch {
            self.main_acc = vec![vec![0.0; mb]; main_ch];
        }
        self.instantiate_failures.clear();
        let mut cx = SlotCtx {
            sr: self.sample_rate,
            mb,
            external: self.external,
            adopt: &mut self.adopt,
            retired: &mut self.retired,
            failed: &mut self.instantiate_failures,
        };
        let gone: Vec<TrackId> = self.strips.keys().filter(|id| s.track(**id).is_none()).copied().collect();
        for id in gone {
            if let Some(mut st) = self.strips.remove(&id) {
                for p in st.plugins.drain(..) {
                    cx.retire(p);
                }
                cx.retire(st.instrument.take());
                for d in st.dedicated.iter_mut() {
                    cx.retire(d.take());
                }
            }
        }
        for t in &s.tracks {
            let ch = strip_channels(t, main_ch);
            let strip = self.strips.entry(t.id).or_insert_with(Strip::new);
            if strip.buf.len() != ch || strip.buf.first().map_or(0, Vec::len) != mb {
                strip.buf = vec![vec![0.0; mb]; ch];
            }
            while strip.plugins.len() > t.mixer.inserts.len() {
                let p = strip.plugins.pop().flatten();
                cx.retire(p);
            }
            strip.plugins.resize_with(t.mixer.inserts.len(), || None);
            for (idx, (slot, ins)) in strip.plugins.iter_mut().zip(t.mixer.inserts.iter()).enumerate() {
                match ins {
                    Some(i) if slot.as_ref().is_none_or(|p| p.id != i.plugin || p.ch != ch) => {
                        let old = slot.take();
                        cx.retire(old);
                        *slot = cx.make(t.id, idx, i, ch);
                    }
                    None => {
                        let old = slot.take();
                        cx.retire(old);
                    }
                    _ => {}
                }
            }
            let want = t.instrument.as_ref();
            let ich = ch.max(2);
            if strip.instrument.as_ref().map(|p| (p.id.as_str(), p.ch)) != want.map(|i| (i.plugin.as_str(), ich)) {
                let old = strip.instrument.take();
                cx.retire(old);
                strip.instrument = want.and_then(|i| cx.make(t.id, INSTRUMENT_SLOT, i, ich));
            }
            for (k, inst) in SlotKind::ALL.iter().zip(strip.dedicated.iter_mut()) {
                let want = t.mixer.slots.get(*k).and_then(|x| x.provider.as_ref());
                if inst.as_ref().map(|p| (p.id.as_str(), p.ch)) != want.map(|i| (i.plugin.as_str(), ch)) {
                    let old = inst.take();
                    cx.retire(old);
                    *inst = want.and_then(|i| cx.make(t.id, k.instance_index(), i, ch));
                }
            }
        }
        // Instances nobody claimed (the session changed again before they arrived).
        let leftovers: Vec<(TrackId, usize)> = cx.adopt.keys().copied().collect();
        for k in leftovers {
            if let Some(p) = cx.adopt.remove(&k) {
                cx.retired.push(p.plugin);
            }
        }
        if !self.external {
            self.retired.clear();
        }
        for b in &s.busses {
            let n = b.format.channels().clamp(1, MAX_CHANNELS);
            let bus = self.busses.entry(b.id).or_insert_with(|| BusBuf { fmt: b.format, buf: Vec::new(), acc: Vec::new(), prev: Vec::new() });
            bus.fmt = b.format;
            if bus.buf.len() != n {
                bus.buf = vec![vec![0.0; mb]; n];
            }
            if bus.acc.len() != n {
                bus.acc = vec![vec![0.0; mb]; n];
            }
            if bus.prev.len() != n {
                bus.prev = vec![vec![0.0; mb]; n];
            }
        }
        self.busses.retain(|id, _| s.bus(*id).is_some());
    }

    /// Re-syncs plugin instances and the routing order when the session's structure changed
    /// since the last sync (or instances were adopted). `render` calls it; a realtime host may
    /// also call it right after swapping sessions so slots are filled before playback starts.
    /// Opt in to announcing session changes: afterwards `render` checks the graph structure
    /// only after [`MixEngine::session_changed`] (or [`MixEngine::ensure_synced`]) instead of
    /// hashing it every block. A caller that passes a different session without announcing it
    /// renders the previous structure — so only owners of the session hand-off (the playback
    /// engine) enable this; offline renders keep the per-block check.
    pub fn set_session_changes_signalled(&mut self, on: bool) {
        self.changes_signalled = on;
        self.session_dirty = true;
    }

    /// The next `render` receives a different session: re-check the graph structure once.
    pub fn session_changed(&mut self) {
        self.session_dirty = true;
    }

    pub fn ensure_synced(&mut self, s: &Session) {
        self.session_dirty = false;
        let key = (structure_fingerprint(s), s.tracks.len(), s.busses.len());
        if key != self.synced {
            self.sync(s);
            self.order = processing_order(s);
            self.synced = key;
        }
    }

    /// Render `frames` (≤ max_block) samples starting at `pos` into `out` (≥ 2 channels).
    pub fn render(&mut self, s: &Session, pos: Samples, frames: usize, out: &mut [Vec<f32>]) {
        let frames = frames.min(self.max_block);
        if pos != self.last_end {
            // Seek: silence held notes and ringing plugins.
            for st in self.strips.values_mut() {
                if let Some(i) = &mut st.instrument {
                    i.plugin.all_notes_off();
                }
                st.held.clear();
            }
        }
        self.last_end = pos.saturating_add(frames as i64);
        if !self.changes_signalled || self.session_dirty {
            self.ensure_synced(s);
        }
        self.profile = MixProfileView::of(s);
        self.graph_version = s.mix_profile.graph_version;
        for b in self.busses.values_mut() {
            for c in b.buf.iter_mut() {
                c.iter_mut().take(frames).for_each(|x| *x = 0.0);
            }
        }
        for c in &mut self.main {
            c.iter_mut().take(frames).for_each(|x| *x = 0.0);
        }
        for c in self.main_acc.iter_mut().chain(self.busses.values_mut().flat_map(|b| b.acc.iter_mut())) {
            c.iter_mut().take(frames).for_each(|x| *x = 0.0);
        }
        let any_solo = s.tracks.iter().any(|t| t.mixer.solo && !t.inactive);
        let any_sidechain = s.tracks.iter().any(|t| sidechain_busses(t).next().is_some());
        let order = std::mem::take(&mut self.order);
        let live = |t: &&Track| !(t.kind == TrackKind::Master || t.inactive || t.kind == TrackKind::Vca);
        let is_aux = |t: &Track| matches!(t.kind, TrackKind::Aux | TrackKind::Folder);
        // Phase 1: independent strips (audio, instrument, MIDI) in parallel.
        // Reused buffer (no per-block allocation once warmed up): (track index, strip).
        let mut work = std::mem::take(&mut self.work);
        work.clear();
        for &i in &order {
            if let Some(t) = s.tracks.get(i).filter(live).filter(|t| !is_aux(t))
                && let Some(st) = self.strips.remove(&t.id)
            {
                work.push((i, st));
            }
        }
        for (_, st) in work.iter_mut() {
            if st.scratch.len() < self.max_block {
                st.scratch.resize(self.max_block, 0.0);
            }
        }
        let busses = std::mem::take(&mut self.busses);
        let live_in = LiveInput {
            input: &self.input,
            monitor_only: self.monitor_only,
            recording: self.recording,
            audition: self.audition,
            listen: self.listen,
            tracking: self.realtime_overrides,
            sidechain_current: false,
        };
        let strip_fn = |(i, st): &mut (usize, Strip)| {
            if let Some(t) = s.tracks.get(*i) {
                process_strip(s, t, st, &busses, &live_in, pos, frames, any_solo);
            }
        };
        // CL05: worker threads only pay off for strips with hosted processing; light strips
        // (clip playback, gain, pan) render faster serially (measured: docs/evidence/sprint_04).
        let heavy = work
            .iter()
            .filter(|(_, st)| st.plugins.iter().any(Option::is_some) || st.dedicated.iter().any(Option::is_some) || st.instrument.is_some())
            .count();
        if self.parallel_strips && heavy >= PARALLEL_MIN_HEAVY_STRIPS {
            run_strips(&mut work, strip_fn);
        } else {
            work.iter_mut().for_each(strip_fn);
        }
        self.busses = busses;
        // Delay compensation: align every strip to the slowest; direct-to-main outputs also wait
        // for the slowest aux return (aux latencies are from the previous block; they are stable).
        let pdc = s.edit.delay_compensation;
        let lmax_t = if pdc { work.iter().map(|(_, st)| st.latency).max().unwrap_or(0) } else { 0 };
        let lmax_a =
            if pdc { self.strips.iter().filter(|(id, _)| s.track(**id).is_some_and(is_aux)).map(|(_, st)| st.latency).max().unwrap_or(0) } else { 0 };
        let total = lmax_t + lmax_a;
        // Compare against the latency the take started with (not the first block's change).
        match (self.recording, self.rec_latency) {
            (true, None) => self.rec_latency = Some(total),
            (true, Some(l)) if l != total => {
                self.latency_event = true;
                self.rec_latency = Some(total);
            }
            (false, _) => self.rec_latency = None,
            _ => {}
        }
        self.latency = total;
        for (i, st) in work.iter_mut() {
            let Some(t) = s.tracks.get(*i) else { continue };
            let ch = st.buf.len();
            st.align.set(lmax_t.saturating_sub(st.latency), ch);
            st.align.process(&mut st.buf, frames);
            if t.mixer.sends.iter().flatten().any(|x| x.pre_fader) {
                st.align_pre.set(lmax_t.saturating_sub(st.latency), ch);
                st.align_pre.process(&mut st.pre, frames);
            }
            st.out_extra.set(if t.mixer.output == Route::Main { lmax_a } else { 0 }, ch);
        }
        for (i, st) in work.iter_mut() {
            if let Some(t) = s.tracks.get(*i) {
                self.route_strip(t, st, pos, frames);
            }
        }
        for (i, st) in work.drain(..) {
            if let Some(t) = s.tracks.get(i) {
                self.strips.insert(t.id, st);
            }
        }
        self.work = work;
        let is_aux = |t: &Track| matches!(t.kind, TrackKind::Aux | TrackKind::Folder);
        // Phase 2: auxes in dependency order (they read busses).
        let v2 = self.graph_version >= 2;
        for t in order.iter().filter_map(|&i| s.tracks.get(i)).filter(live).filter(|t| is_aux(t)) {
            if v2
                && let Route::Bus(b) = &t.mixer.input
                && let Some(bus) = self.busses.get_mut(b)
            {
                bus.flush(frames);
            }
            let Some(mut st) = self.strips.remove(&t.id) else { continue };
            if st.scratch.len() < self.max_block {
                st.scratch.resize(self.max_block, 0.0);
            }
            // Sidechain busses this aux reads must be complete (v2 sums in f64 until flushed).
            if v2 {
                for b in sidechain_busses(t) {
                    if let Some(bus) = self.busses.get_mut(&b) {
                        bus.flush(frames);
                    }
                }
            }
            let live_in = LiveInput {
                input: &self.input,
                monitor_only: self.monitor_only,
                recording: self.recording,
                audition: self.audition,
                listen: self.listen,
                tracking: self.realtime_overrides,
                sidechain_current: true,
            };
            process_strip(s, t, &mut st, &self.busses, &live_in, pos, frames, any_solo);
            let ch = st.buf.len();
            st.align.set(if pdc { lmax_a.saturating_sub(st.latency) } else { 0 }, ch);
            st.align.process(&mut st.buf, frames);
            st.out_extra.set(0, ch);
            self.route_strip(t, &mut st, pos, frames);
            self.strips.insert(t.id, st);
        }
        self.order = order;
        if v2 {
            for (m, a) in self.main.iter_mut().zip(self.main_acc.iter()) {
                for (x, y) in m.iter_mut().zip(a.iter()).take(frames) {
                    *x = *y as f32;
                }
            }
        }
        // Master faders process the main mix.
        for t in s.tracks.iter().filter(|t| t.kind == TrackKind::Master && !t.inactive) {
            if let Some(strip) = self.strips.get_mut(&t.id) {
                for (dst, src) in strip.buf.iter_mut().zip(self.main.iter()) {
                    if let (Some(d), Some(sr)) = (dst.get_mut(..frames), src.get(..frames)) {
                        d.copy_from_slice(sr);
                    }
                }
                let audition = self.audition;
                // SL02 Main: summed mix -> Summing slot -> Master Tape (Pre) -> inserts -> fader
                // -> Master Tape (Post). Main is a stereo identity (no extra pan stage).
                {
                    let [_, sum_i, mt_i] = &mut strip.dedicated;
                    let [_, sum_d, mt_d] = &mut strip.slot_delay;
                    let [_, sum_io, mt_io] = &mut strip.slot_io;
                    run_dedicated(t.mixer.slots.summing.as_ref(), sum_i, sum_d, sum_io, &mut strip.buf, frames, audition.summing, false);
                    if t.mixer.slots.master_tape_position == TapePosition::Pre {
                        run_dedicated(t.mixer.slots.master_tape.as_ref(), mt_i, mt_d, mt_io, &mut strip.buf, frames, audition.tape, false);
                    }
                }
                if v2 {
                    for b in sidechain_busses(t) {
                        if let Some(bus) = self.busses.get_mut(&b) {
                            bus.flush(frames);
                        }
                    }
                }
                for (i, ins) in t.mixer.inserts.iter().enumerate() {
                    if let (Some(ins), Some(Some(slot))) = (ins, strip.plugins.get_mut(i))
                        && ins.active
                        && !audition.inserts
                    {
                        if ins.bypass {
                            let lat = slot.plugin.latency();
                            slot.bypass_delay.set(lat, strip.buf.len());
                            slot.bypass_delay.process(&mut strip.buf, frames);
                            continue;
                        }
                        apply_params(slot, ins, t, i, pos);
                        run_insert(slot, ins, &mut strip.buf, &self.busses, true, frames);
                    }
                }
                let v0 = volume_db_at(t, pos);
                let v1 = volume_db_at(t, pos + frames as i64);
                for c in strip.buf.iter_mut() {
                    for i in 0..frames {
                        if let Some(x) = c.get_mut(i) {
                            *x *= lerp_gain(v0, v1, i, frames);
                        }
                    }
                }
                if t.mixer.slots.master_tape_position == TapePosition::Post {
                    let [_, _, mt_i] = &mut strip.dedicated;
                    let [_, _, mt_d] = &mut strip.slot_delay;
                    let [_, _, mt_io] = &mut strip.slot_io;
                    run_dedicated(t.mixer.slots.master_tape.as_ref(), mt_i, mt_d, mt_io, &mut strip.buf, frames, audition.tape, false);
                }
                for (ch, src) in strip.buf.iter().enumerate().take(self.main.len()) {
                    if let Some(m) = self.main.get_mut(ch) {
                        for i in 0..frames {
                            if let (Some(d), Some(x)) = (m.get_mut(i), src.get(i)) {
                                *d = *x;
                            }
                        }
                    }
                }
                let m = self.meters.entry(t.id).or_default();
                m.measure(&self.main, frames, 0.0);
                m.slots = strip.slot_io;
            }
        }
        // Metronome: summed after the master fader, so it reaches the speakers without passing
        // through any track, send or insert. Only the playback engine sets `metronome`.
        // Not while monitoring only (transport stopped): the position does not advance there, so the
        // start of the same beat would repeat every block as a buzz.
        if self.metronome && !self.monitor_only {
            if s.edit.click && (!s.edit.flag("click.only_during_record") || self.recording) {
                click::render(s, pos, frames, &mut self.main, Samples::MAX);
            } else if pos < self.click_until {
                // Count Off: the click sounds before the record point even when it is off.
                click::render(s, pos, frames, &mut self.main, self.click_until);
            }
        }
        // Output: min(out.len(), main channels) channels; any further output channels are silent.
        for (c, o) in out.iter_mut().enumerate() {
            let src = self.main.get(c);
            for i in 0..frames {
                let v = src.and_then(|m| m.get(i)).copied().unwrap_or(0.0);
                if let Some(d) = o.get_mut(i) {
                    *d = if v.is_finite() { v } else { 0.0 };
                }
            }
        }
        self.main_meter.measure(&self.main, frames, 0.0);
        // Sidechain sources for the next block's audio/instrument strips (one block late).
        if any_sidechain {
            for b in self.busses.values_mut() {
                if v2 {
                    b.flush(frames);
                }
                for (p, c) in b.prev.iter_mut().zip(b.buf.iter()) {
                    if let (Some(d), Some(sr)) = (p.get_mut(..frames), c.get(..frames)) {
                        d.copy_from_slice(sr);
                    }
                }
            }
        }
    }

    /// Send, meter and pan a processed strip into the busses / main mix.
    fn route_strip(&mut self, t: &Track, strip: &mut Strip, pos: Samples, frames: usize) {
        let m = self.meters.entry(t.id).or_default();
        m.measure(&strip.buf, frames, strip.gr);
        m.slots = strip.slot_io;
        let src_fmt = strip_format(t, strip.buf.len());
        let law = self.pan_law;
        let v2 = s_graph_v2(t, self.graph_version);
        let pans = [pan_at(t, 0, pos), pan_at(t, 1, pos)];
        for (i, snd) in t.mixer.sends.iter().enumerate() {
            let Some(snd) = snd else { continue };
            // v2 policy: solo exclusion gates every send; channel mute gates post-fader sends
            // only (pre-fader sends follow their own send mute). Legacy: mute gates all.
            let gated = if v2 { strip.soloed_out || (!snd.pre_fader && strip.muted) } else { strip.muted };
            if snd.mute || gated {
                continue;
            }
            let lvl = send_level(t, i, snd.level_db, pos);
            let gain = db_to_gain(lvl);
            let src = if snd.pre_fader { &strip.pre } else { &strip.buf };
            let Route::Bus(b) = &snd.target else { continue };
            let Some(dst) = self.busses.get_mut(b) else { continue };
            if gain <= 0.0 {
                continue;
            }
            let m = if v2 && src.len() <= 2 && dst.buf.len() == 2 && snd.surround.is_none() {
                // SL06: exactly one panner per route. Follow = the channel's own panner
                // coefficients; Independent = the send's saved position (mono pan / balance).
                if snd.follow_main_pan {
                    // Same coefficients as the channel's main-output route.
                    if self.profile.logic {
                        profile_matrix(t, src.len(), pans, &self.profile, law)
                    } else {
                        build_matrix(src_fmt, src.len(), dst.fmt, dst.buf.len(), &PanSpec { stereo: pans, surround: t.mixer.surround.as_ref() }, law)
                    }
                } else {
                    profile_matrix(t, src.len(), [snd.pan, snd.pan], &MixProfileView { independent: true, ..self.profile }, law)
                }
            } else {
                // Mono sends use the send's own pan; stereo sends go straight to L/R.
                let stereo = if src.len() == 1 { [snd.pan, 0.0] } else { [-1.0, 1.0] };
                let surround = snd.surround.as_ref().or(if snd.follow_main_pan { t.mixer.surround.as_ref() } else { None });
                build_matrix(src_fmt, src.len(), dst.fmt, dst.buf.len(), &PanSpec { stereo, surround }, law)
            };
            if v2 {
                apply_matrix_acc(src, frames, &mut dst.acc, &m, gain);
            } else {
                apply_matrix(src, frames, &mut dst.buf, &m, gain);
            }
        }
        let spec = PanSpec { stereo: [pan_at(t, 0, pos), pan_at(t, 1, pos)], surround: t.mixer.surround.as_ref() };
        let profile = self.profile;
        let route_matrix = |src_n: usize, dst_fmt: ChannelFormat, dst_n: usize| -> Matrix {
            if profile.logic && src_n <= 2 && dst_n == 2 && t.mixer.surround.is_none() {
                profile_matrix(t, src_n, pans, &profile, law)
            } else {
                build_matrix(src_fmt, src_n, dst_fmt, dst_n, &spec, law)
            }
        };
        match &t.mixer.output {
            Route::Main if strip.out_extra.len > 0 => {
                if strip.outbuf.len() != strip.buf.len() {
                    strip.outbuf = vec![Vec::new(); strip.buf.len()];
                }
                for (d, src) in strip.outbuf.iter_mut().zip(strip.buf.iter()) {
                    d.clear();
                    d.extend_from_slice(src.get(..frames).unwrap_or(&[]));
                }
                strip.out_extra.process(&mut strip.outbuf, frames);
                let m = route_matrix(strip.outbuf.len(), self.main_fmt, self.main.len());
                if v2 {
                    apply_matrix_acc(&strip.outbuf, frames, &mut self.main_acc, &m, 1.0);
                } else {
                    apply_matrix(&strip.outbuf, frames, &mut self.main, &m, 1.0);
                }
            }
            Route::Main => {
                let m = route_matrix(strip.buf.len(), self.main_fmt, self.main.len());
                if v2 {
                    apply_matrix_acc(&strip.buf, frames, &mut self.main_acc, &m, 1.0);
                } else {
                    apply_matrix(&strip.buf, frames, &mut self.main, &m, 1.0);
                }
            }
            Route::Bus(b) => {
                if let Some(dst) = self.busses.get_mut(b) {
                    let m = route_matrix(strip.buf.len(), dst.fmt, dst.buf.len());
                    if v2 {
                        apply_matrix_acc(&strip.buf, frames, &mut dst.acc, &m, 1.0);
                    } else {
                        apply_matrix(&strip.buf, frames, &mut dst.buf, &m, 1.0);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Source → trim → inserts → (pre-fader copy) → mute/fader for one strip. Independent of other
/// strips except that aux inputs read `busses`, so non-aux strips can run in parallel.
/// Live input routed to monitored tracks.
struct LiveInput<'a> {
    input: &'a [Vec<f32>],
    monitor_only: bool,
    recording: bool,
    /// Temporary audition bypass (realtime only; offline renders/exports pass the default).
    audition: AuditionBypass,
    /// Listen-tool mask (realtime only).
    listen: soundcraft_model::ListenMask,
    /// Honour the session's temporary Tracking deactivations (realtime host only).
    tracking: bool,
    /// Sidechain busses are complete for this block (aux/master pass); otherwise strips hear the
    /// previous block.
    sidechain_current: bool,
}

fn process_strip(
    s: &Session,
    t: &Track,
    strip: &mut Strip,
    busses: &HashMap<BusId, BusBuf>,
    live: &LiveInput<'_>,
    pos: Samples,
    frames: usize,
    any_solo: bool,
) {
    for c in strip.buf.iter_mut() {
        c.iter_mut().take(frames).for_each(|x| *x = 0.0);
    }
    match t.kind {
        TrackKind::Audio
            if !live.input.is_empty()
                && !hardware_monitoring(s)
                && (t.mixer.input_monitor || (t.mixer.record_arm && (live.monitor_only || live.recording))) =>
        {
            // Input monitoring: the track hears its hardware input instead of its clips.
            let first = match &t.mixer.input {
                Route::Hardware(h) => {
                    h.trim_start_matches("In ").split(['-', ' ']).next().and_then(|n| n.parse::<usize>().ok()).map_or(0, |n| n.saturating_sub(1))
                }
                _ => 0,
            };
            // A channel the device does not have is silent — never another physical input; a
            // one-input device feeds a stereo track as dual mono (same rule as recording).
            let dual_mono = live.input.len() == 1 && first == 0;
            for (k, dst) in strip.buf.iter_mut().enumerate() {
                let ch = if dual_mono { 0 } else { first + k };
                if let Some(src) = live.input.get(ch) {
                    for (d, x) in dst.iter_mut().zip(src.iter()).take(frames) {
                        *d = *x;
                    }
                }
            }
        }
        TrackKind::Audio if live.monitor_only => {}
        TrackKind::Instrument | TrackKind::Midi if live.monitor_only => {}
        TrackKind::Audio => {
            let block = Range { start: pos, end: pos.saturating_add(frames as i64) };
            for clip in t.clips() {
                if clip.muted || !clip.is_audio() || !clip.range().overlaps(&block) {
                    continue;
                }
                match clip_fx_params(s, clip.id.0) {
                    None => render_clip(s, clip, pos, frames, &mut strip.buf, &mut strip.scratch),
                    Some(params) => {
                        let nch = strip.buf.len();
                        if strip.clipbuf.len() != nch || strip.clipbuf.first().map_or(0, Vec::len) < frames {
                            strip.clipbuf = vec![vec![0.0; frames.max(strip.scratch.len())]; nch];
                        }
                        for c in strip.clipbuf.iter_mut() {
                            c.iter_mut().take(frames).for_each(|x| *x = 0.0);
                        }
                        render_clip(s, clip, pos, frames, &mut strip.clipbuf, &mut strip.scratch);
                        let fx = strip.clip_fx.entry(clip.id.0).or_insert_with(|| ClipFx::new(s.sample_rate.as_f64() as f32, nch));
                        fx.configure(&params);
                        fx.process(&mut strip.clipbuf, frames);
                        for (d, c) in strip.buf.iter_mut().zip(strip.clipbuf.iter()) {
                            for (x, y) in d.iter_mut().zip(c.iter()).take(frames) {
                                *x += *y;
                            }
                        }
                    }
                }
            }
        }
        TrackKind::Aux | TrackKind::Folder => {
            if let Route::Bus(b) = &t.mixer.input
                && let Some(src) = busses.get(b)
            {
                let (ns, nd) = (src.buf.len(), strip.buf.len());
                let dst_fmt = strip_format(t, nd);
                let m = if src.fmt == dst_fmt && ns == nd {
                    identity_matrix(ns)
                } else if ns == 1 && nd == 2 {
                    // A mono bus feeds both sides of a stereo aux.
                    let mut m = [[0.0; MAX_CHANNELS]; MAX_CHANNELS];
                    m[0][0] = 1.0;
                    m[0][1] = 1.0;
                    m
                } else {
                    let stereo = if ns == 1 { [0.0, 0.0] } else { [-1.0, 1.0] };
                    build_matrix(src.fmt, ns, dst_fmt, nd, &PanSpec { stereo, surround: None }, PanLaw::Minus3)
                };
                apply_matrix(&src.buf, frames, &mut strip.buf, &m, 1.0);
            }
        }
        TrackKind::Instrument | TrackKind::Midi => {
            if let Some(inst) = &mut strip.instrument {
                schedule_notes(s, t, pos, frames, inst, &mut strip.held);
                if strip.buf.len() >= 2 {
                    inst.plugin.process(&mut strip.buf, frames);
                } else if let Some(c0) = strip.buf.first_mut() {
                    // Mono MIDI track: render the instrument in stereo and fold.
                    let mut st = [std::mem::take(c0), std::mem::take(&mut strip.scratch)];
                    inst.plugin.process(&mut st, frames);
                    let [a, b] = st;
                    *c0 = a;
                    strip.scratch = b;
                    for (x, y) in c0.iter_mut().zip(strip.scratch.iter()).take(frames) {
                        *x = 0.5 * (*x + *y);
                    }
                }
            }
        }
        _ => {}
    }
    // SL02 bus/Main: incoming sum -> Summing provider (+ slot trim) -> utility trim/polarity.
    let is_bus = matches!(t.kind, TrackKind::Aux | TrackKind::Folder);
    let mut latency = 0usize;
    {
        let [tape_i, sum_i, mt_i] = &mut strip.dedicated;
        let [tape_d, sum_d, mt_d] = &mut strip.slot_delay;
        let [tape_io, sum_io, mt_io] = &mut strip.slot_io;
        if is_bus {
            latency += run_dedicated(
                t.mixer.slots.summing.as_ref(),
                sum_i,
                sum_d,
                sum_io,
                &mut strip.buf,
                frames,
                live.audition.summing,
                live.tracking && s.tracking_off.contains(&(t.id, SlotKind::Summing.instance_index())),
            );
        }
        let trim = db_to_gain(t.mixer.trim_db) * if t.mixer.phase_invert { -1.0 } else { 1.0 };
        if (trim - 1.0).abs() > f32::EPSILON {
            for c in strip.buf.iter_mut() {
                c.iter_mut().take(frames).for_each(|x| *x *= trim);
            }
        }
        // Track Tape (before user inserts) / bus Master Tape when Pre.
        if is_bus {
            if t.mixer.slots.master_tape_position == TapePosition::Pre {
                latency += run_dedicated(
                    t.mixer.slots.master_tape.as_ref(),
                    mt_i,
                    mt_d,
                    mt_io,
                    &mut strip.buf,
                    frames,
                    live.audition.tape,
                    live.tracking && s.tracking_off.contains(&(t.id, SlotKind::MasterTape.instance_index())),
                );
            }
        } else {
            latency += run_dedicated(
                t.mixer.slots.tape.as_ref(),
                tape_i,
                tape_d,
                tape_io,
                &mut strip.buf,
                frames,
                live.audition.tape,
                live.tracking && s.tracking_off.contains(&(t.id, SlotKind::Tape.instance_index())),
            );
        }
    }
    let mut gr = 0.0f32;
    let v2_graph = s.mix_profile.graph_version >= 2;
    for (i, ins) in t.mixer.inserts.iter().enumerate() {
        let (Some(ins), Some(Some(slot))) = (ins, strip.plugins.get_mut(i)) else { continue };
        if !ins.active || (v2_graph && ins.post_fader) || (live.tracking && s.tracking_off.contains(&(t.id, i))) {
            continue;
        }
        let lat = slot.plugin.latency();
        latency = latency.saturating_add(lat);
        if live.audition.inserts {
            // Audition bypass keeps compensation: the insert's latency stays as a plain delay.
            continue;
        }
        if ins.bypass {
            // Ordinary bypass keeps compensation too: the plugin's latency stays as a plain delay.
            slot.bypass_delay.set(lat, strip.buf.len());
            slot.bypass_delay.process(&mut strip.buf, frames);
            continue;
        }
        apply_params(slot, ins, t, i, pos);
        run_insert(slot, ins, &mut strip.buf, busses, live.sidechain_current, frames);
        if gr == 0.0 {
            gr = slot.plugin.gain_reduction_db();
        }
    }
    if live.audition.inserts {
        let ins_lat: usize = t
            .mixer
            .inserts
            .iter()
            .zip(strip.plugins.iter())
            .filter_map(|(i, p)| i.as_ref().filter(|i| i.active).and(p.as_ref()))
            .map(|p| p.plugin.latency())
            .sum();
        strip.audition_delay.set(ins_lat, strip.buf.len());
        strip.audition_delay.process(&mut strip.buf, frames);
    }
    // Native Stereo Width (VM03): after user Pre inserts, before the pre-fader tap; exact
    // pass-through at a settled 100 %; ramps over the block when it changes.
    if strip.buf.len() == 2 {
        let w = if t.mixer.width.is_finite() { t.mixer.width.clamp(0.0, 2.0) } else { 1.0 };
        if let [l, r] = strip.buf.as_mut_slice()
            && let (Some(l), Some(r)) = (l.get_mut(..frames), r.get_mut(..frames))
        {
            soundcraft_dsp::mixmath::apply_width_ramp(l, r, strip.width_prev, w);
        }
        strip.width_prev = w;
    }
    strip.gr = gr;
    let auto_mute = t.mixer.automation_mode.reads() && t.lane(&AutoParam::Mute).is_some_and(|l| l.value_at(pos, 0.0) >= 0.5);
    let mut soloed_out = if s.mix_profile.graph_version >= 2 {
        any_solo && !solo_admits(s, t)
    } else {
        any_solo && !t.mixer.solo && !t.mixer.solo_safe && !receives_solo(s, t)
    };
    // Listen audition (T09): a source track outside the mask is excluded like a solo would
    // exclude it (buses and returns the audited tracks feed stay open; explicit mutes still win).
    if live.listen.active
        && matches!(t.kind, TrackKind::Audio | TrackKind::Instrument | TrackKind::Midi)
        && !live.listen.contains(s.track_index(t.id).unwrap_or(usize::MAX))
    {
        soloed_out = true;
    }
    strip.soloed_out = soloed_out;
    strip.mute_only = t.mixer.mute || auto_mute;
    strip.muted = t.mixer.mute || auto_mute || soloed_out;
    if t.mixer.sends.iter().flatten().any(|x| x.pre_fader) {
        strip.pre.resize_with(strip.buf.len(), Vec::new);
        for (d, src) in strip.pre.iter_mut().zip(strip.buf.iter()) {
            d.clear();
            d.extend_from_slice(src.get(..frames).unwrap_or(&[]));
        }
    }
    let mut v0 = volume_db_at(t, pos);
    let mut v1 = volume_db_at(t, pos + frames as i64);
    // Trim automation (an offset on top of the volume curve).
    if let Some(l) = t
        .automation
        .iter()
        .find(|l| matches!(&l.param, AutoParam::Plugin { slot: u8::MAX, param } if param == "trim"))
        .filter(|l| !l.points.is_empty())
    {
        v0 += l.value_at(pos, 0.0);
        v1 += l.value_at(pos + frames as i64, 0.0);
    }
    // VCA master: its fader offsets the member's, and its mute mutes the member.
    if let Some(vid) = t.mixer.vca
        && let Some(vca) = s.tracks.iter().find(|x| x.id.0 == vid && x.kind == TrackKind::Vca)
    {
        v0 += volume_db_at(vca, pos);
        v1 += volume_db_at(vca, pos + frames as i64);
        if vca.mixer.mute {
            strip.muted = true;
        }
    }
    if strip.muted {
        strip.mute_only = strip.mute_only || !strip.soloed_out;
    }
    let (g0, g1) = if strip.muted { (-144.0, -144.0) } else { (v0.min(12.0), v1.min(12.0)) };
    for c in strip.buf.iter_mut() {
        for i in 0..frames {
            if let Some(x) = c.get_mut(i) {
                *x *= lerp_gain(g0, g1, i, frames);
            }
        }
    }
    // User Post inserts (graph v2): after the fader, before Master Tape Post / post-fader tap.
    // (While inserts are audition-bypassed their latency is already in `audition_delay`.)
    if v2_graph {
        for (i, ins) in t.mixer.inserts.iter().enumerate() {
            let (Some(ins), Some(Some(slot))) = (ins, strip.plugins.get_mut(i)) else { continue };
            if !ins.active || !ins.post_fader || (live.tracking && s.tracking_off.contains(&(t.id, i))) {
                continue;
            }
            let lat = slot.plugin.latency();
            latency = latency.saturating_add(lat);
            if live.audition.inserts {
                continue;
            }
            if ins.bypass {
                slot.bypass_delay.set(lat, strip.buf.len());
                slot.bypass_delay.process(&mut strip.buf, frames);
                continue;
            }
            apply_params(slot, ins, t, i, pos);
            run_insert(slot, ins, &mut strip.buf, busses, live.sidechain_current, frames);
        }
    }
    // Bus Master Tape when Post: after the fader (and user Post inserts), before the post tap.
    if is_bus && t.mixer.slots.master_tape_position == TapePosition::Post {
        let [_, _, mt_i] = &mut strip.dedicated;
        let [_, _, mt_d] = &mut strip.slot_delay;
        let [_, _, mt_io] = &mut strip.slot_io;
        latency += run_dedicated(
            t.mixer.slots.master_tape.as_ref(),
            mt_i,
            mt_d,
            mt_io,
            &mut strip.buf,
            frames,
            live.audition.tape,
            live.tracking && s.tracking_off.contains(&(t.id, SlotKind::MasterTape.instance_index())),
        );
    }
    strip.latency = latency;
}

/// CL03 Hardware Monitor mode: the interface's direct monitoring is used, so software input
/// monitoring is off for every track (playback continues; recording stays dry).
fn hardware_monitoring(s: &Session) -> bool {
    s.edit.value("monitor.mode", 1.0) == 2.0
}

/// Runs one dedicated slot in place: provider, then its post-provider trim. A bypassed (or
/// audition-bypassed) slot skips provider *and* trim together but keeps the provider's latency
/// as a plain delay so path compensation stays valid. An empty unity slot touches nothing.
/// Returns the latency the slot contributes. Provider settings come from its own state/editor.
#[allow(clippy::too_many_arguments)]
fn run_dedicated(
    cfg: Option<&DedicatedSlot>,
    inst: &mut Option<PluginSlot>,
    delay: &mut DelayLine,
    io: &mut SlotIo,
    buf: &mut [Vec<f32>],
    frames: usize,
    audition_off: bool,
    deactivated: bool,
) -> usize {
    let block_peak = |buf: &[Vec<f32>]| buf.iter().map(|c| peak(c.get(..frames).unwrap_or(&[]))).fold(0.0f32, f32::max);
    let Some(cfg) = cfg else {
        *io = SlotIo::default();
        return 0;
    };
    let input = block_peak(buf);
    if deactivated {
        // CL03 Tracking deactivation: out of the live graph entirely (no processing, no delay).
        *io = SlotIo { input, output: input, active: false };
        return 0;
    }
    let lat = inst.as_ref().map_or(0, |p| p.plugin.latency());
    if cfg.bypass || audition_off {
        delay.set(lat, buf.len());
        delay.process(buf, frames);
        *io = SlotIo { input, output: block_peak(buf), active: true };
        return lat;
    }
    if let Some(p) = inst.as_mut() {
        p.plugin.process(buf, frames);
    }
    if cfg.trim_db != 0.0 {
        let g = db_to_gain(cfg.trim_db);
        for c in buf.iter_mut() {
            c.iter_mut().take(frames).for_each(|x| *x *= g);
        }
    }
    *io = SlotIo { input, output: block_peak(buf), active: true };
    lat
}

/// True when `from` feeds `to` through outputs/sends and bus inputs (bounded depth; no
/// allocation, safe on the audio thread).
fn feeds(s: &Session, from: &Track, to: &Track, depth: u8) -> bool {
    if depth == 0 {
        return false;
    }
    let reaches = |b: BusId| -> bool {
        if to.mixer.input == Route::Bus(b) && matches!(to.kind, TrackKind::Aux | TrackKind::Folder) {
            return true;
        }
        s.tracks
            .iter()
            .filter(|a| matches!(a.kind, TrackKind::Aux | TrackKind::Folder) && a.mixer.input == Route::Bus(b) && a.id != from.id)
            .any(|a| feeds(s, a, to, depth - 1))
    };
    if let Route::Bus(b) = from.mixer.output
        && reaches(b)
    {
        return true;
    }
    from.mixer.sends.iter().flatten().any(|x| matches!(x.target, Route::Bus(b) if reaches(b)))
}

/// Graph-version-2 solo policy (BUS_WORKFLOW_SPEC): a strip stays eligible when it is soloed,
/// solo-safe, downstream of a soloed strip (its buses/returns, nested), or upstream of a soloed
/// bus (that bus's contributors). Unrelated sources sharing a return are not admitted.
fn solo_admits(s: &Session, t: &Track) -> bool {
    if t.mixer.solo || t.mixer.solo_safe {
        return true;
    }
    s.tracks.iter().filter(|o| o.mixer.solo && !o.inactive && o.id != t.id).any(|o| feeds(s, o, t, 8) || feeds(s, t, o, 8))
}

/// Clip-effect parameters of a clip, or None when it has none or they are bypassed.
fn clip_fx_params(s: &Session, clip: u64) -> Option<Vec<(String, f32)>> {
    if s.edit.values.is_empty() {
        return None;
    }
    let pre = format!("clip_fx.{clip}.");
    let v: Vec<(String, f32)> = s
        .edit
        .values
        .range(pre.clone()..)
        .take_while(|(k, _)| k.starts_with(&pre))
        .map(|(k, v)| (k.get(pre.len()..).unwrap_or("").to_string(), *v as f32))
        .collect();
    if v.is_empty() || s.edit.flag(&format!("clip_fx.bypass.{clip}")) { None } else { Some(v) }
}

impl ClipFx {
    fn new(sr: f32, ch: usize) -> ClipFx {
        ClipFx { eq: None, comp: None, gain_db: 0.0, sr, ch }
    }
    /// Clip Effects EQ/compressor were upstream bundled processors and are removed in SML
    /// Recording: their parameters stay in the session (inactive) and are not processed; the
    /// clip-effects gain stage remains.
    fn make(&self, _id: &str) -> Option<Box<dyn Plugin>> {
        let _ = (self.sr, self.ch);
        None
    }
    /// Modules exist only when one of their parameters is set.
    fn configure(&mut self, params: &[(String, f32)]) {
        self.gain_db = 0.0;
        let has_eq = params.iter().any(|(k, _)| k.starts_with("eq."));
        let has_comp = params.iter().any(|(k, _)| k.starts_with("comp."));
        if has_eq && self.eq.is_none() {
            self.eq = self.make("eq_7band");
        } else if !has_eq {
            self.eq = None;
        }
        if has_comp && self.comp.is_none() {
            self.comp = self.make("compressor");
        } else if !has_comp {
            self.comp = None;
        }
        for (k, v) in params {
            if let Some(p) = k.strip_prefix("eq.") {
                if let Some(eq) = &mut self.eq
                    && eq.param(p) != Some(*v)
                {
                    eq.set_param(p, *v);
                }
            } else if let Some(p) = k.strip_prefix("comp.") {
                if let Some(c) = &mut self.comp
                    && c.param(p) != Some(*v)
                {
                    c.set_param(p, *v);
                }
            } else if k == "gain" {
                self.gain_db = *v;
            }
        }
    }
    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        if let Some(eq) = &mut self.eq {
            eq.process(io, frames);
        }
        if let Some(c) = &mut self.comp {
            c.process(io, frames);
        }
        if self.gain_db.abs() > 1e-6 {
            let g = db_to_gain(self.gain_db);
            for ch in io.iter_mut() {
                ch.iter_mut().take(frames).for_each(|x| *x *= g);
            }
        }
    }
}

/// Hash of everything `sync`/`processing_order` depend on (tracks, kinds, routing, plugins, sends).
fn structure_fingerprint(s: &Session) -> usize {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for t in &s.tracks {
        t.id.hash(&mut h);
        t.kind.hash(&mut h);
        t.format.hash(&mut h);
        t.inactive.hash(&mut h);
        t.mixer.inserts.len().hash(&mut h);
        t.mixer.input.hash(&mut h);
        t.mixer.output.hash(&mut h);
        for i in &t.mixer.inserts {
            i.as_ref().map(|x| x.plugin.as_str()).hash(&mut h);
        }
        for snd in t.mixer.sends.iter().flatten() {
            snd.target.hash(&mut h);
        }
        t.instrument.as_ref().map(|x| x.plugin.as_str()).hash(&mut h);
        for k in SlotKind::ALL {
            t.mixer.slots.get(k).and_then(|x| x.provider.as_ref()).map(|x| x.plugin.as_str()).hash(&mut h);
        }
    }
    for b in &s.busses {
        b.id.hash(&mut h);
        b.format.hash(&mut h);
    }
    s.main_format().hash(&mut h);
    h.finish() as usize
}

std::thread_local! {
    static AUDIO_THREAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Mark the calling thread as the realtime audio thread (`soundcraft_playback`'s cpal callbacks
/// do, every call: one thread-local store). Strips [`MixEngine::render`] hands to worker threads
/// count as the audio thread while they render for it. Code that might run there asks
/// [`on_audio_thread`] before doing anything that blocks; the desktop app's logger does, so a
/// `log::` record from the audio thread never takes a lock or touches a file.
pub fn mark_audio_thread() {
    AUDIO_THREAD.set(true);
}

/// True on the thread [`mark_audio_thread`] marked, and on a worker while it renders strips for
/// it. Lock-free and allocation-free.
pub fn on_audio_thread() -> bool {
    AUDIO_THREAD.get()
}

/// Run `f` on this thread as (or as not) the audio thread, then restore the thread's own mark
/// (also when `f` unwinds): a pool worker renders for realtime playback and offline bounces alike.
#[cfg(not(target_arch = "wasm32"))]
fn as_audio_thread(audio: bool, f: impl FnOnce()) {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            AUDIO_THREAD.set(self.0);
        }
    }
    let _restore = Restore(AUDIO_THREAD.replace(audio));
    f();
}

/// Strips hosting processors needed before rendering fans out to worker threads.
pub const PARALLEL_MIN_HEAVY_STRIPS: usize = 4;

/// Process strips, in parallel on native targets when there are enough of them.
#[cfg(not(target_arch = "wasm32"))]
fn run_strips<T: Send>(work: &mut [T], f: impl Fn(&mut T) + Sync + Send) {
    use rayon::prelude::*;
    if work.len() >= 4 {
        // The workers render on behalf of this thread: on the audio thread, they are it too.
        let audio = on_audio_thread();
        work.par_iter_mut().for_each(|w| as_audio_thread(audio, || f(w)));
    } else {
        work.iter_mut().for_each(f);
    }
}

#[cfg(target_arch = "wasm32")]
fn run_strips<T: Send>(work: &mut [T], f: impl Fn(&mut T) + Sync + Send) {
    work.iter_mut().for_each(f);
}

fn strip_channels(t: &Track, main_ch: usize) -> usize {
    match t.kind {
        TrackKind::Master => main_ch,
        TrackKind::Instrument => t.channels().clamp(2, MAX_CHANNELS),
        _ => t.channels().clamp(1, MAX_CHANNELS),
    }
}

/// The format a strip's buffer actually carries (a mono instrument renders in stereo).
fn strip_format(t: &Track, n: usize) -> ChannelFormat {
    if t.format.channels().min(MAX_CHANNELS) == n { t.format } else { ChannelFormat::for_channels(n) }
}

/// Audio/instrument tracks first, then auxes ordered so bus producers come before consumers.
fn processing_order(s: &Session) -> Vec<usize> {
    let mut order: Vec<usize> =
        (0..s.tracks.len()).filter(|&i| s.tracks.get(i).is_some_and(|t| !matches!(t.kind, TrackKind::Aux | TrackKind::Folder))).collect();
    let mut auxes: Vec<usize> =
        (0..s.tracks.len()).filter(|&i| s.tracks.get(i).is_some_and(|t| matches!(t.kind, TrackKind::Aux | TrackKind::Folder))).collect();
    // Depth = how many aux hops feed this aux's input bus (bounded; cycles cap out).
    let depth = |i: usize| -> usize {
        let mut d = 0;
        let mut cur = vec![i];
        for _ in 0..8 {
            let inputs: Vec<&Route> = cur.iter().filter_map(|&k| s.tracks.get(k)).map(|t| &t.mixer.input).collect();
            let feeders: Vec<usize> = auxes_feeding(s, &inputs);
            if feeders.is_empty() {
                break;
            }
            d += 1;
            cur = feeders;
        }
        d
    };
    auxes.sort_by_key(|&i| depth(i));
    order.extend(auxes);
    order
}

fn auxes_feeding(s: &Session, inputs: &[&Route]) -> Vec<usize> {
    s.tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| matches!(t.kind, TrackKind::Aux | TrackKind::Folder))
        .filter(|(_, t)| {
            inputs.iter().any(|r| matches!(r, Route::Bus(_)) && (&&t.mixer.output == r || t.mixer.sends.iter().flatten().any(|sn| &&sn.target == r)))
        })
        .map(|(i, _)| i)
        .collect()
}

/// Implicit solo: an aux is not muted when a soloed track feeds its input bus.
fn receives_solo(s: &Session, t: &Track) -> bool {
    let Route::Bus(b) = &t.mixer.input else { return false };
    s.tracks
        .iter()
        .any(|o| o.mixer.solo && (o.mixer.output == Route::Bus(*b) || o.mixer.sends.iter().flatten().any(|sn| sn.target == Route::Bus(*b))))
}

/// Metronome: a short synthesized blip on every beat of the tempo map, accented on beat 1 of
/// each bar. Settings come from `setup.click_countoff` (`click.*` values).
mod click {
    use super::*;

    /// Blip length in seconds.
    const LEN_S: f64 = 0.03;

    /// Clicks on beats before `until` (exclusive).
    pub fn render(s: &Session, pos: Samples, frames: usize, main: &mut [Vec<f32>], until: Samples) {
        let sr = s.sample_rate;
        let rate = sr.as_f64();
        let len = (LEN_S * rate) as i64;
        let end = pos.saturating_add(frames as i64);
        let gain = f64::from(db_to_gain(s.edit.value("click.volume_db", -6.0).clamp(-60.0, 12.0) as f32));
        let accent = (note_hz(s.edit.value("click.accent_note", 84.0)), s.edit.value("click.accent_velocity", 127.0));
        let normal = (note_hz(s.edit.value("click.normal_note", 72.0)), s.edit.value("click.normal_velocity", 100.0));
        // Start one blip length early so a click that began in an earlier block still rings into this one.
        let t0 = s.tempo.samples_to_ticks(pos.saturating_sub(len), sr).max(0).saturating_sub(1);
        let mut tick = t0.saturating_sub(s.tempo.bar_beat_at_tick(t0).tick);
        // Blocks hold a few beats at most; the guard only protects against a malformed tempo map.
        for _ in 0..4096 {
            let at = s.tempo.tick_to_samples(tick, sr);
            if at >= end || at >= until {
                break;
            }
            if at.saturating_add(len) > pos {
                let (hz, vel) = if s.tempo.bar_beat_at_tick(tick).beat == 1 { accent } else { normal };
                let amp = gain * (vel.clamp(1.0, 127.0) / 127.0);
                blip(main, at - pos, frames, hz, amp as f32, rate);
            }
            tick = tick.saturating_add(s.tempo.meter_at_tick(tick).ticks_per_beat());
        }
    }

    /// Adds one decaying sine blip whose first sample sits `rel` frames into the block (negative
    /// when it began before the block) to every channel.
    fn blip(main: &mut [Vec<f32>], rel: i64, frames: usize, hz: f64, amp: f32, rate: f64) {
        let len = (LEN_S * rate) as usize;
        let w = std::f64::consts::TAU * hz / rate;
        let first = usize::try_from(-rel).unwrap_or(0);
        for i in first..len {
            let Ok(at) = usize::try_from(rel + i as i64) else { continue };
            if at >= frames {
                break;
            }
            let env = (-6.0 * i as f64 / len as f64).exp() as f32;
            let v = ((w * i as f64).sin() as f32) * env * amp;
            for ch in main.iter_mut() {
                if let Some(d) = ch.get_mut(at) {
                    *d += v;
                }
            }
        }
    }

    /// Click pitch for a MIDI note setting (clamped to the MIDI range).
    fn note_hz(note: f64) -> f64 {
        soundcraft_dsp::midi_to_hz(note.clamp(0.0, 127.0))
    }
}

fn render_clip(s: &Session, clip: &Clip, pos: Samples, frames: usize, buf: &mut [Vec<f32>], _scratch: &mut [f32]) {
    let ClipContent::Audio { source, offset } = clip.content else { return };
    let block = Range { start: pos, end: pos.saturating_add(frames as i64) };
    let Some(isect) = clip.range().intersect(&block) else { return };
    let Some(audio) = s.pool.get(source) else { return };
    let nsrc = audio.buffer.channels.len();
    if nsrc == 0 {
        return;
    }
    let stretch = if clip.stretch.is_finite() && clip.stretch > 0.0 { clip.stretch } else { 1.0 };
    let i0 = usize::try_from(isect.start - pos).unwrap_or(0);
    let i1 = usize::try_from(isect.end - pos).unwrap_or(0).min(frames);
    // Gain is evaluated every 32 samples and interpolated (fades and envelopes stay smooth).
    const STEP: usize = 32;
    for (ch, dst) in buf.iter_mut().enumerate() {
        let src = audio.buffer.channels.get(ch % nsrc).map(Vec::as_slice).unwrap_or(&[]);
        let mut i = i0;
        while i < i1 {
            let seg_end = (i + STEP).min(i1);
            let rel0 = pos.saturating_add(i as i64).saturating_sub(clip.start);
            let rel1 = pos.saturating_add(seg_end as i64).saturating_sub(clip.start);
            let g0 = clip.gain_at(rel0);
            let g1 = clip.gain_at(rel1.min(clip.length));
            let n = (seg_end - i) as f32;
            for k in i..seg_end {
                let rel = pos.saturating_add(k as i64).saturating_sub(clip.start);
                let v = if (stretch - 1.0).abs() < 1e-9 {
                    usize::try_from(offset.saturating_add(rel)).ok().and_then(|x| src.get(x)).copied().unwrap_or(0.0)
                } else {
                    let f = offset as f64 + rel as f64 / stretch;
                    let x0 = f.floor();
                    let fr = (f - x0) as f32;
                    let a = if x0 >= 0.0 { src.get(x0 as usize).copied().unwrap_or(0.0) } else { 0.0 };
                    let b = if x0 + 1.0 >= 0.0 { src.get((x0 as usize).saturating_add(1)).copied().unwrap_or(0.0) } else { 0.0 };
                    a + (b - a) * fr
                };
                let t = (k - i) as f32 / n;
                if let Some(d) = dst.get_mut(k) {
                    *d += v * (g0 + (g1 - g0) * t);
                }
            }
            i = seg_end;
        }
    }
}

/// Send note on/offs for MIDI clips overlapping the block.
fn schedule_notes(s: &Session, t: &Track, pos: Samples, frames: usize, inst: &mut PluginSlot, held: &mut Vec<u8>) {
    let sr = s.sample_rate;
    let end = pos.saturating_add(frames as i64);
    for clip in t.clips() {
        if clip.muted {
            continue;
        }
        let ClipContent::Midi { sequence } = &clip.content else { continue };
        if !clip.range().overlaps(&Range { start: pos, end }) {
            continue;
        }
        let base = s.tempo.samples_to_ticks(clip.start, sr);
        // MIDI Real-Time Properties (non-destructive velocity/transpose/duration/delay).
        let rtp = |name: &str, d: f64| s.edit.value(&format!("rtp.{}.{name}", t.id.0), d);
        let (dv, dp, dur, delay) = if s.edit.values.is_empty() {
            (0i64, 0i64, 1.0f64, 0i64)
        } else {
            (
                rtp("velocity", 0.0).clamp(-127.0, 127.0) as i64,
                rtp("transpose", 0.0).clamp(-127.0, 127.0) as i64,
                rtp("duration", 100.0).clamp(1.0, 1000.0) / 100.0,
                rtp("delay", 0.0).clamp(-1e7, 1e7) as i64,
            )
        };
        for n0 in &sequence.notes {
            let mut n = *n0;
            n.velocity = (i64::from(n.velocity) + dv).clamp(1, 127) as u8;
            n.pitch = (i64::from(n.pitch) + dp).clamp(0, 127) as u8;
            n.length = ((n.length as f64 * dur) as i64).max(1);
            n.start = n.start.saturating_add(delay).max(0);
            let on = s.tempo.tick_to_samples(base.saturating_add(n.start), sr);
            let off = s.tempo.tick_to_samples(base.saturating_add(n.start).saturating_add(n.length), sr).min(clip.end());
            if on >= clip.end() {
                continue;
            }
            if on >= pos && on < end {
                inst.plugin.note_on(usize::try_from(on - pos).unwrap_or(0), n.pitch, n.velocity);
                if !held.contains(&n.pitch) && held.len() < 128 {
                    held.push(n.pitch);
                }
            }
            if off >= pos && off < end {
                inst.plugin.note_off(usize::try_from(off - pos).unwrap_or(0), n.pitch);
                held.retain(|p| *p != n.pitch);
            }
        }
    }
}

fn apply_params(slot: &mut PluginSlot, ins: &soundcraft_model::Insert, t: &Track, i: usize, pos: Samples) {
    let slot_u8 = u8::try_from(i).unwrap_or(0);
    let automated = t.mixer.automation_mode.reads()
        && t.automation.iter().any(|l| matches!(&l.param, AutoParam::Plugin { slot, .. } if *slot == slot_u8) && !l.points.is_empty());
    if slot.applied.len() != ins.params.len() {
        slot.applied = vec![f32::NAN; ins.params.len()];
    }
    for ((k, v), last) in ins.params.iter().zip(slot.applied.iter_mut()) {
        let mut val = *v;
        if automated
            && let Some(l) = t.automation.iter().find(|l| matches!(&l.param, AutoParam::Plugin { slot, param } if *slot == slot_u8 && param == k))
            && !l.points.is_empty()
        {
            val = l.value_at(pos, val);
        }
        if val.to_bits() != last.to_bits() {
            slot.plugin.set_param(k, val);
            *last = val;
        }
    }
}

fn volume_db_at(t: &Track, pos: Samples) -> f32 {
    if t.mixer.automation_mode.reads()
        && t.mixer.automation_mode != soundcraft_model::AutomationMode::Write
        && let Some(l) = t.lane(&AutoParam::Volume)
        && !l.points.is_empty()
    {
        return l.value_at(pos, t.mixer.volume_db);
    }
    t.mixer.volume_db
}

fn pan_at(t: &Track, idx: usize, pos: Samples) -> f32 {
    let base = t.mixer.pan.get(idx).copied().unwrap_or(if idx == 0 { 0.0 } else { 1.0 });
    if t.mixer.automation_mode.reads()
        && let Some(l) = t.lane(&AutoParam::Pan(u8::try_from(idx).unwrap_or(0)))
        && !l.points.is_empty()
    {
        return l.value_at(pos, base);
    }
    base
}

fn send_level(t: &Track, i: usize, base: f32, pos: Samples) -> f32 {
    if t.mixer.automation_mode.reads()
        && let Some(l) = t.lane(&AutoParam::SendLevel(u8::try_from(i).unwrap_or(0)))
        && !l.points.is_empty()
    {
        return l.value_at(pos, base);
    }
    base
}

fn lerp_gain(db0: f32, db1: f32, i: usize, n: usize) -> f32 {
    let g0 = db_to_gain(db0);
    let g1 = db_to_gain(db1);
    if n == 0 { g0 } else { g0 + (g1 - g0) * (i as f32 / n as f32) }
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| if v.is_finite() { m.max(v.abs()) } else { m })
}

/// Per-block routing gains: `m[src][dst]`.
type Matrix = [[f32; MAX_CHANNELS]; MAX_CHANNELS];

/// How a source is panned: the stereo pan pair (mono uses `[0]`) and the optional surround pan.
struct PanSpec<'a> {
    stereo: [f32; 2],
    surround: Option<&'a SurroundPan>,
}

fn identity_matrix(n: usize) -> Matrix {
    let mut m = [[0.0; MAX_CHANNELS]; MAX_CHANNELS];
    for (i, row) in m.iter_mut().enumerate().take(n) {
        if let Some(g) = row.get_mut(i) {
            *g = 1.0;
        }
    }
    m
}

/// ITU-R BS.775 stereo fold-down gains (to L, to R) for one speaker: C and surrounds at -3 dB,
/// the centre surround at -6 dB per side, LFE dropped.
pub fn itu_stereo_fold(sp: Speaker) -> (f32, f32) {
    use Speaker::*;
    let h = std::f32::consts::FRAC_1_SQRT_2;
    match sp {
        L | Lc | Lw => (1.0, 0.0),
        R | Rc | Rw => (0.0, 1.0),
        C => (h, h),
        Lfe => (0.0, 0.0),
        Cs => (0.5, 0.5),
        Ls | Lss | Lrs | Ltf | Ltm | Ltr => (h, 0.0),
        Rs | Rss | Rrs | Rtf | Rtm | Rtr => (0.0, h),
    }
}

/// Speaker directions of a format (for the DSP panner). Returns the count written.
fn layout_of(fmt: ChannelFormat, out: &mut [SpeakerPos; MAX_CHANNELS]) -> usize {
    let sp = fmt.speakers();
    for (o, s) in out.iter_mut().zip(sp.iter()) {
        *o = SpeakerPos { az: s.azimuth(), el: s.elevation(), lfe: s.is_lfe() };
    }
    sp.len().min(MAX_CHANNELS)
}

fn surround_params(sp: &SurroundPan, dx: f32) -> SurroundParams {
    SurroundParams { x: (sp.x + dx).clamp(-1.0, 1.0), y: sp.y, z: sp.z, divergence: sp.divergence, center: sp.center / 100.0 }
}

/// Routing gains from a source (`ns` channels of `src_fmt`) into a destination (`nd` channels of
/// `dst_fmt`). See the crate docs for the rules.
fn build_matrix(src_fmt: ChannelFormat, ns: usize, dst_fmt: ChannelFormat, nd: usize, pan: &PanSpec<'_>, law: PanLaw) -> Matrix {
    let mut m = [[0.0f32; MAX_CHANNELS]; MAX_CHANNELS];
    let (ns, nd) = (ns.min(MAX_CHANNELS), nd.min(MAX_CHANNELS));
    if ns == 0 || nd == 0 {
        return m;
    }
    let set = |m: &mut Matrix, s: usize, d: usize, g: f32| {
        if let Some(x) = m.get_mut(s).and_then(|r| r.get_mut(d)) {
            *x += g;
        }
    };
    if nd == 1 {
        // Mono destination: the stereo routing summed at -6 dB.
        let st = build_matrix(src_fmt, ns, ChannelFormat::Stereo, 2, pan, law);
        for (row, srow) in m.iter_mut().zip(st.iter()).take(ns) {
            row[0] = 0.5 * (srow[0] + srow[1]);
        }
        return m;
    }
    let src_sp = src_fmt.speakers();
    let src_amb = src_fmt.is_ambisonic() && ns >= 4;
    if nd == 2 && !dst_fmt.is_ambisonic() {
        match ns {
            1 => {
                let (gl, gr) = gains(pan.stereo[0], law);
                m[0][0] = gl;
                m[0][1] = gr;
            }
            2 if !src_amb => {
                let (a_l, a_r) = gains(pan.stereo[0], PanLaw::Zero);
                let (b_l, b_r) = gains(pan.stereo[1], PanLaw::Zero);
                m[0] = [0.0; MAX_CHANNELS];
                m[0][0] = a_l;
                m[0][1] = a_r;
                m[1][0] = b_l;
                m[1][1] = b_r;
            }
            _ if src_amb => {
                for (d, az) in [(0usize, -30.0f32), (1, 30.0)] {
                    let g = soundcraft_dsp::pan::ambisonic_decode(az, 0.0, 2);
                    for (c, gc) in g.iter().enumerate() {
                        set(&mut m, c, d, *gc);
                    }
                }
            }
            _ if src_sp.len() == ns => {
                for (s, sp) in src_sp.iter().enumerate() {
                    let (l, r) = itu_stereo_fold(*sp);
                    set(&mut m, s, 0, l);
                    set(&mut m, s, 1, r);
                }
            }
            _ => {
                // Unknown layout: first two channels to L/R, the rest to the centre.
                let h = std::f32::consts::FRAC_1_SQRT_2;
                m[0][0] = 1.0;
                m[1][1] = 1.0;
                for row in m.iter_mut().take(ns).skip(2) {
                    row[0] = h;
                    row[1] = h;
                }
            }
        }
        return m;
    }
    if ns > 2 && src_fmt == dst_fmt && ns == nd {
        return identity_matrix(ns);
    }
    // Multichannel destination.
    let mut layout = [SpeakerPos::default(); MAX_CHANNELS];
    let nl = layout_of(dst_fmt, &mut layout);
    let layout = layout.get(..nl).unwrap_or(&[]);
    let dst_sp = dst_fmt.speakers();
    let dst_amb = dst_fmt.is_ambisonic();
    let find = |sp: Speaker| dst_sp.iter().position(|x| *x == sp).filter(|i| *i < nd);
    let mut g = [0.0f32; MAX_CHANNELS];
    if dst_amb {
        if src_amb || (src_fmt.is_ambisonic() && ns > 2) {
            return identity_matrix(ns.min(nd));
        }
        let mut enc = |m: &mut Matrix, s: usize, az: f32, el: f32, spread: f32| {
            soundcraft_dsp::pan::ambisonic_encode(az, el, spread, &mut g);
            for (d, gd) in g.iter().enumerate().take(nd) {
                set(m, s, d, *gd);
            }
        };
        match (ns, pan.surround) {
            (1 | 2, Some(sp)) => {
                for s in 0..ns {
                    let dx = if ns == 2 { if s == 0 { -1.0 } else { 1.0 } } else { 0.0 };
                    let p = surround_params(sp, dx);
                    enc(&mut m, s, soundcraft_dsp::pan::puck_azimuth(&[], p.x, p.y), p.z * 90.0, p.divergence);
                }
            }
            (1, None) => enc(&mut m, 0, pan.stereo[0].clamp(-1.0, 1.0) * 30.0, 0.0, 0.0),
            (2, None) => {
                enc(&mut m, 0, -30.0, 0.0, 0.0);
                enc(&mut m, 1, 30.0, 0.0, 0.0);
            }
            _ => {
                for (s, sp) in src_sp.iter().enumerate().take(ns) {
                    if !sp.is_lfe() {
                        enc(&mut m, s, sp.azimuth(), sp.elevation(), 0.0);
                    }
                }
            }
        }
        return m;
    }
    if dst_sp.len() != nd {
        // No layout to pan on: channel for channel.
        return identity_matrix(ns.min(nd));
    }
    let (li, ri) = (find(Speaker::L), find(Speaker::R));
    match (ns, pan.surround) {
        (1 | 2, Some(sp)) if !src_amb => {
            for s in 0..ns {
                let dx = if ns == 2 { if s == 0 { -1.0 } else { 1.0 } } else { 0.0 };
                soundcraft_dsp::pan::surround_gains(layout, &surround_params(sp, dx), &mut g);
                for (d, gd) in g.iter().enumerate().take(nd) {
                    set(&mut m, s, d, *gd);
                }
                if let Some(lfe) = find(Speaker::Lfe)
                    && sp.lfe_db > -143.9
                {
                    set(&mut m, s, lfe, db_to_gain(sp.lfe_db.min(12.0)) / ns as f32);
                }
            }
        }
        (1, None) => {
            let (gl, gr) = gains(pan.stereo[0], law);
            if let (Some(l), Some(r)) = (li, ri) {
                set(&mut m, 0, l, gl);
                set(&mut m, 0, r, gr);
            }
        }
        (2, None) if !src_amb => {
            let (a_l, a_r) = gains(pan.stereo[0], PanLaw::Zero);
            let (b_l, b_r) = gains(pan.stereo[1], PanLaw::Zero);
            if let (Some(l), Some(r)) = (li, ri) {
                set(&mut m, 0, l, a_l);
                set(&mut m, 0, r, a_r);
                set(&mut m, 1, l, b_l);
                set(&mut m, 1, r, b_r);
            }
        }
        _ if src_amb => {
            let n_spk = dst_sp.iter().filter(|s| !s.is_lfe()).count();
            for (d, sp) in dst_sp.iter().enumerate().take(nd) {
                if sp.is_lfe() {
                    continue;
                }
                let dec = soundcraft_dsp::pan::ambisonic_decode(sp.azimuth(), sp.elevation(), n_spk);
                for (c, gc) in dec.iter().enumerate() {
                    set(&mut m, c, d, *gc);
                }
            }
        }
        _ if src_sp.len() == ns => {
            for (s, sp) in src_sp.iter().enumerate() {
                if let Some(d) = find(*sp) {
                    set(&mut m, s, d, 1.0);
                } else if sp.is_lfe() {
                    // No LFE in the destination: dropped.
                } else {
                    soundcraft_dsp::pan::direction_gains(layout, sp.azimuth(), sp.elevation(), &mut g);
                    for (d, gd) in g.iter().enumerate().take(nd) {
                        set(&mut m, s, d, *gd);
                    }
                }
            }
        }
        _ => return identity_matrix(ns.min(nd)),
    }
    m
}

/// `dst[d] += Σ_s src[s] × gain × m[s][d]` for every destination channel (no allocation).
fn apply_matrix(src: &[Vec<f32>], frames: usize, dst: &mut [Vec<f32>], m: &Matrix, gain: f32) {
    let ns = src.len().min(MAX_CHANNELS);
    for (d, out) in dst.iter_mut().enumerate().take(MAX_CHANNELS) {
        let mut idx = [(0usize, 0.0f32); MAX_CHANNELS];
        let mut k = 0;
        for (s, row) in m.iter().enumerate().take(ns) {
            let g = row.get(d).copied().unwrap_or(0.0);
            if g != 0.0
                && let Some(slot) = idx.get_mut(k)
            {
                *slot = (s, g);
                k += 1;
            }
        }
        let taps = idx.get(..k).unwrap_or(&[]);
        if taps.is_empty() {
            continue;
        }
        for (i, o) in out.iter_mut().enumerate().take(frames) {
            let mut acc = 0.0f32;
            for &(s, g) in taps {
                acc += src.get(s).and_then(|c| c.get(i)).copied().unwrap_or(0.0) * gain * g;
            }
            *o += acc;
        }
    }
}

/// [`apply_matrix`] into f64 accumulators (graph version 2 bus/Main sums, M02): each
/// contribution is computed in f64 and added without f32 rounding between contributors.
fn apply_matrix_acc(src: &[Vec<f32>], frames: usize, dst: &mut [Vec<f64>], m: &Matrix, gain: f32) {
    let ns = src.len().min(MAX_CHANNELS);
    for (d, out) in dst.iter_mut().enumerate().take(MAX_CHANNELS) {
        for (s, row) in m.iter().enumerate().take(ns) {
            let g = f64::from(row.get(d).copied().unwrap_or(0.0)) * f64::from(gain);
            if g == 0.0 {
                continue;
            }
            let Some(c) = src.get(s) else { continue };
            for (o, x) in out.iter_mut().zip(c.iter()).take(frames) {
                *o += f64::from(*x) * g;
            }
        }
    }
}

/// Every active processor (inserts by index, dedicated slots by their instance index) with its
/// reported latency, for the CL03 Tracking ledger. Creates instances: call off the audio thread.
pub fn processor_latencies(s: &Session) -> Vec<(TrackId, usize, String, usize)> {
    let sr = s.sample_rate.as_f64() as f32;
    let main_ch = main_channels(s);
    let mut out = Vec::new();
    for t in &s.tracks {
        let ch = strip_channels(t, main_ch);
        let mut probe = |idx: usize, ins: &soundcraft_model::Insert| {
            if let Some(p) = create_instance(&ins.plugin, ins.state_bytes().as_deref(), sr, 512, ch) {
                out.push((t.id, idx, ins.plugin.clone(), p.latency()));
            }
        };
        for (i, ins) in t.mixer.inserts.iter().enumerate() {
            // Bypassed inserts keep their latency as a plain delay (compensation preserved).
            if let Some(ins) = ins.as_ref().filter(|x| x.active) {
                probe(i, ins);
            }
        }
        for k in SlotKind::ALL {
            if let Some(ins) = t.mixer.slots.get(k).filter(|x| !x.bypass).and_then(|x| x.provider.as_ref()) {
                probe(k.instance_index(), ins);
            }
        }
    }
    out
}

/// Per-strip processing latency (inserts + dedicated slots, plugin-reported) after one silent
/// block of the session, for the CL03 latency ledger. Creates plugins: call off the audio thread.
pub fn strip_latencies(s: &Session) -> Vec<(TrackId, usize)> {
    let mut eng = MixEngine::new(s.sample_rate.as_f64() as f32, 256);
    let mut out = vec![vec![0.0f32; 256]; main_channels(s)];
    eng.render(s, 0, 256, &mut out);
    let mut v: Vec<(TrackId, usize)> = eng.strips.iter().map(|(t, st)| (*t, st.latency)).collect();
    v.sort_by_key(|(t, _)| *t);
    v
}

/// Ceiling on one offline render (samples summed over channels): 2^29 f32 = 2 GiB, about 93
/// minutes of stereo at 48 kHz. Longer requests are truncated rather than exhausting memory.
pub const MAX_RENDER_SAMPLES: usize = 1 << 29;

/// Offline render of the main mix over `range` (planar, [`main_channels`] channels in SMPTE/WAV
/// order). The mix's delay-compensation latency is removed, so the result lines up with the
/// timeline.
pub fn render_range(s: &Session, range: Range, block: usize) -> Vec<Vec<f32>> {
    render_range_report(s, range, block, Tail::None).audio
}

/// How much to render past the end of the range so plugin tails (reverbs, delays, lookahead)
/// are not cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tail {
    #[default]
    None,
    /// A fixed number of samples.
    Samples(usize),
    /// The longest tail any hosted processor declares (0 when none does).
    Auto,
}

/// An offline render with its provenance: what it is and what it is not.
pub struct RenderReport {
    pub audio: Vec<Vec<f32>>,
    /// Hosted plugins that could not be created (track name, slot, id): their slots passed audio
    /// through, so `audio` is NOT the mix the user hears. Exports must refuse unless the user
    /// explicitly accepts a substituted result.
    pub failed_plugins: Vec<(String, usize, String)>,
    /// Hosted plugins that were created but did not process every block of this render (an
    /// isolated worker crashed, hung or was restarting): (track name, slot label, id, silent
    /// blocks). Like `failed_plugins`, the audio is NOT the mix; exports must refuse unless the
    /// user explicitly accepts it.
    pub faulted_plugins: Vec<(String, String, String, u64)>,
    /// Samples rendered past the range end.
    pub tail_samples: usize,
    /// Delay-compensation latency that was removed from the head.
    pub latency_samples: usize,
}

/// Mastering an imported program (R34): `program` (planar, session rate) replaces the mix as the
/// main sum and passes through the Master track's chain exactly once; tracks are not rendered.
pub fn render_program_report(s: &Session, program: &[Vec<f32>], range: Range, block: usize, tail: Tail) -> RenderReport {
    let mut solo = s.clone();
    for t in &mut solo.tracks {
        if t.kind != TrackKind::Master {
            t.inactive = true;
        }
    }
    let n = program.first().map_or(0, Vec::len);
    let src = soundcraft_model::SourceId(u64::MAX - 1);
    solo.pool.insert(
        src,
        std::sync::Arc::new(soundcraft_model::SourceAudio::new(soundcraft_audio_io::AudioBuffer {
            sample_rate: s.sample_rate.hz(),
            channels: program.to_vec(),
        })),
    );
    let fmt = if program.len() == 1 { ChannelFormat::Mono } else { ChannelFormat::Stereo };
    let t = solo.add_track(TrackKind::Audio, fmt, Some("Program"));
    let id = solo.new_clip_id();
    if let Some(pl) = solo.track_mut(t).and_then(|t| t.playlist_mut()) {
        pl.clips.push(Clip::audio(id, "program", src, 0, 0, n as i64));
    }
    if let Some(tr) = solo.track_mut(t) {
        tr.mixer.pan = if program.len() == 1 { vec![0.0] } else { vec![-1.0, 1.0] };
    }
    render_range_report(&solo, range, block, tail)
}

/// [`render_range`] with tail handling and the instantiation report (R31 offline evidence).
pub fn render_range_report(s: &Session, range: Range, block: usize, tail: Tail) -> RenderReport {
    let nch = main_channels(s);
    let len = usize::try_from(range.len().max(0)).unwrap_or(0).min(MAX_RENDER_SAMPLES / nch.max(1));
    // Probe the latency (and declared tails) with one block, then render `len + tail + latency`
    // and drop the head.
    let (latency, declared_tail, failed) = {
        let mut probe = MixEngine::new(s.sample_rate.as_f64() as f32, block);
        let b = probe.max_block();
        let mut tmp = vec![vec![0.0f32; b]; nch];
        probe.render(s, range.start, b.min(len.max(1)), &mut tmp);
        let tail = probe
            .strips
            .values()
            .flat_map(|st| st.plugins.iter().flatten().chain(st.dedicated.iter().flatten()))
            .map(|p| p.plugin.tail_samples())
            .max()
            .unwrap_or(0);
        let failed = probe
            .instantiate_failures
            .iter()
            .map(|(t, slot, id)| (s.track(*t).map_or_else(|| format!("track {}", t.0), |x| x.name.clone()), *slot, id.clone()))
            .collect::<Vec<_>>();
        (probe.latency, tail, failed)
    };
    let tail_samples = match tail {
        Tail::None => 0,
        Tail::Samples(n) => n,
        Tail::Auto => declared_tail,
    }
    .min(MAX_RENDER_SAMPLES / nch.max(1));
    let len = len.saturating_add(tail_samples);
    let total = len.saturating_add(latency);
    let mut out = vec![vec![0.0f32; total]; nch];
    let mut eng = MixEngine::new(s.sample_rate.as_f64() as f32, block);
    let b = eng.max_block();
    let mut tmp = vec![vec![0.0f32; b]; nch];
    let mut done = 0usize;
    while done < total {
        let n = (total - done).min(b);
        eng.render(s, range.start + done as i64, n, &mut tmp);
        for (o, t) in out.iter_mut().zip(tmp.iter()) {
            if let (Some(d), Some(sr)) = (o.get_mut(done..done + n), t.get(..n)) {
                d.copy_from_slice(sr);
            }
        }
        done += n;
    }
    if latency > 0 {
        for c in &mut out {
            c.drain(..latency.min(c.len()));
        }
    }
    let faulted = substituted_plugins(s, &eng);
    RenderReport { audio: out, failed_plugins: failed, faulted_plugins: faulted, tail_samples, latency_samples: latency }
}

/// Plugins of `eng` that substituted silence for some blocks, in track order.
fn substituted_plugins(s: &Session, eng: &MixEngine) -> Vec<(String, String, String, u64)> {
    let mut out = Vec::new();
    for t in &s.tracks {
        let Some(st) = eng.strips.get(&t.id) else { continue };
        for (label, slot) in st
            .plugins
            .iter()
            .enumerate()
            .map(|(i, p)| (format!("insert {}", char::from(b'a' + u8::try_from(i % 26).unwrap_or(0))), p.as_ref()))
            .chain(std::iter::once(("instrument".to_string(), st.instrument.as_ref())))
            .chain(st.dedicated.iter().map(|p| ("dedicated slot".to_string(), p.as_ref())))
        {
            if let Some(p) = slot {
                let n = p.plugin.substituted_blocks();
                if n > 0 {
                    out.push((t.name.clone(), label, p.id.clone(), n));
                }
            }
        }
    }
    out
}

/// ITU-R BS.775 stereo fold-down of planar audio in `fmt` (C and surrounds at -3 dB, LFE dropped;
/// Ambisonics play their W channel). Mono/stereo input is returned unchanged.
pub fn fold_down_stereo(fmt: ChannelFormat, ch: &[Vec<f32>]) -> Vec<Vec<f32>> {
    if ch.len() <= 2 {
        return ch.to_vec();
    }
    let len = ch.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = vec![vec![0.0f32; len]; 2];
    let sp = fmt.speakers();
    let h = std::f32::consts::FRAC_1_SQRT_2;
    for (c, src) in ch.iter().enumerate() {
        let (gl, gr) = match sp.get(c) {
            Some(x) if sp.len() == ch.len() => itu_stereo_fold(*x),
            _ if c == 0 => (h, h),
            _ => (0.0, 0.0),
        };
        for (g, dst) in [(gl, 0usize), (gr, 1)] {
            if g == 0.0 {
                continue;
            }
            if let Some(d) = out.get_mut(dst) {
                for (o, x) in d.iter_mut().zip(src.iter()) {
                    *o += x * g;
                }
            }
        }
    }
    out
}

/// Render only one track's clips (no mixer processing) — Consolidate.
pub fn render_clips(s: &Session, track: TrackId, range: Range) -> Vec<Vec<f32>> {
    let Some(t) = s.track(track) else { return Vec::new() };
    let len = usize::try_from(range.len().max(0)).unwrap_or(0).min(MAX_RENDER_SAMPLES / t.channels().max(1));
    let mut out = vec![vec![0.0f32; len]; t.channels().max(1)];
    let mut scratch = Vec::new();
    for c in t.clips() {
        if c.muted {
            continue;
        }
        match clip_fx_params(s, c.id.0) {
            None => render_clip(s, c, range.start, len, &mut out, &mut scratch),
            Some(params) => {
                let mut tmp = vec![vec![0.0f32; len]; out.len()];
                render_clip(s, c, range.start, len, &mut tmp, &mut scratch);
                let mut fx = ClipFx::new(s.sample_rate.as_f64() as f32, tmp.len());
                fx.configure(&params);
                let mut done = 0;
                while done < len {
                    let n = (len - done).min(4096);
                    let mut block: Vec<Vec<f32>> = tmp.iter().map(|ch| ch.get(done..done + n).map(<[f32]>::to_vec).unwrap_or_default()).collect();
                    fx.process(&mut block, n);
                    for (o, b) in out.iter_mut().zip(block.iter()) {
                        if let Some(dst) = o.get_mut(done..done + n) {
                            for (x, y) in dst.iter_mut().zip(b.iter()) {
                                *x += *y;
                            }
                        }
                    }
                    done += n;
                }
            }
        }
    }
    out
}

/// Render one track through its inserts (pre-fader) — Commit / Freeze.
pub fn render_track_pre_fader(s: &Session, track: TrackId, range: Range) -> Vec<Vec<f32>> {
    let mut solo = s.clone();
    // Multichannel tracks render into a main mix of their own format (straight through).
    let fmt = s.track(track).map_or(ChannelFormat::Stereo, |t| if t.channels() > 2 { t.format } else { ChannelFormat::Stereo });
    solo.outputs = vec![soundcraft_model::OutputPath { name: "Out".into(), first_channel: 0, format: fmt }];
    for t in &mut solo.tracks {
        if t.id == track {
            t.mixer.volume_db = 0.0;
            t.mixer.mute = false;
            t.mixer.solo = false;
            t.mixer.output = Route::Main;
            t.mixer.pan = match t.channels() {
                1 => vec![0.0],
                _ => vec![-1.0, 1.0],
            };
            t.automation.retain(|l| matches!(l.param, AutoParam::Plugin { .. }));
            t.mixer.sends.iter_mut().for_each(|x| *x = None);
            t.mixer.surround = None;
        } else {
            t.inactive = true;
        }
    }
    let mut out = render_range(&solo, range, 1024);
    // Mono tracks: fold back to one channel (undo the centre pan law).
    if s.track(track).is_some_and(|t| t.channels() == 1)
        && let Some(l) = out.first().cloned()
    {
        let g = std::f32::consts::SQRT_2;
        out = vec![l.iter().map(|x| x * g).collect()];
    }
    out
}

/// A hosted external plugin (CLAP / VST3 / AU). Removed upstream built-ins are never created:
/// their inserts stay in the session as inactive missing placeholders that pass audio through.
/// Safe Open (R05): when set, no hosted (CLAP/VST3/AU) plugin is loaded — every hosted slot
/// becomes a pass-through placeholder with its state kept, exactly like a missing plugin — and
/// the plugin scanner does not start. Session data is never changed by this switch.
static HOSTED_PLUGINS_DISABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_hosted_plugins_disabled(on: bool) {
    HOSTED_PLUGINS_DISABLED.store(on, std::sync::atomic::Ordering::Release);
}

pub fn hosted_plugins_disabled() -> bool {
    HOSTED_PLUGINS_DISABLED.load(std::sync::atomic::Ordering::Acquire)
}

fn create_plugin(id: &str) -> Option<Box<dyn Plugin>> {
    #[cfg(any(test, feature = "test-providers"))]
    if id.starts_with(test_providers::PREFIX) {
        return test_providers::create(id);
    }
    if soundcraft_dsp::is_removed_builtin(id) {
        return None;
    }
    if is_third_party(id) && hosted_plugins_disabled() {
        return None;
    }
    #[cfg(not(target_arch = "wasm32"))]
    if id.starts_with(SANDBOX_PREFIX) {
        return sandbox::create(id);
    }
    soundcraft_clap_host::create(id).or_else(|| soundcraft_vst3_host::create(id)).or_else(|| soundcraft_au_host::create(id))
}

#[cfg(test)]
mod safe_open_tests {
    use super::*;

    /// R05 Safe Open: hosted ids yield no instance (placeholder path), test providers and the
    /// session data are untouched, and the switch is reversible.
    #[test]
    fn safe_open_disables_hosted_plugins_only() {
        set_hosted_plugins_disabled(true);
        assert!(hosted_plugins_disabled());
        assert!(create_instance("clap:does.not.exist", None, 48_000.0, 64, 2).is_none());
        assert!(create_instance("vst3:00000000000000000000000000000001", None, 48_000.0, 64, 2).is_none());
        assert!(create_instance("test:gain:0", None, 48_000.0, 64, 2).is_some(), "built-in test providers are not hosted plugins");
        let mut s = Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Stereo, None);
        s.track_mut(t).unwrap().mixer.inserts[0] = Some(soundcraft_model::Insert::new("clap:some.plugin"));
        assert!(prepare_instances(&s, 48_000.0, 64).is_empty());
        assert_eq!(s.track(t).unwrap().mixer.inserts[0].as_ref().map(|i| i.plugin.as_str()), Some("clap:some.plugin"), "session untouched");
        set_hosted_plugins_disabled(false);
        assert!(!hosted_plugins_disabled());
    }
}

#[cfg(test)]
mod audio_thread_tests {
    use super::*;

    #[test]
    fn only_a_marked_thread_is_the_audio_thread() {
        assert!(!on_audio_thread());
        let marked = std::thread::spawn(|| {
            let before = on_audio_thread();
            mark_audio_thread();
            (before, on_audio_thread())
        })
        .join()
        .unwrap();
        assert_eq!(marked, (false, true));
        assert!(!on_audio_thread(), "marking another thread leaves this one alone");
    }

    /// What `on_audio_thread` says inside each strip job, for strips run from a thread that is
    /// (or is not) the audio thread.
    fn seen_by_strips(audio: bool) -> Vec<bool> {
        std::thread::spawn(move || {
            if audio {
                mark_audio_thread();
            }
            let mut work = vec![None; 64];
            run_strips(&mut work, |w| {
                // Long enough that the pool's workers take part.
                std::thread::sleep(std::time::Duration::from_millis(1));
                *w = Some(on_audio_thread());
            });
            work.into_iter().map(|w| w.unwrap()).collect()
        })
        .join()
        .unwrap()
    }

    #[test]
    fn strips_rendered_for_the_audio_thread_count_as_the_audio_thread_and_only_then() {
        assert!(seen_by_strips(true).iter().all(|&rt| rt), "a worker rendering for the audio thread is on it");
        assert!(seen_by_strips(false).iter().all(|&rt| !rt), "the workers don't stay marked afterwards");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_audio_io::AudioBuffer;
    use soundcraft_model::{Insert, SourceAudio, SourceId};
    use std::sync::Arc;

    fn session_with_dc(level: f32, frames: usize) -> (Session, TrackId) {
        let mut s = Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        let buf = AudioBuffer { sample_rate: 48_000, channels: vec![vec![level; frames]] };
        s.pool.insert(SourceId(900), Arc::new(SourceAudio::new(buf)));
        let id = s.new_clip_id();
        if let Some(pl) = s.track_mut(t).and_then(|t| t.playlist_mut()) {
            pl.clips.push(Clip::audio(id, "dc", SourceId(900), 0, 0, frames as i64));
        }
        (s, t)
    }

    #[test]
    fn mono_center_pan_is_minus_3db() {
        let (s, _) = session_with_dc(0.5, 4800);
        let out = render_range(&s, Range::new(0, 4800), 512);
        let expect = 0.5 * std::f32::consts::FRAC_1_SQRT_2;
        assert!((out[0][1000] - expect).abs() < 1e-3, "{}", out[0][1000]);
        assert!((out[1][1000] - expect).abs() < 1e-3);
    }

    #[test]
    fn fader_mute_and_solo() {
        let (mut s, t) = session_with_dc(0.5, 4800);
        s.track_mut(t).unwrap().mixer.volume_db = -6.0;
        let out = render_range(&s, Range::new(0, 4800), 512);
        let expect = 0.5 * std::f32::consts::FRAC_1_SQRT_2 * db_to_gain(-6.0);
        assert!((out[0][2000] - expect).abs() < 1e-3);
        s.track_mut(t).unwrap().mixer.mute = true;
        let out = render_range(&s, Range::new(0, 4800), 512);
        assert!(out[0][2000].abs() < 1e-6);
        // Soloing another track mutes this one.
        s.track_mut(t).unwrap().mixer.mute = false;
        let other = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        s.track_mut(other).unwrap().mixer.solo = true;
        let out = render_range(&s, Range::new(0, 4800), 512);
        assert!(out[0][2000].abs() < 1e-6);
    }

    #[test]
    fn hard_pans() {
        let (mut s, t) = session_with_dc(0.5, 4800);
        s.track_mut(t).unwrap().mixer.pan = vec![-1.0];
        let out = render_range(&s, Range::new(0, 4800), 256);
        assert!(out[0][100] > 0.49 && out[1][100].abs() < 1e-4);
    }

    #[test]
    fn fades_and_clip_gain_apply() {
        let (mut s, t) = session_with_dc(1.0, 48_000);
        {
            let c = &mut s.track_mut(t).unwrap().playlist_mut().unwrap().clips[0];
            c.fade_in = soundcraft_model::Fade { len: 1000, shape: soundcraft_model::FadeShape::Linear };
            c.gain_db = -6.0;
        }
        let out = render_range(&s, Range::new(0, 2000), 512);
        let g = std::f32::consts::FRAC_1_SQRT_2 * db_to_gain(-6.0);
        assert!(out[0][0].abs() < 1e-3);
        assert!((out[0][500] - 0.5 * g).abs() < 0.02, "{}", out[0][500]);
        assert!((out[0][1500] - g).abs() < 1e-3);
    }

    #[test]
    fn sends_feed_aux_via_bus() {
        let (mut s, t) = session_with_dc(0.5, 4800);
        let bus = s.add_bus("Verb", ChannelFormat::Stereo);
        let aux = s.add_track(TrackKind::Aux, ChannelFormat::Stereo, Some("Verb"));
        s.track_mut(aux).unwrap().mixer.input = Route::Bus(bus);
        {
            let tr = s.track_mut(t).unwrap();
            tr.mixer.output = Route::None;
            let mut snd = soundcraft_model::SendSlot::new(Route::Bus(bus));
            snd.level_db = 0.0;
            tr.mixer.sends[0] = Some(snd);
        }
        let out = render_range(&s, Range::new(0, 4800), 512);
        assert!(out[0][1000] > 0.1, "aux should carry the send: {}", out[0][1000]);
    }

    #[test]
    fn volume_automation_ramps() {
        let (mut s, t) = session_with_dc(0.5, 48_000);
        {
            let tr = s.track_mut(t).unwrap();
            let l = tr.lane_mut(&AutoParam::Volume);
            l.set_point(0, 0.0);
            l.set_point(24_000, -144.0);
        }
        let out = render_range(&s, Range::new(0, 48_000), 512);
        assert!(out[0][100] > 0.3);
        assert!(out[0][30_000].abs() < 1e-4);
    }

    #[test]
    fn inserts_process_and_survive_unknown_plugins() {
        let (mut s, t) = session_with_dc(0.5, 4800);
        s.track_mut(t).unwrap().mixer.inserts[0] = Some(Insert::new("does-not-exist"));
        let out = render_range(&s, Range::new(0, 4800), 512);
        assert!(out[0][100] > 0.3);
        let clean = render_range(&s, Range::new(0, 4800), 512);
        // A removed upstream built-in in an old session: kept as an inactive missing placeholder
        // (id/params retained, audio passes through unchanged, never silently replaced).
        let mut ins = Insert::new("gain");
        ins.params.insert("gain".into(), -6.0);
        s.track_mut(t).unwrap().mixer.inserts[0] = Some(ins);
        assert!(is_missing_plugin("gain") && is_missing_plugin("eq_7band") && is_missing_plugin("subtractive_synth"));
        let out = render_range(&s, Range::new(0, 4800), 512);
        assert_eq!(out, clean, "placeholder passes audio through");
        assert_eq!(s.track(t).unwrap().mixer.inserts[0].as_ref().unwrap().params.get("gain"), Some(&-6.0), "state retained");
        // A test fixture with a real effect proves inserts still process.
        s.track_mut(t).unwrap().mixer.inserts[0] = Some(Insert::new("test:gain:-6"));
        let out = render_range(&s, Range::new(0, 4800), 512);
        assert!((out[0][100] / clean[0][100] - 10f32.powf(-6.0 / 20.0)).abs() < 1e-5);
    }

    /// A built-in gain behind a third-party id, with a state blob and a log of parameter sets.
    struct Stateful {
        inner: Box<dyn Plugin>,
        blob: Vec<u8>,
        sets: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl Plugin for Stateful {
        fn info(&self) -> &'static soundcraft_dsp::PluginInfo {
            self.inner.info()
        }
        fn prepare(&mut self, sr: f32, mb: usize, ch: usize) {
            self.inner.prepare(sr, mb, ch);
        }
        fn reset(&mut self) {
            self.inner.reset();
        }
        fn set_param(&mut self, id: &str, v: f32) -> bool {
            self.sets.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.inner.set_param(id, v)
        }
        fn param(&self, id: &str) -> Option<f32> {
            self.inner.param(id)
        }
        fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
            self.inner.process(io, frames);
        }
        fn save_state(&mut self) -> Option<Vec<u8>> {
            Some(self.blob.clone())
        }
    }

    fn render_block(eng: &mut MixEngine, s: &Session, pos: i64) -> f32 {
        let mut out = vec![vec![0.0f32; 512]; 2];
        eng.render(s, pos, 512, &mut out);
        out[0][500]
    }

    #[test]
    fn external_instances_are_adopted_retired_and_captured() {
        let (mut s, t) = session_with_dc(0.5, 48_000);
        let mut ins = Insert::new("clap:test.fake");
        ins.params.insert("gain".into(), -6.0);
        s.track_mut(t).unwrap().mixer.inserts[0] = Some(ins);
        let specs = instance_specs(&s);
        assert_eq!(specs, vec![InstanceSpec { track: t, slot: 0, id: "clap:test.fake".into(), ch: 1, state: None, params: 1 }]);
        let mut eng = MixEngine::new(48_000.0, 512);
        eng.set_external_instances(true);
        // Nothing to adopt yet: the slot passes audio through (and sync never tried to load a
        // CLAP plugin on this thread).
        let dry = render_block(&mut eng, &s, 0);
        assert!(dry > 0.3, "{dry}");
        let sets = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut gain = test_providers::create("test:gain:0").unwrap();
        gain.prepare(48_000.0, 512, 1);
        let fake = Stateful { inner: gain, blob: b"live state".to_vec(), sets: Arc::clone(&sets) };
        // An instance for the wrong channel count is retired, not used.
        let wrong = Stateful { inner: test_providers::create("test:gain:0").unwrap(), blob: Vec::new(), sets: Arc::clone(&sets) };
        eng.adopt(vec![PreparedInstance { track: t, slot: 0, id: "clap:test.fake".into(), ch: 2, plugin: Box::new(wrong), applied: Vec::new() }]);
        assert!(render_block(&mut eng, &s, 512) > 0.3);
        assert_eq!(eng.take_retired().len(), 1);
        eng.adopt(vec![PreparedInstance { track: t, slot: 0, id: "clap:test.fake".into(), ch: 1, plugin: Box::new(fake), applied: Vec::new() }]);
        let mut wet = 0.0;
        for b in 2..20 {
            wet = render_block(&mut eng, &s, b * 512);
        }
        assert!((wet / dry - 0.501).abs() < 0.01, "{wet} vs {dry}");
        assert!(sets.load(std::sync::atomic::Ordering::Relaxed) > 0, "no stored state: session params are pushed");
        assert_eq!(eng.capture_states(), vec![(t, 0, b"live state".to_vec())]);
        assert!(eng.take_retired().is_empty());
        // Removing the insert retires the instance instead of dropping it here.
        s.track_mut(t).unwrap().mixer.inserts[0] = None;
        render_block(&mut eng, &s, 20 * 512);
        assert_eq!(eng.take_retired().len(), 1);
        assert!(eng.capture_states().is_empty());
        // Instances nobody claims are retired too; drain_plugins empties the engine.
        eng.adopt(vec![PreparedInstance {
            track: t,
            slot: 3,
            id: "clap:x".into(),
            ch: 1,
            plugin: test_providers::create("test:gain:0").unwrap(),
            applied: Vec::new(),
        }]);
        render_block(&mut eng, &s, 21 * 512);
        assert_eq!(eng.take_retired().len(), 1);
        assert!(eng.drain_plugins().is_empty());
    }

    #[test]
    fn stored_state_comes_first_then_session_params() {
        let (mut s, t) = session_with_dc(0.5, 48_000);
        let mut ins = Insert::new("vst3:00000000000000000000000000000001");
        ins.params.insert("gain".into(), -6.0);
        ins.set_state_bytes(b"stored");
        s.track_mut(t).unwrap().mixer.inserts[0] = Some(ins);
        assert_eq!(instance_specs(&s)[0].state.as_deref(), Some(&b"stored"[..]));
        let mut eng = MixEngine::new(48_000.0, 512);
        eng.set_external_instances(true);
        let sets = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let fake = Stateful { inner: test_providers::create("test:gain:0").unwrap(), blob: Vec::new(), sets: Arc::clone(&sets) };
        eng.adopt(vec![PreparedInstance {
            track: t,
            slot: 0,
            id: "vst3:00000000000000000000000000000001".into(),
            ch: 1,
            plugin: Box::new(fake),
            applied: Vec::new(),
        }]);
        render_block(&mut eng, &s, 0);
        assert_eq!(sets.load(std::sync::atomic::Ordering::Relaxed), 1, "session params apply once");
        render_block(&mut eng, &s, 512);
        assert_eq!(sets.load(std::sync::atomic::Ordering::Relaxed), 1);
        s.track_mut(t).unwrap().mixer.inserts[0].as_mut().unwrap().params.insert("gain".into(), -12.0);
        render_block(&mut eng, &s, 1024);
        assert_eq!(sets.load(std::sync::atomic::Ordering::Relaxed), 2, "later session edits apply");
        // Offline (not external): unknown third-party plugins are simply absent.
        let out = render_range(&s, Range::new(0, 1024), 512);
        assert!(out[0][100] > 0.3);
    }

    #[test]
    fn clip_effects_gain_and_trim_automation_apply() {
        let (mut s, t) = session_with_dc(0.5, 4800);
        let cid = s.track(t).unwrap().clips()[0].id.0;
        s.edit.values.insert(format!("clip_fx.{cid}.gain"), -6.0);
        let out = render_range(&s, Range::new(0, 4800), 512);
        let expect = 0.5 * std::f32::consts::FRAC_1_SQRT_2 * db_to_gain(-6.0);
        assert!((out[0][2000] - expect).abs() < 0.01, "{}", out[0][2000]);
        s.edit.set_flag(&format!("clip_fx.bypass.{cid}"), true);
        let out = render_range(&s, Range::new(0, 4800), 512);
        assert!((out[0][2000] - 0.5 * std::f32::consts::FRAC_1_SQRT_2).abs() < 0.01);
        s.track_mut(t).unwrap().lane_mut(&AutoParam::Plugin { slot: u8::MAX, param: "trim".into() }).set_point(0, -12.0);
        let out = render_range(&s, Range::new(0, 4800), 512);
        assert!(out[0][2000] < 0.2, "{}", out[0][2000]);
    }

    #[test]
    fn delay_compensation_aligns_latent_tracks() {
        let mut s = Session::default();
        let mut imp = vec![0.0f32; 48_000];
        imp[1000] = 1.0;
        s.pool.insert(SourceId(900), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![imp] })));
        let mut ids = Vec::new();
        for (pan, fx) in [(-1.0f32, true), (1.0, false)] {
            let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
            let cid = s.new_clip_id();
            let tr = s.track_mut(t).unwrap();
            tr.playlist_mut().unwrap().clips.push(Clip::audio(cid, "i", SourceId(900), 0, 0, 48_000));
            tr.mixer.pan = vec![pan];
            if fx {
                // Known-delay test provider (was the removed bundled maximizer's lookahead).
                tr.mixer.inserts[0] = Some(Insert::new("test:delay:64"));
            }
            ids.push(t);
        }
        let argmax = |c: &[f32]| c.iter().enumerate().max_by(|a, b| a.1.abs().total_cmp(&b.1.abs())).map(|(i, _)| i).unwrap();
        let lat = create_plugin("test:delay:64").unwrap().latency();
        assert_eq!(lat, 64, "test needs a latent plugin");
        let out = render_range(&s, Range::new(0, 8000), 512);
        assert_eq!(argmax(&out[0]), argmax(&out[1]), "PDC should align L and R");
        s.edit.delay_compensation = false;
        let out = render_range(&s, Range::new(0, 8000), 512);
        assert_ne!(argmax(&out[0]), argmax(&out[1]));
    }

    #[test]
    fn input_monitoring_replaces_clips() {
        let (mut s, t) = session_with_dc(0.5, 4800);
        let mut eng = MixEngine::new(48_000.0, 512);
        eng.input = vec![vec![0.25; 512]];
        let mut out = vec![vec![0.0; 512]; 2];
        // Not monitored: the clip plays.
        eng.render(&s, 0, 512, &mut out);
        assert!((out[0][100] - 0.5 * std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3);
        // Monitored: the input replaces it, and it still sounds when stopped.
        s.track_mut(t).unwrap().mixer.input_monitor = true;
        eng.render(&s, 512, 512, &mut out);
        assert!((out[0][100] - 0.25 * std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3, "{}", out[0][100]);
        eng.monitor_only = true;
        eng.render(&s, 1024, 512, &mut out);
        assert!(out[0][100] > 0.1);
        s.track_mut(t).unwrap().mixer.input_monitor = false;
        eng.render(&s, 1024, 512, &mut out);
        assert!(out[0][100].abs() < 1e-6, "stopped, unmonitored: silence");
    }

    #[test]
    fn empty_session_is_silent() {
        let s = Session::default();
        let out = render_range(&s, Range::new(0, 1000), 128);
        assert!(out.iter().all(|c| c.iter().all(|x| *x == 0.0)));
    }

    #[test]
    fn stopped_monitoring_does_not_click() {
        let mut s = Session::default();
        s.edit.click = true;
        let mut eng = MixEngine::new(48_000.0, 512);
        eng.metronome = true;
        eng.monitor_only = true;
        let mut out = vec![vec![0.0f32; 512]; 2];
        for _ in 0..4 {
            eng.render(&s, 0, 512, &mut out);
            assert!(out.iter().all(|c| c.iter().all(|x| *x == 0.0)));
        }
    }

    /// Renders `len` frames from zero the way playback does (metronome on), in blocks of `block`.
    fn render_playback(s: &Session, len: usize, block: usize, recording: bool) -> Vec<Vec<f32>> {
        let mut eng = MixEngine::new(s.sample_rate.as_f64() as f32, block);
        eng.metronome = true;
        eng.recording = recording;
        let mut out = vec![vec![0.0f32; len]; 2];
        let mut tmp = vec![vec![0.0f32; block]; 2];
        let mut done = 0usize;
        while done < len {
            let n = (len - done).min(block);
            eng.render(s, done as i64, n, &mut tmp);
            for (o, t) in out.iter_mut().zip(tmp.iter()) {
                if let (Some(d), Some(src)) = (o.get_mut(done..done + n), t.get(..n)) {
                    d.copy_from_slice(src);
                }
            }
            done += n;
        }
        out
    }

    /// CL-T3: clicks across tempo changes with non-integer beat lengths (97.3 and 133.7 BPM at
    /// 44.1 kHz) and a meter change land on the exact sample the tempo map gives, and irregular,
    /// varying block sizes (block edges and interiors) render the identical signal.
    #[test]
    fn click_placement_is_exact_across_tempo_changes_and_irregular_blocks() {
        let mut s = Session { sample_rate: soundcraft_time::SampleRate::new(44_100).unwrap(), ..Session::default() };
        s.edit.click = true;
        let sr = s.sample_rate;
        let tpb = s.tempo.meter_at_tick(0).ticks_per_beat();
        s.tempo.set_tempo(0, 97.3).unwrap();
        s.tempo.set_tempo(4 * tpb, 133.7).unwrap();
        s.tempo.set_meter(8 * tpb, 7, 8).unwrap();
        let mut beats = Vec::new();
        let mut tick = 0i64;
        while beats.len() < 16 {
            beats.push(s.tempo.tick_to_samples(tick, sr));
            tick += s.tempo.meter_at_tick(tick).ticks_per_beat();
        }
        let len = usize::try_from(*beats.last().unwrap()).unwrap() + 4410;
        let reference = render_playback(&s, len, 512, false);
        // Irregular, varying callback sizes.
        let sizes = [64usize, 100, 37, 512, 1, 255, 128, 333];
        let mut eng = MixEngine::new(44_100.0, 512);
        eng.metronome = true;
        let mut out = vec![vec![0.0f32; len]; 2];
        let mut tmp = vec![vec![0.0f32; 512]; 2];
        let (mut done, mut k) = (0usize, 0usize);
        while done < len {
            let n = sizes[k % sizes.len()].min(len - done);
            k += 1;
            eng.render(&s, done as i64, n, &mut tmp);
            out[0][done..done + n].copy_from_slice(&tmp[0][..n]);
            out[1][done..done + n].copy_from_slice(&tmp[1][..n]);
            done += n;
        }
        assert_eq!(out, reference, "block partition never changes the click signal");
        // Each blip starts with sin(0) = 0, so its first non-zero sample is beat + 1.
        for (b, at) in beats.iter().enumerate() {
            let at = usize::try_from(*at).unwrap();
            assert!(out[0][at.saturating_sub(20)..=at].iter().all(|x| *x == 0.0), "silence before beat {b}");
            assert_ne!(out[0][at + 1], 0.0, "beat {b} onset exactly at sample {at}");
        }
    }

    /// Count Off: with the click off, beats before `click_until` click and nothing after;
    /// offline renders never click.
    #[test]
    fn countoff_clicks_only_before_the_record_point() {
        let s = Session::default();
        assert!(!s.edit.click);
        let beat = 24_000usize; // 120 BPM at 48 kHz
        let len = beat * 8;
        let mut eng = MixEngine::new(48_000.0, 512);
        eng.metronome = true;
        eng.click_until = (beat * 4) as i64;
        let mut out = vec![vec![0.0f32; len]; 2];
        let mut tmp = vec![vec![0.0f32; 512]; 2];
        let mut done = 0usize;
        while done < len {
            let n = (len - done).min(300);
            eng.render(&s, done as i64, n, &mut tmp);
            out[0][done..done + n].copy_from_slice(&tmp[0][..n]);
            done += n;
        }
        for b in 0..8usize {
            let peak = out[0][b * beat..b * beat + 400].iter().fold(0.0f32, |m, x| m.max(x.abs()));
            if b < 4 {
                assert!(peak > 0.1, "count-off beat {b}");
            } else {
                assert_eq!(peak, 0.0, "beat {b} at/after the record point is silent");
            }
        }
        assert!(render_range(&s, Range::new(0, len as i64), 512).iter().all(|c| c.iter().all(|x| *x == 0.0)));
    }

    #[test]
    fn click_sounds_on_every_beat_only_in_playback() {
        let mut s = Session::default();
        let beat = s.sample_rate.as_f64() as usize * 60 / 120; // 120 BPM quarter notes
        let len = beat * 8;
        s.edit.click = true;
        // Offline renders (bounces, stems) never print the metronome.
        let offline = render_range(&s, Range::new(0, len as i64), 512);
        assert!(offline.iter().all(|c| c.iter().all(|x| *x == 0.0)));

        let out = render_playback(&s, len, 512, false);
        for b in 0..8usize {
            let at = b * beat;
            let on: f32 = out[0][at..at + 200].iter().map(|x| x.abs()).fold(0.0, f32::max);
            let off: f32 = out[0][at + beat / 2..at + beat / 2 + 200].iter().map(|x| x.abs()).fold(0.0, f32::max);
            assert!(on > 0.1, "beat {b} should click, peak {on}");
            assert_eq!(off, 0.0, "between beats must stay silent");
            assert_eq!(out[0][at], out[1][at], "both channels get the click");
        }
        // Blocks that start mid-beat render the same clicks as a single pass.
        let split = render_playback(&s, len, 97, false);
        assert_eq!(out, split);
    }

    #[test]
    fn click_only_during_record_follows_recording() {
        let mut s = Session::default();
        s.edit.click = true;
        s.edit.set_flag("click.only_during_record", true);
        let idle = render_playback(&s, 48_000, 512, false);
        assert!(idle.iter().all(|c| c.iter().all(|x| *x == 0.0)));
        let rec = render_playback(&s, 48_000, 512, true);
        assert!(rec.iter().any(|c| c.iter().any(|x| x.abs() > 0.1)));
    }

    /// A session with one track of `fmt` playing a distinct DC level per channel (0.1, 0.2, …).
    fn multichannel_session(fmt: ChannelFormat, main: ChannelFormat, frames: usize) -> (Session, TrackId) {
        let mut s = Session::default();
        s.outputs[0].format = main;
        let t = s.add_track(TrackKind::Audio, fmt, None);
        let chans: Vec<Vec<f32>> = (0..fmt.channels()).map(|c| (0..frames).map(|i| 0.1 * (c + 1) as f32 + (i % 7) as f32 * 1e-3).collect()).collect();
        s.pool.insert(SourceId(901), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: chans })));
        let id = s.new_clip_id();
        s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "mc", SourceId(901), 0, 0, frames as i64));
        (s, t)
    }

    #[test]
    fn surround_track_passes_through_bit_exact() {
        let (s, _) = multichannel_session(ChannelFormat::Surround51, ChannelFormat::Surround51, 4800);
        let src = s.pool.get(SourceId(901)).unwrap().buffer.channels.clone();
        let out = render_range(&s, Range::new(0, 4800), 512);
        assert_eq!(out.len(), 6);
        for (o, x) in out.iter().zip(src.iter()) {
            assert_eq!(o, x, "5.1 → 5.1 must be bit-exact");
        }
    }

    #[test]
    fn surround_to_stereo_folds_down_itu() {
        let (s, _) = multichannel_session(ChannelFormat::Surround51, ChannelFormat::Stereo, 4800);
        let out = render_range(&s, Range::new(0, 4800), 512);
        assert_eq!(out.len(), 2);
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let x = |c: usize| 0.1 * (c + 1) as f32 + (1000 % 7) as f32 * 1e-3;
        // L R C LFE Ls Rs.
        let l = x(0) + h * x(2) + h * x(4);
        let r = x(1) + h * x(2) + h * x(5);
        assert!((out[0][1000] - l).abs() < 1e-5, "{} vs {l}", out[0][1000]);
        assert!((out[1][1000] - r).abs() < 1e-5, "{} vs {r}", out[1][1000]);
    }

    #[test]
    fn mono_track_on_surround_main_pans() {
        let (mut s, t) = session_with_dc(0.5, 4800);
        s.outputs[0].format = ChannelFormat::Surround51;
        // No surround pan: stereo behaviour on L/R.
        let out = render_range(&s, Range::new(0, 4800), 512);
        assert_eq!(out.len(), 6);
        let h = 0.5 * gains(0.0, PanLaw::Minus3).0;
        assert!((out[0][100] - h).abs() < 1e-6 && (out[1][100] - h).abs() < 1e-6);
        assert!(out[2..].iter().all(|c| c[100].abs() < 1e-7));
        // Front centre: the centre speaker only.
        s.track_mut(t).unwrap().mixer.surround = Some(SurroundPan::default());
        let out = render_range(&s, Range::new(0, 4800), 512);
        assert!((out[2][100] - 0.5).abs() < 1e-4, "{}", out[2][100]);
        assert!(out.iter().enumerate().all(|(c, x)| c == 2 || x[100].abs() < 1e-4));
        // Rear right with LFE: Rs plus the LFE send.
        s.track_mut(t).unwrap().mixer.surround = Some(SurroundPan { x: 1.0, y: -1.0, lfe_db: -6.0, ..SurroundPan::default() });
        let mut eng = MixEngine::new(48_000.0, 512);
        let mut buf = vec![vec![0.0; 512]; 6];
        eng.render(&s, 0, 512, &mut buf);
        assert!((buf[5][100] - 0.5).abs() < 1e-4 && (buf[3][100] - 0.5 * db_to_gain(-6.0)).abs() < 1e-4);
        assert_eq!(eng.main_meter.peaks.len(), 6);
        assert_eq!(eng.meters.get(&t).map(|m| m.peaks.len()), Some(1));
        // A stereo device buffer gets the first two channels.
        let mut two = vec![vec![1.0; 512]; 2];
        eng.render(&s, 512, 512, &mut two);
        assert!(two[0][10].abs() < 1e-6);
    }

    #[test]
    fn surround_busses_and_master_fader() {
        let (mut s, t) = multichannel_session(ChannelFormat::Surround51, ChannelFormat::Surround51, 4800);
        let bus = s.add_bus("Stem", ChannelFormat::Surround51);
        let aux = s.add_track(TrackKind::Aux, ChannelFormat::Surround51, Some("Stem"));
        s.track_mut(aux).unwrap().mixer.input = Route::Bus(bus);
        s.track_mut(t).unwrap().mixer.output = Route::Bus(bus);
        let m = s.add_track(TrackKind::Master, ChannelFormat::Surround51, None);
        s.track_mut(m).unwrap().mixer.volume_db = -6.0;
        let out = render_range(&s, Range::new(0, 4800), 512);
        let g = db_to_gain(-6.0);
        for (c, ch) in out.iter().enumerate() {
            let x = 0.1 * (c + 1) as f32 + (1000 % 7) as f32 * 1e-3;
            assert!((ch[1000] - x * g).abs() < 1e-5, "channel {c}: {} vs {}", ch[1000], x * g);
        }
        // A mono bus sums a stereo source like the old stereo bus did.
        let (mut s, t) = session_with_dc(0.5, 4800);
        let bus = s.add_bus("Mono", ChannelFormat::Mono);
        let aux = s.add_track(TrackKind::Aux, ChannelFormat::Mono, Some("M"));
        s.track_mut(aux).unwrap().mixer.input = Route::Bus(bus);
        s.track_mut(aux).unwrap().mixer.pan = vec![-1.0];
        s.track_mut(t).unwrap().mixer.output = Route::Bus(bus);
        let out = render_range(&s, Range::new(0, 4800), 512);
        assert!((out[0][1000] - 0.5 * gains(0.0, PanLaw::Minus3).0).abs() < 1e-6, "{}", out[0][1000]);
    }

    #[test]
    fn surround_upmix_and_downmix_between_layouts() {
        // 5.1 → 7.1: shared speakers map 1:1; 5.x surrounds pan between the 7.1 sides and rears.
        let (s, _) = multichannel_session(ChannelFormat::Surround51, ChannelFormat::Surround71, 2000);
        let out = render_range(&s, Range::new(0, 2000), 512);
        assert_eq!(out.len(), 8);
        let x = |c: usize| 0.1 * (c + 1) as f32 + (100 % 7) as f32 * 1e-3;
        for (c, ch) in out.iter().enumerate().take(4) {
            assert!((ch[100] - x(c)).abs() < 1e-6);
        }
        let e_ls = out[4][100].powi(2) + out[6][100].powi(2);
        assert!((e_ls - x(4).powi(2)).abs() < 1e-5, "Ls energy is preserved");
    }
}

/// A3 graph fixtures (MIXER_SLOT_SPEC SL-T, MIXER_MODES_SPEC M-T, CLOCK_LATENCY_SPEC CL-T4,
/// VISUAL_MIXER_SPEC VM-T2/T3) through the real render path. Test-only providers prove plumbing
/// and order; they are not sound models.
#[cfg(test)]
mod sml_graph_tests {
    use super::*;
    use soundcraft_audio_io::AudioBuffer;
    use soundcraft_model::{DedicatedSlot, Insert, MixProfile, ProfileKind, SendSlot, SourceAudio, SourceId, StereoPanMode};
    use std::sync::Arc;

    const N: usize = 4096;

    fn src(s: &mut Session, id: u64, chans: Vec<Vec<f32>>) -> SourceId {
        let sid = SourceId(id);
        s.pool.insert(sid, Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: chans })));
        sid
    }

    fn track(s: &mut Session, fmt: ChannelFormat, chans: Vec<Vec<f32>>, sid: u64) -> TrackId {
        let t = s.add_track(TrackKind::Audio, fmt, None);
        let id = src(s, sid, chans);
        let cid = s.new_clip_id();
        let len = N as i64;
        s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(cid, "x", id, 0, 0, len));
        t
    }

    fn ramp(k: f32) -> Vec<f32> {
        (0..N).map(|i| ((i as f32 * 0.0123 * k).sin()) * 0.25).collect()
    }

    fn render(s: &Session) -> Vec<Vec<f32>> {
        render_range(s, Range::new(0, N as i64), 256)
    }

    fn slot(id: u64, provider: Option<&str>, trim_db: f32) -> Option<DedicatedSlot> {
        Some(DedicatedSlot { id, provider: provider.map(Insert::new), bypass: false, trim_db })
    }

    fn bus(s: &mut Session, name: &str) -> (TrackId, BusId) {
        let b = s.add_bus(name, ChannelFormat::Stereo);
        let a = s.add_track(TrackKind::Aux, ChannelFormat::Stereo, Some(name));
        s.track_mut(a).unwrap().mixer.input = Route::Bus(b);
        (a, b)
    }

    /// R23: each dedicated slot publishes its input and output peak with the strip meter; trim
    /// shows as the difference, bypass as equal peaks, Tracking deactivation as inactive.
    #[test]
    fn sml_graph_slot_io_meters_report_slot_input_and_output() {
        let mut s = Session::default();
        let a = track(&mut s, ChannelFormat::Mono, vec![vec![0.5; N]], 1);
        let (bt, b) = bus(&mut s, "Vox");
        s.track_mut(a).unwrap().mixer.output = Route::Bus(b);
        s.track_mut(a).unwrap().mixer.slots.tape = slot(1, Some("test:identity"), -6.0);
        s.track_mut(bt).unwrap().mixer.slots.summing = slot(2, Some("test:identity"), 0.0);
        let m = s.add_track(TrackKind::Master, ChannelFormat::Stereo, None);
        s.track_mut(m).unwrap().mixer.slots.master_tape = slot(3, None, 0.0);
        let mut eng = MixEngine::new(48_000.0, 256);
        let mut out = vec![vec![0.0f32; 256]; 2];
        eng.render(&s, 0, 256, &mut out);
        let tape = eng.meters[&a].slots[0];
        assert!(tape.active);
        assert!((tape.input - 0.5).abs() < 1e-6, "{tape:?}");
        assert!((tape.output - 0.5 * db_to_gain(-6.0)).abs() < 1e-6, "trim visible: {tape:?}");
        assert_eq!(eng.meters[&a].slots[1], SlotIo::default(), "not configured on a track");
        let sum = eng.meters[&bt].slots[1];
        assert!(sum.active && sum.input > 0.0 && (sum.input - sum.output).abs() < 1e-6, "{sum:?}");
        let mt = eng.meters[&m].slots[2];
        assert!(mt.active && mt.input > 0.0 && mt.input == mt.output, "empty Master Tape slot passes: {mt:?}");
        // Bypass: input = output; Tracking deactivation: reported inactive.
        s.track_mut(a).unwrap().mixer.slots.tape.as_mut().unwrap().bypass = true;
        eng.render(&s, 256, 256, &mut out);
        let tape = eng.meters[&a].slots[0];
        assert!(tape.active && tape.input == tape.output, "{tape:?}");
        s.track_mut(a).unwrap().mixer.slots.tape.as_mut().unwrap().bypass = false;
        s.tracking_off.insert((a, SlotKind::Tape.instance_index()));
        eng.realtime_overrides = true;
        eng.render(&s, 512, 256, &mut out);
        assert!(!eng.meters[&a].slots[0].active);
    }

    /// CL05: with announced session changes the structure is re-checked exactly when announced;
    /// output is identical to the per-block check for the same session sequence.
    #[test]
    fn announced_session_changes_resync_the_graph() {
        let mut s = Session::default();
        track(&mut s, ChannelFormat::Mono, vec![ramp(1.0)], 1);
        s.add_track(TrackKind::Master, ChannelFormat::Stereo, None);
        let mut s2 = s.clone();
        track(&mut s2, ChannelFormat::Stereo, vec![ramp(0.3), ramp(0.7)], 2);
        let run = |signalled: bool, announce: bool| {
            let mut eng = MixEngine::new(48_000.0, 256);
            eng.set_session_changes_signalled(signalled);
            let mut out = vec![vec![0.0f32; 256]; 2];
            let mut all = Vec::new();
            for b in 0..8i64 {
                let cur = if b < 4 { &s } else { &s2 };
                if b == 4 && announce {
                    eng.session_changed();
                }
                eng.render(cur, b * 256, 256, &mut out);
                all.extend_from_slice(&out[0]);
            }
            all
        };
        let reference = run(false, false);
        assert_eq!(run(true, true), reference, "announced: identical to the per-block check");
        assert_ne!(run(true, false), reference, "unannounced change is (by contract) not picked up");
    }

    /// R01/H1: input monitoring hears exactly the track's input channel; a missing channel is
    /// silent (never another input); a one-input device feeds a stereo track as dual mono.
    #[test]
    fn monitoring_never_substitutes_another_input_channel() {
        let hear = |fmt: ChannelFormat, input: &str, device_channels: usize| -> Vec<f32> {
            let mut s = Session::default();
            let t = s.add_track(TrackKind::Audio, fmt, None);
            let tr = s.track_mut(t).unwrap();
            tr.mixer.input_monitor = true;
            tr.mixer.input = Route::Hardware(input.into());
            let mut eng = MixEngine::new(48_000.0, 256);
            eng.input = (0..device_channels).map(|c| vec![(c + 1) as f32 * 0.1; 256]).collect();
            let mut out = vec![vec![0.0f32; 256]; 2];
            eng.render(&s, 0, 256, &mut out);
            eng.strips[&t].buf.iter().map(|c| c[10]).collect()
        };
        assert_eq!(hear(ChannelFormat::Mono, "In 3", 4), vec![0.3f32], "input 3");
        assert_eq!(hear(ChannelFormat::Mono, "In 3", 2), vec![0.0f32], "missing input is silent");
        assert_eq!(hear(ChannelFormat::Stereo, "In 1-2", 1), vec![0.1f32, 0.1], "dual mono");
        assert_eq!(hear(ChannelFormat::Stereo, "In 2-3", 2), vec![0.2f32, 0.0], "second side missing");
    }

    /// SL-T1 / VM-T2: empty/Clean slots at unity, 100 % width and mixer view state are
    /// sample-identical to the same graph without them.
    #[test]
    fn sml_graph_slt1_empty_slots_and_views_are_identity() {
        let mut s = Session::default();
        let a = track(&mut s, ChannelFormat::Stereo, vec![ramp(1.0), ramp(1.7)], 1);
        let m = track(&mut s, ChannelFormat::Mono, vec![ramp(0.6)], 2);
        let (bt, b) = bus(&mut s, "Vox");
        s.track_mut(m).unwrap().mixer.output = Route::Bus(b);
        s.add_track(TrackKind::Master, ChannelFormat::Stereo, None);
        let base = render(&s);
        let mut t = s.clone();
        t.track_mut(a).unwrap().mixer.slots.tape = slot(1, None, 0.0);
        t.track_mut(bt).unwrap().mixer.slots.summing = slot(2, None, 0.0);
        t.track_mut(bt).unwrap().mixer.slots.master_tape = slot(3, None, 0.0);
        t.track_mut(a).unwrap().mixer.width = 1.0;
        t.edit.values.insert("mixer.view".into(), 2.0);
        t.edit.values.insert("visual.zoom".into(), 3.0);
        assert_eq!(render(&t), base, "bit-identical");
        // A loaded identity provider is invoked and still exact.
        t.track_mut(a).unwrap().mixer.slots.tape = slot(1, Some("test:identity"), 0.0);
        assert_eq!(render(&t), base);
    }

    /// SL-T2: noncommuting affine/gain fixtures prove Tape-before-inserts and bus Pre/Post
    /// Master Tape order; moving Pre/Post keeps the slot id/state.
    #[test]
    fn sml_graph_slt2_slot_order_is_sl02() {
        let mut s = Session::default();
        let dc: Vec<f32> = vec![0.25; N];
        let t = track(&mut s, ChannelFormat::Mono, vec![dc.clone()], 1);
        s.track_mut(t).unwrap().mixer.pan = vec![-1.0];
        s.track_mut(t).unwrap().mixer.slots.tape = slot(9, Some("test:affine:2:0.1"), 0.0);
        s.track_mut(t).unwrap().mixer.inserts[0] = Some(Insert::new("test:gain:-6"));
        let g = 10f32.powf(-6.0 / 20.0);
        let out = render(&s);
        let want = (2.0 * 0.25 + 0.1) * g; // tape (affine) first, then insert gain
        assert!((out[0][N / 2] - want).abs() < 1e-6, "{} vs {want} (reversed order would be {})", out[0][N / 2], 2.0 * 0.25 * g + 0.1);
        // Bus Master Tape: Post (default) applies after the fader; Pre before the inserts.
        let mut s = Session::default();
        let t = track(&mut s, ChannelFormat::Mono, vec![dc], 2);
        let (bt, b) = bus(&mut s, "Bus");
        s.track_mut(t).unwrap().mixer.output = Route::Bus(b);
        s.track_mut(t).unwrap().mixer.pan = vec![-1.0];
        {
            let tr = s.track_mut(bt).unwrap();
            tr.mixer.slots.master_tape = slot(5, Some("test:affine:2:0.1"), 0.0);
            tr.mixer.volume_db = -6.0;
        }
        let post = render(&s)[0][N / 2];
        assert!((post - ((0.25 * g) * 2.0 + 0.1)).abs() < 1e-6, "post: fader then tape, got {post}");
        s.track_mut(bt).unwrap().mixer.slots.master_tape_position = TapePosition::Pre;
        let pre = render(&s)[0][N / 2];
        assert!((pre - ((0.25 * 2.0 + 0.1) * g)).abs() < 1e-6, "pre: tape then fader, got {pre}");
        assert_eq!(s.track(bt).unwrap().mixer.slots.master_tape.as_ref().unwrap().id, 5, "same slot id after move");
    }

    /// SL-T2/SL-T7: hard-left main pan with an Independent hard-right send and a one-sided stereo
    /// source: the send applies exactly one pan; Follow copies the channel coefficients.
    #[test]
    fn sml_graph_slt7_independent_and_follow_send_pan() {
        let mut s = Session::default();
        let t = track(&mut s, ChannelFormat::Mono, vec![vec![0.5; N]], 1);
        let (_, b) = bus(&mut s, "FX");
        let dead = s.add_bus("dead", ChannelFormat::Stereo);
        {
            let tr = s.track_mut(t).unwrap();
            tr.mixer.pan = vec![-1.0];
            tr.mixer.output = Route::Bus(dead); // listen to the send only
            let mut snd = SendSlot::new(Route::Bus(b));
            snd.level_db = 0.0;
            snd.follow_main_pan = false;
            snd.pan = 1.0;
            tr.mixer.sends[0] = Some(snd);
        }
        let out = render(&s);
        assert_eq!(out[0][100], 0.0, "independent hard-right send: nothing on the left");
        assert!((out[1][100] - 0.5).abs() < 1e-6, "exactly one pan stage (no double -3 dB): {}", out[1][100]);
        // Follow: same coefficients as the channel (hard left).
        s.track_mut(t).unwrap().mixer.sends[0].as_mut().unwrap().follow_main_pan = true;
        let out = render(&s);
        assert!((out[0][100] - 0.5).abs() < 1e-6 && out[1][100] == 0.0, "{:?}", (out[0][100], out[1][100]));
        // Independent centre within 0.01 dB of the -3 coefficient.
        let tr = s.track_mut(t).unwrap();
        tr.mixer.sends[0].as_mut().unwrap().follow_main_pan = false;
        tr.mixer.sends[0].as_mut().unwrap().pan = 0.0;
        let out = render(&s);
        let db = 20.0 * (out[0][100] / 0.5).log10();
        assert!((db + 3.0103).abs() < 0.01, "{db}");
        // One-sided stereo source keeps its side through a centred stereo send.
        let mut s2 = Session::default();
        let st = track(&mut s2, ChannelFormat::Stereo, vec![vec![0.5; N], vec![0.0; N]], 2);
        let (_, b2) = bus(&mut s2, "FX");
        let dead2 = s2.add_bus("dead", ChannelFormat::Stereo);
        let tr = s2.track_mut(st).unwrap();
        tr.mixer.output = Route::Bus(dead2);
        let mut snd = SendSlot::new(Route::Bus(b2));
        snd.level_db = 0.0;
        tr.mixer.sends[0] = Some(snd);
        let out = render(&s2);
        assert!((out[0][100] - 0.5).abs() < 1e-6 && out[1][100] == 0.0, "no crossfeed: {:?}", (out[0][100], out[1][100]));
    }

    /// SL-T3 / CL-T4 / M-T5: known-delay providers on track Tape, bus Summing and inserts through
    /// parallel/nested routes stay aligned within 1 sample; bypass keeps compensation; an empty
    /// identity slot adds zero delay.
    /// R24/R25/R29 host work: an insert's sidechain bus feeds the plugin's secondary input.
    /// On an aux the detector is sample-accurate (the bus is complete before the aux runs); on
    /// an audio track it hears the bus one block late (documented, reported). The key is a kick
    /// impulse sent to a bus; the gated signal is DC, so the output is non-zero exactly where the
    /// gate is open. Without a sidechain the gate stays closed. Both graph versions.
    #[test]
    fn sidechain_bus_feeds_insert_exactly_on_aux_and_one_block_late_on_tracks() {
        for v2 in [false, true] {
            let mut s = Session::default();
            if v2 {
                s.mix_profile = MixProfile { graph_version: 2, ..s.mix_profile.clone() };
            }
            let block = 256usize;
            let mut kick = vec![0.0f32; N];
            kick[1000] = 1.0;
            kick[1001] = 1.0;
            kick[3000] = 1.0;
            let k = track(&mut s, ChannelFormat::Mono, vec![kick], 1);
            let (_, sc) = {
                let b = s.add_bus("SC", ChannelFormat::Stereo);
                (b, b)
            };
            // The kick goes to the SC bus only (no main output): a post-fader send at 0 dB.
            s.track_mut(k).unwrap().mixer.output = Route::None;
            s.track_mut(k).unwrap().mixer.sends[0] = Some(SendSlot { level_db: 0.0, ..SendSlot::new(Route::Bus(sc)) });
            // DC vocal through a gate on an aux (bus V) and on the track itself (second source).
            let vox = track(&mut s, ChannelFormat::Mono, vec![vec![0.5; N]], 2);
            let (va, vb) = bus(&mut s, "V");
            s.track_mut(vox).unwrap().mixer.output = Route::Bus(vb);
            s.track_mut(vox).unwrap().mixer.pan = vec![-1.0];
            let mut gate = Insert::new("test:sidegate:-6");
            gate.sidechain = Some(sc);
            s.track_mut(va).unwrap().mixer.inserts[0] = Some(gate.clone());
            s.track_mut(va).unwrap().mixer.pan = vec![-1.0, -1.0];
            let dc2 = track(&mut s, ChannelFormat::Mono, vec![vec![0.5; N]], 3);
            s.track_mut(dc2).unwrap().mixer.pan = vec![1.0];
            s.track_mut(dc2).unwrap().mixer.inserts[0] = Some(gate);
            let out = render_range(&s, Range::new(0, N as i64), block);
            let open_l: Vec<usize> = out[0].iter().enumerate().filter(|(_, x)| x.abs() > 1e-4).map(|(i, _)| i).collect();
            let open_r: Vec<usize> = out[1].iter().enumerate().filter(|(_, x)| x.abs() > 1e-4).map(|(i, _)| i).collect();
            assert_eq!(open_l, vec![1000, 1001, 3000], "v2 {v2}: aux gate opens exactly at the key");
            assert_eq!(open_r, vec![1000 + block, 1001 + block, 3000 + block], "v2 {v2}: track gate hears the bus one block late");
            // No sidechain: closed.
            s.track_mut(va).unwrap().mixer.inserts[0].as_mut().unwrap().sidechain = None;
            let out = render_range(&s, Range::new(0, N as i64), block);
            assert!(out[0].iter().all(|x| x.abs() < 1e-6), "v2 {v2}: without a sidechain the gate is closed");
        }
    }

    /// Offline tail handling (R31): a processor that keeps ringing after the range end is cut
    /// without a tail and complete with `Tail::Auto` (its declared tail) or an explicit length.
    #[test]
    fn offline_render_tail_is_explicit_and_auto() {
        let mut s = Session::default();
        let mut imp = vec![0.0f32; N];
        imp[N - 1] = 1.0;
        let a = track(&mut s, ChannelFormat::Mono, vec![imp], 1);
        s.track_mut(a).unwrap().mixer.inserts[0] = Some(Insert::new("test:tail:300"));
        let r = render_range_report(&s, Range::new(0, N as i64), 256, Tail::None);
        assert_eq!(r.audio[0].len(), N);
        assert_eq!(r.tail_samples, 0);
        assert!(r.failed_plugins.is_empty());
        let auto = render_range_report(&s, Range::new(0, N as i64), 256, Tail::Auto);
        assert_eq!(auto.tail_samples, 300);
        assert_eq!(auto.audio[0].len(), N + 300);
        assert!(auto.audio[0][N - 1 + 300].abs() > 0.1, "the echo lands inside the tail");
        let fixed = render_range_report(&s, Range::new(0, N as i64), 256, Tail::Samples(100));
        assert_eq!(fixed.audio[0].len(), N + 100);
        assert!(fixed.audio[0][N..].iter().all(|x| x.abs() < 1e-6), "a shorter tail cuts the echo");
    }

    /// Insert PDC fixtures (0/1/63/128/1024 samples): parallel tracks, a nested bus path and
    /// a bypassed insert all stay aligned within one sample; ordinary bypass keeps the
    /// plugin's latency as a plain delay and passes the signal.
    #[test]
    fn insert_pdc_fixtures_parallel_nested_and_bypassed() {
        let argmax = |c: &[f32]| c.iter().enumerate().max_by(|x, y| x.1.abs().total_cmp(&y.1.abs())).map(|(i, _)| i).unwrap();
        for d in [0usize, 1, 63, 128, 1024] {
            let mut s = Session::default();
            let mut imp = vec![0.0f32; N];
            imp[500] = 1.0;
            let a = track(&mut s, ChannelFormat::Mono, vec![imp.clone()], 1);
            let c = track(&mut s, ChannelFormat::Mono, vec![imp.clone()], 2);
            s.track_mut(a).unwrap().mixer.pan = vec![-1.0];
            s.track_mut(c).unwrap().mixer.pan = vec![1.0];
            s.track_mut(a).unwrap().mixer.inserts[0] = Some(Insert::new(format!("test:delay:{d}")));
            let len = (N + 2048) as i64;
            let out = render_range(&s, Range::new(0, len), 256);
            assert!((argmax(&out[0]) as i64 - argmax(&out[1]) as i64).abs() <= 1, "parallel delay {d}: {} vs {}", argmax(&out[0]), argmax(&out[1]));
            // Bypassed: still aligned, and the signal passes (peak equals the undelayed track's).
            s.track_mut(a).unwrap().mixer.inserts[0].as_mut().unwrap().bypass = true;
            let out = render_range(&s, Range::new(0, len), 256);
            assert!((argmax(&out[0]) as i64 - argmax(&out[1]) as i64).abs() <= 1, "bypassed delay {d}: {} vs {}", argmax(&out[0]), argmax(&out[1]));
            assert!((out[0][argmax(&out[0])] - out[1][argmax(&out[1])]).abs() < 1e-4, "bypass passes the signal");
            // Nested: the delayed insert on an aux fed by a bus, against a direct track.
            let mut s = Session::default();
            let a = track(&mut s, ChannelFormat::Mono, vec![imp.clone()], 1);
            let c = track(&mut s, ChannelFormat::Mono, vec![imp.clone()], 2);
            let (bt, b) = bus(&mut s, "Nest");
            s.track_mut(a).unwrap().mixer.output = Route::Bus(b);
            s.track_mut(a).unwrap().mixer.pan = vec![-1.0];
            s.track_mut(bt).unwrap().mixer.pan = vec![-1.0, -1.0];
            s.track_mut(c).unwrap().mixer.pan = vec![1.0];
            s.track_mut(bt).unwrap().mixer.inserts[0] = Some(Insert::new(format!("test:delay:{d}")));
            s.track_mut(bt).unwrap().mixer.inserts[1] = Some(Insert::new("test:delay:63"));
            let out = render_range(&s, Range::new(0, len), 256);
            assert!((argmax(&out[0]) as i64 - argmax(&out[1]) as i64).abs() <= 1, "nested delay {d}: {} vs {}", argmax(&out[0]), argmax(&out[1]));
        }
    }

    #[test]
    fn sml_graph_clt4_compensation_through_slots() {
        for d in [0usize, 1, 63, 128, 1024] {
            let mut s = Session::default();
            let mut imp = vec![0.0f32; N];
            imp[500] = 1.0;
            let a = track(&mut s, ChannelFormat::Mono, vec![imp.clone()], 1);
            let bt_src = track(&mut s, ChannelFormat::Mono, vec![imp], 2);
            s.track_mut(a).unwrap().mixer.pan = vec![-1.0];
            s.track_mut(bt_src).unwrap().mixer.pan = vec![1.0];
            if d > 0 {
                s.track_mut(a).unwrap().mixer.slots.tape = slot(1, Some(&format!("test:delay:{d}")), 0.0);
            }
            let argmax = |c: &[f32]| c.iter().enumerate().max_by(|x, y| x.1.abs().total_cmp(&y.1.abs())).map(|(i, _)| i).unwrap();
            let len = (N + 2048) as i64;
            let out = render_range(&s, Range::new(0, len), 256);
            assert!((argmax(&out[0]) as i64 - argmax(&out[1]) as i64).abs() <= 1, "delay {d}: L {} R {}", argmax(&out[0]), argmax(&out[1]));
            if d > 0 {
                // Bypass keeps the compensation delay.
                s.track_mut(a).unwrap().mixer.slots.tape.as_mut().unwrap().bypass = true;
                let out = render_range(&s, Range::new(0, len), 256);
                assert_eq!(argmax(&out[0]), argmax(&out[1]), "bypassed delay {d}");
            }
        }
        // Nested: delay on a bus Summing slot vs a direct track.
        let mut s = Session::default();
        let mut imp = vec![0.0f32; N];
        imp[300] = 1.0;
        let a = track(&mut s, ChannelFormat::Mono, vec![imp.clone()], 1);
        let c = track(&mut s, ChannelFormat::Mono, vec![imp], 2);
        let (bt, b) = bus(&mut s, "Vox");
        s.track_mut(a).unwrap().mixer.output = Route::Bus(b);
        s.track_mut(a).unwrap().mixer.pan = vec![-1.0];
        s.track_mut(c).unwrap().mixer.pan = vec![1.0];
        s.track_mut(bt).unwrap().mixer.slots.summing = slot(2, Some("test:delay:128"), 0.0);
        let out = render_range(&s, Range::new(0, (N + 2048) as i64), 256);
        let argmax = |c: &[f32]| c.iter().enumerate().max_by(|x, y| x.1.abs().total_cmp(&y.1.abs())).map(|(i, _)| i).unwrap();
        // Aux latency comes from the previous block (documented); after the first block it is stable.
        assert!(
            (argmax(&out[0]) as i64 - argmax(&out[1]) as i64).abs() <= 1,
            "bus summing delay compensated: {} vs {}",
            argmax(&out[0]),
            argmax(&out[1])
        );
    }

    /// M-T1: 64 inputs summed on a bus (graph v2, f64 accumulation) vs an f64 reference: peak
    /// error ≤ 1e-6 for normalised output ≤ 1, no intermediate clipping above 0 dBFS.
    #[test]
    fn sml_graph_mt1_f64_bus_sum() {
        let mut s = Session::default();
        assert_eq!(s.mix_profile.graph_version, 2);
        let (_, b) = bus(&mut s, "Sum");
        let mut reference = vec![0.0f64; N];
        let mut seed = 7u32;
        for k in 0..64u64 {
            let data: Vec<f32> = (0..N)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    (seed as f32 / u32::MAX as f32 - 0.5) / 32.0
                })
                .collect();
            for (r, x) in reference.iter_mut().zip(&data) {
                *r += f64::from(*x);
            }
            let t = track(&mut s, ChannelFormat::Mono, vec![data], 10 + k);
            s.track_mut(t).unwrap().mixer.pan = vec![-1.0];
            s.track_mut(t).unwrap().mixer.output = Route::Bus(b);
        }
        let out = render(&s);
        let err = out[0].iter().zip(&reference).map(|(a, r)| (f64::from(*a) - r).abs()).fold(0.0, f64::max);
        assert!(err <= 1e-6, "peak error {err:e}");
        // Above 0 dBFS internally: a +12 dB bus sum is not clipped before the fader.
        let mut s = Session::default();
        let (bt, b) = bus(&mut s, "Hot");
        for k in 0..4 {
            let t = track(&mut s, ChannelFormat::Mono, vec![vec![0.9; N]], 100 + k);
            s.track_mut(t).unwrap().mixer.pan = vec![-1.0];
            s.track_mut(t).unwrap().mixer.output = Route::Bus(b);
        }
        s.track_mut(bt).unwrap().mixer.volume_db = -12.0;
        let out = render(&s);
        let want = 3.6 * 10f32.powf(-12.0 / 20.0);
        assert!((out[0][N / 2] - want).abs() < 1e-5, "{} vs {want}", out[0][N / 2]);
    }

    /// M-T2: Logic-style profile pans with the published SML coefficients (0.01 dB), stereo
    /// Balance has no crossfeed, dual stereo Pan routes each side, neutral buses are identity.
    #[test]
    fn sml_graph_mt2_logic_profile_pan() {
        let mut s = Session::default();
        s.mix_profile = MixProfile { kind: ProfileKind::LogicStyle, ..s.mix_profile.clone() };
        let t = track(&mut s, ChannelFormat::Mono, vec![vec![0.5; N]], 1);
        for (p, law) in [(0.0f32, "-3db"), (0.5, "-3db"), (0.0, "0db"), (-0.5, "0db"), (0.0, "-3db_compensated"), (1.0, "-3db")] {
            s.mix_profile.law = law.into();
            s.track_mut(t).unwrap().mixer.pan = vec![p];
            let out = render(&s);
            let (l, r) = soundcraft_dsp::mixmath::mono_pan(f64::from(p), soundcraft_dsp::mixmath::ProfileLaw::parse(law).unwrap());
            for (got, want) in [(out[0][100], l), (out[1][100], r)] {
                if want == 0.0 {
                    assert_eq!(got, 0.0, "exact zero at {p} {law}");
                } else {
                    let db = 20.0 * (f64::from(got) / (0.5 * want)).log10();
                    assert!(db.abs() < 0.01, "{p} {law}: {db} dB");
                }
            }
        }
        // Stereo balance: one-sided input, no crossfeed.
        let mut s2 = Session::default();
        s2.mix_profile = MixProfile { kind: ProfileKind::LogicStyle, ..s2.mix_profile.clone() };
        let st = track(&mut s2, ChannelFormat::Stereo, vec![vec![0.5; N], vec![0.0; N]], 2);
        s2.track_mut(st).unwrap().mixer.pan = vec![-0.5, 0.5];
        let out = render(&s2);
        assert_eq!((out[0][100], out[1][100]), (0.5, 0.0), "centred balance passes L, no crossfeed");
        s2.mix_profile.stereo_mode = StereoPanMode::StereoPan;
        s2.track_mut(st).unwrap().mixer.pan = vec![1.0, 1.0];
        let out = render(&s2);
        assert_eq!(out[0][100], 0.0, "stereo pan routes the left input hard right");
        assert!((out[1][100] - 0.5).abs() < 1e-6);
        // Neutral nested bus (Aux at default full width) adds no gain under compensated laws.
        let mut s3 = Session::default();
        s3.mix_profile =
            MixProfile { kind: ProfileKind::LogicStyle, law: "-3db_compensated".into(), balance_compensated: true, ..s3.mix_profile.clone() };
        let m = track(&mut s3, ChannelFormat::Stereo, vec![vec![0.3; N], vec![0.2; N]], 3);
        let direct = render(&s3);
        let (_, b) = bus(&mut s3, "Vox");
        let (bt2, b2) = bus(&mut s3, "Mix");
        s3.track_mut(m).unwrap().mixer.output = Route::Bus(b);
        let vox = s3.tracks.iter().find(|x| x.name == "Vox").unwrap().id;
        s3.track_mut(vox).unwrap().mixer.output = Route::Bus(b2);
        let _ = bt2;
        let nested = render(&s3);
        assert!((nested[0][N / 2] - direct[0][N / 2]).abs() < 1e-6 && (nested[1][N / 2] - direct[1][N / 2]).abs() < 1e-6, "no +3 dB per bus level");
    }

    /// VM-T3 at the engine: width processes once, before the pre-fader send tap; 100 % exact.
    #[test]
    fn sml_graph_vmt3_width_in_graph() {
        let mut s = Session::default();
        let t = track(&mut s, ChannelFormat::Stereo, vec![vec![0.5; N], vec![0.1; N]], 1);
        let base = render(&s);
        s.track_mut(t).unwrap().mixer.width = 0.0;
        let out = render(&s);
        // Settled after the first block's ramp: dual mono (0.3, 0.3).
        assert!((out[0][N - 1] - 0.3).abs() < 1e-6 && (out[1][N - 1] - 0.3).abs() < 1e-6, "{:?}", (out[0][N - 1], out[1][N - 1]));
        s.track_mut(t).unwrap().mixer.width = 1.0;
        assert_eq!(render(&s), base, "fresh render at 100 % is identity");
        // Pre-fader send sees the widened signal.
        let (_, b) = bus(&mut s, "FX");
        let dead = s.add_bus("dead", ChannelFormat::Stereo);
        let tr = s.track_mut(t).unwrap();
        tr.mixer.width = 2.0;
        tr.mixer.volume_db = -144.0;
        tr.mixer.output = Route::Bus(dead);
        let mut snd = SendSlot::new(Route::Bus(b));
        snd.level_db = 0.0;
        snd.pre_fader = true;
        tr.mixer.sends[0] = Some(snd);
        let out = render(&s);
        assert!((out[0][N - 1] - 0.7).abs() < 1e-6 && (out[1][N - 1] + 0.1).abs() < 1e-6, "{:?}", (out[0][N - 1], out[1][N - 1]));
    }

    /// T09 / T-T4: the Listen mask silences every source outside it, keeps the buses and returns
    /// the audited source feeds open, lets an explicit mute win, and never reaches an offline
    /// render (the mask lives on the realtime engine, not in the document).
    #[test]
    fn listen_mask_gates_sources_only_on_the_realtime_engine() {
        let mut s = Session::default();
        let a = track(&mut s, ChannelFormat::Mono, vec![vec![0.5; N]], 1);
        let u = track(&mut s, ChannelFormat::Mono, vec![vec![0.25; N]], 2);
        let (_, b) = bus(&mut s, "Vox");
        for t in [a, u] {
            let tr = s.track_mut(t).unwrap();
            tr.mixer.pan = vec![-1.0];
            tr.mixer.output = Route::Bus(b);
        }
        s.add_track(TrackKind::Master, ChannelFormat::Stereo, None);
        let ia = s.track_index(a).unwrap();
        let mut eng = MixEngine::new(48_000.0, 256);
        eng.parallel_strips = false;
        let mut out = vec![vec![0.0f32; 256]; 2];
        eng.render(&s, 0, 256, &mut out);
        assert!((out[0][100] - 0.75).abs() < 1e-6, "both sources through the bus: {}", out[0][100]);
        let mut mask = soundcraft_model::ListenMask::default();
        mask.set(ia);
        mask.active = true;
        eng.listen = mask;
        eng.render(&s, 256, 256, &mut out);
        assert!((out[0][100] - 0.5).abs() < 1e-6, "only the audited source, its bus open: {}", out[0][100]);
        s.track_mut(a).unwrap().mixer.mute = true;
        eng.render(&s, 512, 256, &mut out);
        assert!(out[0][100].abs() < 1e-6, "an explicit mute still wins: {}", out[0][100]);
        s.track_mut(a).unwrap().mixer.mute = false;
        // The document carries no mask: an offline render of the same session hears everything.
        assert!((render(&s)[0][100] - 0.75).abs() < 1e-6);
        eng.listen = soundcraft_model::ListenMask::default();
        eng.render(&s, 768, 256, &mut out);
        assert!((out[0][100] - 0.75).abs() < 1e-6, "inactive mask: everything again");
    }

    /// B policy (v2): channel mute keeps pre-fader sends, gates post-fader sends; solo of one
    /// source opens its return without admitting an unrelated source sharing it; solo-safe.
    #[test]
    fn sml_graph_mute_solo_policy() {
        let mut s = Session::default();
        let a = track(&mut s, ChannelFormat::Mono, vec![vec![0.5; N]], 1);
        let u = track(&mut s, ChannelFormat::Mono, vec![vec![0.25; N]], 2);
        let (ret, b) = bus(&mut s, "Verb");
        let dead = s.add_bus("dead", ChannelFormat::Stereo);
        for t in [a, u] {
            let tr = s.track_mut(t).unwrap();
            tr.mixer.pan = vec![-1.0];
            tr.mixer.output = Route::Bus(dead);
            let mut snd = SendSlot::new(Route::Bus(b));
            snd.level_db = 0.0;
            snd.pre_fader = true;
            tr.mixer.sends[0] = Some(snd);
        }
        assert!((render(&s)[0][100] - 0.75).abs() < 1e-6);
        s.track_mut(a).unwrap().mixer.mute = true;
        assert!((render(&s)[0][100] - 0.75).abs() < 1e-6, "muted source keeps its pre-fader send");
        s.track_mut(a).unwrap().mixer.sends[0].as_mut().unwrap().pre_fader = false;
        assert!((render(&s)[0][100] - 0.25).abs() < 1e-6, "post-fader send follows mute");
        s.track_mut(a).unwrap().mixer.mute = false;
        s.track_mut(a).unwrap().mixer.solo = true;
        let out = render(&s);
        assert!((out[0][100] - 0.5).abs() < 1e-6, "solo A: return open, unrelated U not admitted ({})", out[0][100]);
        s.track_mut(u).unwrap().mixer.solo_safe = true;
        assert!((render(&s)[0][100] - 0.75).abs() < 1e-6, "solo-safe U stays eligible");
        s.track_mut(u).unwrap().mixer.solo_safe = false;
        s.track_mut(a).unwrap().mixer.solo = false;
        s.track_mut(ret).unwrap().mixer.solo = true;
        assert!((render(&s)[0][100] - 0.75).abs() < 1e-6, "soloing the bus admits its contributors");
    }

    /// SL-T4 (engine side) + SL05: unknown provider = named placeholder passing audio; audition
    /// overrides are realtime-only and never reach offline renders/exports.
    #[test]
    fn sml_graph_slt4_missing_provider_and_audition_isolation() {
        let mut s = Session::default();
        let t = track(&mut s, ChannelFormat::Mono, vec![vec![0.5; N]], 1);
        let base = render(&s);
        s.track_mut(t).unwrap().mixer.slots.tape = slot(4, Some("vst3:00000000000000000000000000000000"), 0.0);
        assert_eq!(render(&s), base, "missing provider passes audio (placeholder)");
        assert!(is_missing_plugin("vst3:00000000000000000000000000000000"));
        s.track_mut(t).unwrap().mixer.slots.tape = slot(4, Some("test:gain:-6"), 0.0);
        let normal = render(&s);
        s.audition = AuditionBypass { tape: true, summing: true, inserts: true };
        assert_eq!(render(&s), normal, "offline render ignores the session's audition flags");
        let mut eng = MixEngine::new(48_000.0, 256);
        eng.audition = AuditionBypass { tape: true, ..AuditionBypass::default() };
        let mut out = vec![vec![0.0; 256]; 2];
        eng.render(&s, 0, 256, &mut out);
        assert!((out[0][100] - base[0][100]).abs() < 1e-6, "realtime audition bypass removes the tape slot");
        // Slot trim alone is visible processing.
        s.track_mut(t).unwrap().mixer.slots.tape = slot(4, None, -6.0);
        assert!((render(&s)[0][100] / base[0][100] - 10f32.powf(-6.0 / 20.0)).abs() < 1e-5);
    }
}

#[cfg(test)]
mod sml_post_insert_tests {
    use super::*;
    use soundcraft_audio_io::AudioBuffer;
    use soundcraft_model::{Insert, SourceAudio, SourceId};
    use std::sync::Arc;

    /// SL-T2 Post inserts: a noncommuting affine Post insert runs after the fader; a Pre one
    /// before it; legacy v1 sessions ignore the flag (all inserts pre-fader as before).
    #[test]
    fn sml_graph_post_inserts_follow_the_fader() {
        let mut s = Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        s.pool.insert(SourceId(1), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![vec![0.25; 2048]] })));
        let cid = s.new_clip_id();
        let tr = s.track_mut(t).unwrap();
        tr.playlist_mut().unwrap().clips.push(Clip::audio(cid, "dc", SourceId(1), 0, 0, 2048));
        tr.mixer.pan = vec![-1.0];
        tr.mixer.volume_db = -6.0;
        let mut ins = Insert::new("test:affine:2:0.1");
        ins.post_fader = true;
        tr.mixer.inserts[0] = Some(ins);
        let g = 10f32.powf(-6.0 / 20.0);
        let v = render_range(&s, Range::new(0, 2048), 256)[0][1000];
        assert!((v - (0.25 * g * 2.0 + 0.1)).abs() < 1e-6, "post: {v}");
        s.track_mut(t).unwrap().mixer.inserts[0].as_mut().unwrap().post_fader = false;
        let v = render_range(&s, Range::new(0, 2048), 256)[0][1000];
        assert!((v - ((0.25 * 2.0 + 0.1) * g)).abs() < 1e-6, "pre: {v}");
        s.track_mut(t).unwrap().mixer.inserts[0].as_mut().unwrap().post_fader = true;
        s.mix_profile.graph_version = 1;
        let v = render_range(&s, Range::new(0, 2048), 256)[0][1000];
        assert!((v - ((0.25 * 2.0 + 0.1) * g)).abs() < 1e-6, "legacy keeps pre-fader order: {v}");
    }

    /// CL05: serial and parallel strip rendering produce bit-identical audio for a session with
    /// enough hosted processors to fan out.
    #[test]
    fn serial_and_parallel_strips_render_identically() {
        let mut s = Session::default();
        for k in 0..6u64 {
            let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
            s.pool.insert(
                SourceId(10 + k),
                Arc::new(SourceAudio::new(AudioBuffer {
                    sample_rate: 48_000,
                    channels: vec![(0..4096).map(|i| ((i as f32) * 0.01 * (k as f32 + 1.0)).sin() * 0.2).collect()],
                })),
            );
            let cid = s.new_clip_id();
            let tr = s.track_mut(t).unwrap();
            tr.playlist_mut().unwrap().clips.push(Clip::audio(cid, "c", SourceId(10 + k), 0, 0, 4096));
            tr.mixer.inserts[0] = Some(Insert::new(format!("test:gain:-{k}")));
        }
        let render = |parallel: bool| {
            let mut eng = MixEngine::new(48_000.0, 256);
            eng.parallel_strips = parallel;
            let mut all = Vec::new();
            let mut out = vec![vec![0.0f32; 256]; 2];
            for b in 0..16 {
                eng.render(&s, b * 256, 256, &mut out);
                all.extend_from_slice(&out[0]);
            }
            all
        };
        const { assert!(PARALLEL_MIN_HEAVY_STRIPS <= 6) };
        assert_eq!(render(true), render(false));
    }

    /// CL04: a compensation-latency change during recording raises one event; a latent plugin
    /// present when the take starts does not; nothing fires while not recording.
    #[test]
    fn latency_change_while_recording_raises_an_event() {
        let mut s = Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        s.track_mut(t).unwrap().mixer.inserts[0] = Some(Insert::new("test:delay:64"));
        let mut eng = MixEngine::new(48_000.0, 256);
        let mut out = vec![vec![0.0f32; 256]; 2];
        eng.render(&s, 0, 256, &mut out);
        eng.recording = true;
        eng.render(&s, 256, 256, &mut out);
        eng.render(&s, 512, 256, &mut out);
        assert!(!eng.take_latency_event(), "steady latency at take start is fine");
        s.track_mut(t).unwrap().mixer.inserts[1] = Some(Insert::new("test:delay:128"));
        eng.render(&s, 768, 256, &mut out);
        assert!(eng.take_latency_event(), "latency changed mid-take");
        assert!(!eng.take_latency_event(), "reported once");
        eng.recording = false;
        s.track_mut(t).unwrap().mixer.inserts[1] = None;
        eng.render(&s, 1024, 256, &mut out);
        assert!(!eng.take_latency_event());
    }
}
