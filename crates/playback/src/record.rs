//! Audio input capture for recording.
//!
//! A [`Recorder`] opens an input device (the negotiated one from a [`DeviceRequest`], or the
//! host default) and, in its callback, only (1) stamps the block on the stream clock, (2) feeds
//! the monitoring ring and (3) offers the block to a preallocated lock-free [`CaptureRing`]. A
//! take is written to disk by a writer thread ([`crate::capture`]); no take is held in RAM by the
//! callback and nothing in the callback allocates, locks or touches files.

use crate::capture::{ActiveCapture, CaptureReport, CaptureRing, CaptureState, WriterConfig};
use crate::clock::StreamClock;
use crate::device::{DeviceRequest, NegotiatedConfig};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Captured audio: planar f32 at `sample_rate`, plus the durable media file it came from.
#[derive(Debug, Clone, Default)]
pub struct Take {
    pub sample_rate: u32,
    pub channels: Vec<Vec<f32>>,
    /// The finalized dry capture file (never overwritten).
    pub media: Option<PathBuf>,
    /// Capture outcome (frames, partial/failed state, stream provenance).
    pub report: Option<CaptureReport>,
}

/// Stream statistics shared with the UI (written by the callback with atomics only).
#[derive(Debug, Default)]
pub struct StreamStats {
    pub generation: AtomicU64,
    pub frames: AtomicU64,
    pub min_block: AtomicU32,
    pub max_block: AtomicU32,
    /// Input stream failure (CL-T2 disconnect): 0 none, 1 device not available, 2 other backend
    /// error. Set from the stream's error callback (one atomic store, realtime-safe).
    pub lost: std::sync::atomic::AtomicU8,
}

impl StreamStats {
    /// Records an input stream failure (first one wins).
    pub fn mark_lost(&self, device_gone: bool) {
        let _ = self.lost.compare_exchange(0, if device_gone { 1 } else { 2 }, Ordering::AcqRel, Ordering::Acquire);
    }

    /// Why the input stream stopped, if it did.
    pub fn lost_reason(&self) -> Option<&'static str> {
        match self.lost.load(Ordering::Acquire) {
            0 => None,
            1 => Some("the input device is no longer available"),
            _ => Some("the input driver reported an error"),
        }
    }
}

pub struct Recorder {
    /// Live input for monitoring (always fed while the input stream runs).
    pub monitor: Arc<InputRing>,
    /// Capture ring reused for every take of this stream.
    pub ring: Arc<CaptureRing>,
    pub stats: Arc<StreamStats>,
    active: Mutex<Option<ActiveCapture>>,
    #[cfg(not(target_os = "freebsd"))]
    _stream: Option<cpal::Stream>,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: usize,
    /// The negotiated configuration when one was requested (None = host default device).
    pub config: Option<NegotiatedConfig>,
    /// Backend capture instant of the current take's first frame (CL04 placement).
    pub first_capture: Arc<Mutex<Option<cpal::StreamInstant>>>,
}

/// Seconds of capture ring headroom (the writer must stay within this of the callback).
pub const RING_SECONDS: f64 = 4.0;

impl Recorder {
    /// Open the default input device. Errors when there is none (the caller shows a message).
    pub fn open() -> Result<Recorder, String> {
        Recorder::open_with(None)
    }

