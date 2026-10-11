//! Audio-owned timebase (CL01): stream generations, integer device frames, transport mapping,
//! in-block event offsets and the recording-placement latency ledger.
//!
//! Musical/session time is derived only from frames the device actually serviced, never from GUI
//! repaint, wall time or disk-writer progress. Every running stream gets a fresh generation id, so
//! a timestamp from a stopped/reconfigured stream can never be mistaken for a current one.
//! Unknown measurements stay `None`/`Unknown`, never zero.

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// A new, never-reused stream generation id.
pub fn new_generation() -> u64 {
    NEXT_GENERATION.fetch_add(1, Ordering::Relaxed)
}

/// Where a timing value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Provenance {
    /// Reported by the audio backend/driver.
    Backend,
    /// Measured by a calibration run.
    Measured,
    /// Entered by the user.
    Manual,
    /// Derived arithmetically (e.g. buffer size / rate); not a measurement.
    Estimated,
    /// Not available. Never treated as zero.
    Unknown,
}

/// A timing quantity in samples with its provenance and uncertainty (± samples, if known).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Timed {
    pub samples: Option<i64>,
    pub provenance: Provenance,
    pub uncertainty: Option<f64>,
}

impl Timed {
    pub const UNKNOWN: Timed = Timed { samples: None, provenance: Provenance::Unknown, uncertainty: None };
    pub fn known(samples: i64, provenance: Provenance) -> Timed {
        Timed { samples: Some(samples), provenance, uncertainty: None }
    }
}

/// One device callback, stamped on the audio thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockStamp {
    pub generation: u64,
    /// Device stream frame of the first frame of this block (monotonic within a generation).
    pub stream_frame: u64,
    pub frames: u32,
}

/// Monotonic per-stream frame counter. Lives on the audio thread; never allocates.
#[derive(Debug, Clone)]
pub struct StreamClock {
    generation: u64,
    frames: u64,
    pub sample_rate: u32,
    min_block: u32,
    max_block: u32,
    blocks: u64,
}

impl StreamClock {
    pub fn new(sample_rate: u32) -> StreamClock {
        StreamClock { generation: new_generation(), frames: 0, sample_rate, min_block: u32::MAX, max_block: 0, blocks: 0 }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Frames serviced so far in this generation.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Stamps a block of `frames` and advances.
    pub fn advance(&mut self, frames: usize) -> BlockStamp {
        let n = u32::try_from(frames).unwrap_or(u32::MAX);
        let st = BlockStamp { generation: self.generation, stream_frame: self.frames, frames: n };
        self.frames = self.frames.saturating_add(u64::from(n));
        self.blocks += 1;
        self.min_block = self.min_block.min(n);
        self.max_block = self.max_block.max(n);
        st
    }

    /// Observed callback sizes (min, max) — the *effective* callback buffer, which may differ
    /// from the requested one.
    pub fn observed_block_range(&self) -> Option<(u32, u32)> {
        (self.blocks > 0).then_some((self.min_block, self.max_block))
    }

    /// Stop/restart/device change: a new generation, frame counter from zero.
    pub fn restart(&mut self, sample_rate: u32) {
        *self = StreamClock::new(sample_rate);
    }
}

/// Errors mapping stream time to session time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClockError {
    /// The timestamp belongs to another (stopped/replaced) stream.
    StaleGeneration { expected: u64, got: u64 },
    /// Before the anchor (the transport was not running then).
    BeforeAnchor,
}

/// Maps device stream frames to signed session samples. Set at transport start / seek / loop
/// wrap; the only owner of the stream→session relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportMap {
    pub generation: u64,
    pub stream_anchor: u64,
    pub session_anchor: i64,
}