    /// Open the requested input device/rate/buffer exactly (or the default device with `None`).
    /// A request the device cannot honour is an error, never a silent substitution.
    #[cfg(not(target_os = "freebsd"))]
    pub fn open_with(req: Option<&DeviceRequest>) -> Result<Recorder, String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let (device, config, negotiated) = match req {
            Some(r) => {
                let caps = crate::device::enumerate(crate::device::Direction::Input);
                let n = crate::device::negotiate(r, &caps).map_err(|e| e.to_string())?;
                let d = crate::device::find_device(&n, crate::device::Direction::Input)?;
                (d, crate::device::stream_config(&n), Some(n))
            }
            None => {
                let host = cpal::default_host();
                let device = host.default_input_device().ok_or("no audio input device")?;
                let cfg = device.default_input_config().map_err(|e| e.to_string())?;
                if cfg.sample_format() != cpal::SampleFormat::F32 {
                    return Err(format!("unsupported input sample format {:?}", cfg.sample_format()));
                }
                (device, cfg.config(), None)
            }
        };
        #[allow(deprecated)]
        let name = device.name().unwrap_or_else(|_| "Input".into());
        let channels = usize::from(config.channels).max(1);
        let rate = config.sample_rate.0;
        let monitor = InputRing::new(channels, rate);
        let ring = CaptureRing::new(channels, (f64::from(rate) * RING_SECONDS) as usize);
        let stats = Arc::new(StreamStats::default());
        let (mon, rg, st) = (Arc::clone(&monitor), Arc::clone(&ring), Arc::clone(&stats));
        let st_err = Arc::clone(&stats);
        let first_capture: Arc<Mutex<Option<cpal::StreamInstant>>> = Arc::new(Mutex::new(None));
        let fc = Arc::clone(&first_capture);
        let mut clock = StreamClock::new(rate);
        st.generation.store(clock.generation(), Ordering::Release);
        st.min_block.store(u32::MAX, Ordering::Release);
        let stream = device
            .build_input_stream(
                &config,
                move |data: &[f32], info: &cpal::InputCallbackInfo| {
                    crate::mark_audio_thread();
                    let stamp = clock.advance(data.len() / channels);
                    mon.push(data);
                    let first = rg.accepted_frames() == 0;
                    if rg.push(data, stamp.generation, stamp.stream_frame)
                        && first
                        && let Ok(mut f) = fc.try_lock()
                    {
                        *f = Some(info.timestamp().capture);
                    }
                    st.frames.store(clock.frames(), Ordering::Release);
                    st.min_block.fetch_min(stamp.frames, Ordering::AcqRel);
                    st.max_block.fetch_max(stamp.frames, Ordering::AcqRel);
                },
                move |e| {
                    // Possibly the audio thread (see `crate::mark_audio_thread`).
                    crate::mark_audio_thread();
                    st_err.mark_lost(matches!(e, cpal::StreamError::DeviceNotAvailable));
                    log::warn!("input stream error: {e}");
                },
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Recorder {
            monitor,
            ring,
            stats,
            active: Mutex::new(None),
            _stream: Some(stream),
            device_name: name,
            sample_rate: rate,
            channels,
            config: negotiated,
            first_capture,
        })
    }

    #[cfg(target_os = "freebsd")]
    pub fn open_with(_req: Option<&DeviceRequest>) -> Result<Recorder, String> {
        Err("recording is not supported on this platform yet".into())
    }

    fn active(&self) -> std::sync::MutexGuard<'_, Option<ActiveCapture>> {
        self.active.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Start a new take written to a fresh file under `dir` (never overwriting).
    pub fn start_take(&self, dir: &Path, base: &str) -> Result<PathBuf, String> {
        let mut a = self.active();
        if a.is_some() {
            return Err("a take is already being recorded".into());
        }
        if let Ok(mut f) = self.first_capture.lock() {
            *f = None;
        }
        let cap =
            ActiveCapture::start_on_ring(Arc::clone(&self.ring), dir, base, self.sample_rate, WriterConfig::default()).map_err(|e| e.to_string())?;
        let path = cap.media().to_path_buf();
        *a = Some(cap);
        Ok(path)
    }

    /// Compatibility: start a take in the system temp folder.
    pub fn arm(&self) {
        let dir = std::env::temp_dir().join("sml-recording");
        if let Err(e) = self.start_take(&dir, "Take") {
            log::warn!("cannot start capture: {e}");
        }
    }

    pub fn is_armed(&self) -> bool {
        self.ring.is_armed()
    }

    /// Stop capturing, finalize the file and return the take (read back from disk).
    pub fn take(&self) -> Take {
        self.take_selected(&|_| true)
    }

    /// Finish the take and read back only the capture channels `keep` selects (the others are
    /// empty at their index): the file on disk always holds every input.
    pub fn take_selected(&self, keep: &dyn Fn(usize) -> bool) -> Take {
        let Some(cap) = self.active().take() else {
            return Take { sample_rate: self.sample_rate, channels: vec![Vec::new(); self.channels], media: None, report: None };
        };
        let mut report = cap.finish();
        // CL-T2 disconnect: the take ended because the input stream failed — explicit partial.
        if let Some(why) = self.stats.lost_reason()
            && report.state == CaptureState::Finalized
        {
            report.state = CaptureState::DeviceLost;
            report.error = Some(why.to_string());
        }
        let channels = crate::capture::read_capture_selected(&report.media, keep).map(|(_, c)| c).unwrap_or_else(|e| {
            log::warn!("reading back {}: {e}", report.media.display());
            vec![Vec::new(); self.channels]
        });
        Take { sample_rate: self.sample_rate, channels, media: Some(report.media.clone()), report: Some(report) }
    }

    /// Frames captured so far in the current take.
    pub fn captured_frames(&self) -> usize {
        if self.ring.is_armed() { usize::try_from(self.ring.accepted_frames()).unwrap_or(usize::MAX) } else { 0 }
    }

    /// Why the input stream stopped (device unplugged / driver error), if it did. Recording
    /// cannot continue on this stream; the UI finalizes the take and reopens the input later.
    pub fn input_lost(&self) -> Option<&'static str> {
        self.stats.lost_reason()
    }

    /// Observed callback sizes (effective buffer), if any callback ran.
    pub fn observed_block_range(&self) -> Option<(u32, u32)> {
        let (a, b) = (self.stats.min_block.load(Ordering::Acquire), self.stats.max_block.load(Ordering::Acquire));
        (a != u32::MAX && b > 0).then_some((a, b))
    }
}