impl TransportMap {
    pub fn session_at(&self, generation: u64, stream_frame: u64) -> Result<i64, ClockError> {
        if generation != self.generation {
            return Err(ClockError::StaleGeneration { expected: self.generation, got: generation });
        }
        let d = stream_frame.checked_sub(self.stream_anchor).ok_or(ClockError::BeforeAnchor)?;
        Ok(self.session_anchor.saturating_add(i64::try_from(d).unwrap_or(i64::MAX)))
    }
}

/// Exact offset of a session-time event inside a block starting at `block_start` with `frames`
/// frames; `None` when the event is outside the block. Used for click, punch and loop
/// boundaries (events execute at this sample, not at block boundaries).
pub fn offset_in_block(block_start: i64, frames: usize, event_at: i64) -> Option<usize> {
    let d = event_at.checked_sub(block_start)?;
    let d = usize::try_from(d).ok()?;
    (d < frames).then_some(d)
}

/// Splits a block at every event offset inside it: returns `(start, len)` sub-blocks covering
/// `frames` exactly. Events at offset 0 or duplicates create no empty sub-block.
pub fn split_block(block_start: i64, frames: usize, events: &[i64], out: &mut Vec<(usize, usize)>) {
    out.clear();
    let mut cuts: [usize; 16] = [0; 16];
    let mut n = 0;
    for &e in events {
        if let Some(o) = offset_in_block(block_start, frames, e)
            && o > 0
            && n < cuts.len()
            && !cuts[..n].contains(&o)
        {
            cuts[n] = o;
            n += 1;
        }
    }
    cuts[..n].sort_unstable();
    let mut at = 0;
    for &c in &cuts[..n] {
        out.push((at, c - at));
        at = c;
    }
    out.push((at, frames - at));
}

/// A punch/record window in session samples: which frames of a block are recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PunchWindow {
    pub start: i64,
    pub end: i64,
}

impl PunchWindow {
    /// `(offset, len)` of the recorded part of a block, if any.
    pub fn clip_block(&self, block_start: i64, frames: usize) -> Option<(usize, usize)> {
        let bend = block_start.saturating_add(frames as i64);
        let s = self.start.max(block_start);
        let e = self.end.min(bend);
        (e > s).then(|| ((s - block_start) as usize, (e - s) as usize))
    }
}

/// Count-in/pre-roll: session position where transport starts for a punch at `punch_start`.
pub fn preroll_start(punch_start: i64, preroll_samples: i64) -> i64 {
    punch_start.saturating_sub(preroll_samples.max(0))
}

/// Recording-placement ledger (CL04). Every term is signed samples; positive = the captured audio
/// arrives that much *late* relative to the moment it was played/performed. Each term is applied
/// exactly once: driver-reported terms are only used when the driver does not already compensate.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LatencyLedger {
    /// Input latency reported by the backend.
    pub driver_input: Timed,
    /// Output latency reported by the backend (matters for overdubs against playback).
    pub driver_output: Timed,
    /// True when the backend timestamps already account for its own latency.
    pub driver_compensated: bool,
    /// Residual measured by loopback calibration *after* the driver terms (overdub path).
    pub measured_residual: Timed,
    /// User offset.
    pub manual: i64,
}

impl Default for LatencyLedger {
    fn default() -> Self {
        LatencyLedger {
            driver_input: Timed::UNKNOWN,
            driver_output: Timed::UNKNOWN,
            driver_compensated: false,
            measured_residual: Timed::UNKNOWN,
            manual: 0,
        }
    }
}

/// Placement result: the correction applied and whether any term was unknown.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Placement {
    /// Session start of the take's first sample.
    pub session_start: i64,
    /// Total correction subtracted from the raw capture-time position.
    pub correction: i64,
    /// True when a required driver term was Unknown (placement precision cannot be claimed).
    pub incomplete: bool,
}

impl LatencyLedger {
    /// Correction (samples to move earlier) for an overdub played against playback output.
    pub fn overdub_correction(&self) -> (i64, bool) {
        let mut c = self.manual;
        let mut incomplete = false;
        if !self.driver_compensated {
            for t in [self.driver_input, self.driver_output] {
                match t.samples {
                    Some(v) => c = c.saturating_add(v),
                    None => incomplete = true,
                }
            }
        }
        if let Some(r) = self.measured_residual.samples {
            c = c.saturating_add(r);
        }
        (c, incomplete)
    }

    /// Places a take whose first frame was captured at stream frame `first` (mapped through the
    /// transport). Raw file samples are never touched — only this placement metadata changes.
    pub fn place(&self, map: &TransportMap, generation: u64, first: u64) -> Result<Placement, ClockError> {
        let raw = map.session_at(generation, first)?;
        let (correction, incomplete) = self.overdub_correction();
        Ok(Placement { session_start: raw.saturating_sub(correction), correction, incomplete })
    }
}

/// Bounded statistics for callback timing (CL05/CL06). Load uses frames serviced / rate as the
/// denominator, never the observed wall interval.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct CallbackStats {
    pub blocks: u64,
    pub xruns: u64,
    pub max_load: f64,
    pub sum_load: f64,
    pub deadline_misses: u64,
}