pub fn deinterleave(data: &[f32], channels: usize, sample_rate: u32) -> Take {
    let ch = channels.max(1);
    let frames = data.len() / ch;
    let mut out = vec![Vec::with_capacity(frames); ch];
    for frame in data.chunks_exact(ch) {
        for (c, v) in frame.iter().enumerate() {
            if let Some(o) = out.get_mut(c) {
                o.push(*v);
            }
        }
    }
    Take { sample_rate, channels: out, media: None, report: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CL-T2 / A1 recovery, simulated disconnect: the stream error marks the recorder; the take
    /// that was running ends as an explicit DeviceLost partial with every frame captured before
    /// the failure intact (no silent truncation, no substitute device).
    #[test]
    fn disconnect_mid_take_is_an_explicit_partial_with_frames_kept() {
        let ring = CaptureRing::new(2, 48_000);
        let rec = Recorder {
            monitor: InputRing::new(2, 48_000),
            ring: Arc::clone(&ring),
            stats: Arc::new(StreamStats::default()),
            active: Mutex::new(None),
            #[cfg(not(target_os = "freebsd"))]
            _stream: None,
            device_name: "fixture".into(),
            sample_rate: 48_000,
            channels: 2,
            config: None,
            first_capture: Arc::new(Mutex::new(None)),
        };
        let dir = std::env::temp_dir().join(format!("sml-disconnect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        rec.start_take(&dir, "Vox").unwrap();
        let block: Vec<f32> = (0..256).map(|i| i as f32 / 256.0).collect();
        let mut pushed = 0u64;
        for k in 0..20u64 {
            assert!(ring.push(&block, 0, k * 128));
            pushed += 128;
        }
        assert!(rec.input_lost().is_none());
        rec.stats.mark_lost(true); // the backend's error callback
        rec.stats.mark_lost(false); // later errors do not overwrite the first reason
        assert_eq!(rec.input_lost(), Some("the input device is no longer available"));
        let take = rec.take();
        let report = take.report.unwrap();
        assert_eq!(report.state, CaptureState::DeviceLost);
        assert!(report.state.is_partial());
        assert_eq!(report.frames, pushed, "every frame before the failure is on disk");
        assert_eq!(take.channels[0].len() as u64, pushed);
        assert_eq!(take.channels[1][1], 3.0 / 256.0, "interleaving intact");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn deinterleaves() {
        let t = super::deinterleave(&[1.0, 2.0, 3.0, 4.0, 5.0], 2, 48_000);
        assert_eq!(t.channels, vec![vec![1.0, 3.0], vec![2.0, 4.0]]);
        let t = super::deinterleave(&[], 0, 48_000);
        assert_eq!(t.channels.len(), 1);
    }
}

/// Ring state behind the lock: interleaved input frames at the input device's rate, plus the
/// fractional read position (in input frames) of the next output sample.
struct RingState {
    data: std::collections::VecDeque<f32>,
    phase: f64,
}

/// A small interleaved FIFO from the input callback to the output callback, for input
/// monitoring. Both sides only `try_lock`; it holds at most ~0.5 s and drops the oldest frames.
/// The input device may run at a different rate than the session, so the reader converts
/// rates with linear interpolation (cheap, allocation-free, fine for monitoring).
pub struct InputRing {
    state: Mutex<RingState>,
    pub channels: std::sync::atomic::AtomicUsize,
    /// Input device sample rate in Hz (0 when unknown, which reads as "same as the session").
    rate: u32,
    /// Most samples kept: half a second of input, or 24 000 frames when the rate is unknown.
    cap: usize,
}

impl InputRing {
    pub fn new(channels: usize, rate: u32) -> Arc<InputRing> {
        let ch = channels.max(1);
        let frames = if rate == 0 { 24_000 } else { rate as usize / 2 };
        Arc::new(InputRing {
            state: Mutex::new(RingState { data: std::collections::VecDeque::with_capacity(rate.max(48_000) as usize * ch), phase: 0.0 }),
            channels: std::sync::atomic::AtomicUsize::new(ch),
            rate,
            cap: frames * ch,
        })
    }

    pub fn push(&self, samples: &[f32]) {
        if let Ok(mut s) = self.state.try_lock() {
            s.data.extend(samples.iter().copied());
            while s.data.len() > self.cap {
                s.data.pop_front();
            }
        }
    }

    /// Fill `out` (planar, at `session_rate`) with up to `frames` frames, converting from the
    /// input rate. Missing frames are silence.
    pub fn pop_into(&self, out: &mut [Vec<f32>], frames: usize, session_rate: u32) {
        let ch = self.channels.load(Ordering::Relaxed).max(1);
        for c in out.iter_mut() {
            c.iter_mut().take(frames).for_each(|x| *x = 0.0);
        }
        let Ok(mut s) = self.state.try_lock() else { return };
        // Input frames consumed per output frame.
        let step = if self.rate == 0 || session_rate == 0 { 1.0 } else { f64::from(self.rate) / f64::from(session_rate) };
        for f in 0..frames {
            // Drop input frames we have moved past; keep the one at `phase` and the next one.
            while s.phase >= 1.0 && s.data.len() >= 2 * ch {
                s.data.drain(..ch);
                s.phase -= 1.0;
            }
            if s.phase >= 1.0 || s.data.len() < 2 * ch {
                break;
            }
            let fr = s.phase as f32;
            for c in 0..ch {
                let a = s.data.get(c).copied().unwrap_or(0.0);
                let b = s.data.get(ch + c).copied().unwrap_or(a);
                if let Some(x) = out.get_mut(c).and_then(|o| o.get_mut(f)) {
                    *x = a + (b - a) * fr;
                }
            }
            s.phase += step;
        }
    }
}

#[cfg(test)]
mod ring_tests {
    use super::InputRing;

    /// A 1 kHz sine recorded at 16 kHz must come out as a 1 kHz sine at 48 kHz, across calls.
    #[test]
    fn monitor_converts_input_rate_to_session_rate() {
        let ring = InputRing::new(1, 16_000);
        let input: Vec<f32> = (0..4_000).map(|i| (2.0 * std::f64::consts::PI * 1_000.0 * f64::from(i) / 16_000.0).sin() as f32).collect();
        ring.push(&input);
        let mut out = vec![vec![0.0f32; 48]];
        let mut k = 0usize;
        for _ in 0..10 {
            ring.pop_into(&mut out, 48, 48_000);
            for v in &out[0] {
                let want = (2.0 * std::f64::consts::PI * 1_000.0 * k as f64 / 48_000.0).sin() as f32;
                assert!((v - want).abs() < 0.05, "sample {k}: got {v}, want {want}");
                k += 1;
            }
        }
    }

    /// Input faster than the session (96 kHz into 48 kHz): every other frame is read, in order.
    #[test]
    fn monitor_decimates_when_input_is_faster() {
        let ring = InputRing::new(1, 96_000);
        let input: Vec<f32> = (0..1_000).map(|i| i as f32).collect();
        ring.push(&input);
        let mut out = vec![vec![0.0f32; 100]];
        ring.pop_into(&mut out, 100, 48_000);
        for (k, v) in out[0].iter().enumerate() {
            assert_eq!(*v, (2 * k) as f32, "output {k}");
        }
    }

    /// Same rate: samples pass through unchanged.
    #[test]
    fn monitor_passes_through_at_equal_rates() {
        let ring = InputRing::new(2, 48_000);
        let input: Vec<f32> = (0..200).map(|i| i as f32).collect();
        ring.push(&input);
        let mut out = vec![vec![0.0f32; 50], vec![0.0f32; 50]];
        ring.pop_into(&mut out, 50, 48_000);
        assert_eq!(out[0][0], 0.0);
        assert_eq!(out[1][0], 1.0);
        assert_eq!(out[0][49], 98.0);
    }

    /// Nothing buffered: silence, not stale data.
    #[test]
    fn monitor_underrun_is_silence() {
        let ring = InputRing::new(1, 16_000);
        let mut out = vec![vec![1.0f32; 16]];
        ring.pop_into(&mut out, 16, 48_000);
        assert!(out[0].iter().all(|v| *v == 0.0));
    }
}