impl CallbackStats {
    /// Records one callback: `compute_ns` spent rendering `frames` at `rate`.
    pub fn record(&mut self, compute_ns: u64, frames: usize, rate: u32) {
        let budget_ns = frames as f64 / f64::from(rate.max(1)) * 1e9;
        let load = if budget_ns > 0.0 { compute_ns as f64 / budget_ns } else { 0.0 };
        self.blocks += 1;
        self.sum_load += load;
        self.max_load = self.max_load.max(load);
        if load >= 1.0 {
            self.deadline_misses += 1;
        }
    }
    pub fn mean_load(&self) -> f64 {
        if self.blocks == 0 { 0.0 } else { self.sum_load / self.blocks as f64 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CL-T1: variable callback sizes over 30 simulated minutes at 48 kHz: exact frame count,
    /// monotonic cursor, generation reset on restart, no wall-clock dependence, bounded state.
    #[test]
    fn variable_callbacks_count_exact_frames_for_thirty_minutes() {
        let sizes = [32usize, 64, 127, 128, 256, 512, 1024];
        let total: u64 = 48_000 * 60 * 30;
        let mut c = StreamClock::new(48_000);
        let g = c.generation();
        let (mut k, mut expect, mut last) = (0usize, 0u64, None::<u64>);
        while expect < total {
            let n = sizes[k % sizes.len()].min((total - expect) as usize);
            let st = c.advance(n);
            assert_eq!(st.stream_frame, expect);
            assert_eq!(st.generation, g);
            if let Some(l) = last {
                assert!(st.stream_frame > l, "monotonic");
            }
            last = Some(st.stream_frame);
            expect += n as u64;
            k += 1;
        }
        assert_eq!(c.frames(), total);
        assert_eq!(c.observed_block_range(), Some((32, 1024)));
        c.restart(44_100);
        assert_ne!(c.generation(), g, "restart gets a new generation");
        assert_eq!(c.frames(), 0);
        assert_eq!(c.observed_block_range(), None);
    }

    #[test]
    fn transport_map_rejects_stale_generations_and_maps_seeks_and_loops() {
        let mut c = StreamClock::new(48_000);
        c.advance(1000);
        let m = TransportMap { generation: c.generation(), stream_anchor: 1000, session_anchor: 96_000 };
        assert_eq!(m.session_at(c.generation(), 1000), Ok(96_000));
        assert_eq!(m.session_at(c.generation(), 1500), Ok(96_500));
        assert_eq!(m.session_at(c.generation(), 999), Err(ClockError::BeforeAnchor));
        let old = c.generation();
        c.restart(48_000);
        let m2 = TransportMap { generation: c.generation(), stream_anchor: 0, session_anchor: 0 };
        assert!(matches!(m2.session_at(old, 1500), Err(ClockError::StaleGeneration { .. })));
        // Loop wrap = re-anchor: session restarts at loop start at the exact wrap frame.
        let wrap = TransportMap { generation: c.generation(), stream_anchor: 48_000, session_anchor: -480 };
        assert_eq!(wrap.session_at(c.generation(), 48_480), Ok(0), "negative session positions are signed");
    }

    /// CL-T3: events at block edges and interiors land on exact samples.
    #[test]
    fn events_land_on_exact_block_offsets() {
        assert_eq!(offset_in_block(1000, 128, 1000), Some(0));
        assert_eq!(offset_in_block(1000, 128, 1127), Some(127));
        assert_eq!(offset_in_block(1000, 128, 1128), None);
        assert_eq!(offset_in_block(1000, 128, 999), None);
        let mut v = Vec::new();
        split_block(1000, 128, &[1000, 1064, 1010, 1064, 5000], &mut v);
        assert_eq!(v, vec![(0, 10), (10, 54), (64, 64)]);
        assert_eq!(v.iter().map(|x| x.1).sum::<usize>(), 128);
        split_block(0, 64, &[], &mut v);
        assert_eq!(v, vec![(0, 64)]);
    }

    #[test]
    fn punch_window_clips_blocks_exactly() {
        let p = PunchWindow { start: 1010, end: 1300 };
        assert_eq!(p.clip_block(1000, 128), Some((10, 118)));
        assert_eq!(p.clip_block(1128, 128), Some((0, 128)));
        assert_eq!(p.clip_block(1256, 128), Some((0, 44)));
        assert_eq!(p.clip_block(1300, 128), None);
        assert_eq!(p.clip_block(800, 128), None);
        // Total recorded = window length, whatever the callback sizes.
        let mut total = 0;
        let mut pos = 900i64;
        for n in [127usize, 64, 33, 256, 1, 512] {
            total += p.clip_block(pos, n).map_or(0, |x| x.1);
            pos += n as i64;
        }
        assert_eq!(total, 290);
        assert_eq!(preroll_start(48_000, 96_000), -48_000);
    }

    /// CL-T3 ledger/sign fixtures: known late placement +D needs a -D correction; driver terms
    /// already compensated by the backend are not counted twice; unknown terms are flagged.
    #[test]
    fn ledger_signs_and_single_application() {
        let map = TransportMap { generation: 9, stream_anchor: 0, session_anchor: 0 };
        // Audio performed at session 48_000 arrives in the capture 300 samples late.
        let mut l = LatencyLedger {
            driver_input: Timed::known(200, Provenance::Backend),
            driver_output: Timed::known(64, Provenance::Backend),
            measured_residual: Timed::known(36, Provenance::Measured),
            ..LatencyLedger::default()
        };
        let p = l.place(&map, 9, 48_300).unwrap();
        assert_eq!(p.correction, 300);
        assert_eq!(p.session_start, 48_000);
        assert!(!p.incomplete);
        // Backend already compensates: only the residual and manual offsets apply.
        l.driver_compensated = true;
        l.manual = -5;
        assert_eq!(l.place(&map, 9, 48_300).unwrap().correction, 31);
        // Unknown driver latency is not zero: placement is flagged incomplete.
        let u = LatencyLedger::default();
        let p = u.place(&map, 9, 48_300).unwrap();
        assert!(p.incomplete);
        assert_eq!(p.session_start, 48_300, "nothing invented");
        assert!(u.place(&map, 8, 1).is_err(), "stale generation rejected");
    }

    #[test]
    fn callback_load_uses_serviced_frames_not_wall_gap() {
        let mut s = CallbackStats::default();
        // 64 frames @ 48 kHz = 1.333 ms budget; 1 ms compute = 75 % load.
        s.record(1_000_000, 64, 48_000);
        assert!((s.max_load - 0.75).abs() < 1e-9);
        s.record(2_000_000, 64, 48_000);
        assert_eq!(s.deadline_misses, 1);
    }
}
