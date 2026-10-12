//! Bend render service (EDIT_TOOLBAR_SPEC T08, V11): turns a clip's warp map into a derived
//! audio source with the in-house WSOLA time stretch (E05 baseline), off the UI and audio
//! threads. Admission is bounded: one active render per clip, one replaceable pending revision
//! per clip, and a session-wide queue ceiling; a result whose revision no longer matches the clip
//! is stale and dropped. The caller keeps the last valid render until a current result is ready.
//!
//! Limits of this baseline (stated, not hidden): each segment between anchors is stretched on
//! its own and the segments are butt-joined (no crossfade at anchors), so an anchor placed in
//! the middle of a sustained note can click; stereo channels share one timing map (the stretch
//! uses the channel average as its guide); ratios are bounded to 0.25..4 per segment.

use soundcraft_audio_io::AudioBuffer;
use soundcraft_model::{ClipId, SourceAudio, WarpAnchor, warped_len};
use soundcraft_time::Samples;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

/// Session-wide ceiling on queued (not yet started) renders.
pub const QUEUE_CEILING: usize = 4;

/// What a render needs: the immutable original material and the map.
#[derive(Clone)]
pub struct Job {
    pub clip: ClipId,
    pub rev: u64,
    pub source: Arc<SourceAudio>,
    /// Offset into the source and length of the original material (samples).
    pub offset: Samples,
    pub src_len: Samples,
    pub anchors: Vec<WarpAnchor>,
    pub sample_rate: f32,
}

/// A finished render.
pub struct Done {
    pub clip: ClipId,
    pub rev: u64,
    pub result: Result<AudioBuffer, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Status {
    /// Waiting in the queue (replaceable by a newer revision).
    Queued { rev: u64 },
    /// Being rendered on the worker.
    Rendering { rev: u64 },
    /// The last render failed; the clip keeps its previous content.
    Error { rev: u64, message: String },
}

/// Off-thread renderer with bounded admission.
#[derive(Default)]
pub struct Renderer {
    worker: Option<(Sender<Job>, Receiver<Done>)>,
    /// One active render per clip (the worker takes jobs one at a time).
    active: Option<(ClipId, u64)>,
    /// One pending revision per clip; newer replaces older.
    pending: VecDeque<Job>,
    errors: HashMap<ClipId, (u64, String)>,
    /// Count of results dropped as stale (evidence for T-T3).
    pub dropped_stale: u64,
}

impl std::fmt::Debug for Renderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Renderer").field("active", &self.active).field("pending", &self.pending.len()).finish()
    }
}

impl Clone for Renderer {
    /// A cloned engine starts with an idle renderer (jobs belong to the thread that queued them).
    fn clone(&self) -> Self {
        Renderer::default()
    }
}

impl Renderer {
    /// Queue a render. A job for a clip already pending replaces it; the queue ceiling rejects.
    pub fn submit(&mut self, job: Job) -> Result<(), String> {
        self.errors.remove(&job.clip);
        if let Some(slot) = self.pending.iter_mut().find(|j| j.clip == job.clip) {
            *slot = job;
            return Ok(());
        }
        if self.pending.len() >= QUEUE_CEILING {
            return Err(format!("render queue full ({QUEUE_CEILING} clips waiting); try again when a render finishes"));
        }
        self.pending.push_back(job);
        self.pump();
        Ok(())
    }

    /// Start the next pending job when the worker is idle.
    fn pump(&mut self) {
        if self.active.is_some() {
            return;
        }
        let Some(job) = self.pending.pop_front() else { return };
        self.active = Some((job.clip, job.rev));
        let sent = self.worker.get_or_insert_with(spawn_worker).0.send(job).is_ok();
        if !sent {
            // The worker died: start a fresh one next time.
            self.worker = None;
            self.active = None;
        }
    }

    /// Finished results (newest last). Call from the thread that owns the document.
    pub fn poll(&mut self) -> Vec<Done> {
        let mut out = Vec::new();
        if let Some((_, rx)) = &self.worker {
            while let Ok(d) = rx.try_recv() {
                if self.active.is_some_and(|(c, r)| c == d.clip && r == d.rev) {
                    self.active = None;
                }
                if let Err(m) = &d.result {
                    self.errors.insert(d.clip, (d.rev, m.clone()));
                }
                out.push(d);
            }
        }
        self.pump();
        out
    }

    pub fn status(&self, clip: ClipId) -> Option<Status> {
        if let Some((c, rev)) = self.active
            && c == clip
        {
            return Some(Status::Rendering { rev });
        }
        if let Some(j) = self.pending.iter().find(|j| j.clip == clip) {
            return Some(Status::Queued { rev: j.rev });
        }
        self.errors.get(&clip).map(|(rev, message)| Status::Error { rev: *rev, message: message.clone() })
    }

    pub fn is_busy(&self) -> bool {
        self.active.is_some() || !self.pending.is_empty()
    }

    /// Forget a clip's pending work and error (the clip was removed or unwarped).
    pub fn forget(&mut self, clip: ClipId) {
        self.pending.retain(|j| j.clip != clip);
        self.errors.remove(&clip);
    }
}

fn spawn_worker() -> (Sender<Job>, Receiver<Done>) {
    let (jtx, jrx) = channel::<Job>();
    let (dtx, drx) = channel::<Done>();
    let _ = std::thread::Builder::new().name("sml-bend-render".into()).spawn(move || {
        while let Ok(job) = jrx.recv() {
            let result = render(&job);
            if dtx.send(Done { clip: job.clip, rev: job.rev, result }).is_err() {
                break;
            }
        }
    });
    (jtx, drx)
}

/// Render the warp map: every segment between consecutive anchors (with the implicit (0,0)
/// start and the clip end carried at ratio 1 after the last anchor) is stretched to its
/// destination length and the pieces are concatenated. Deterministic; pure function of the job.
pub fn render(job: &Job) -> Result<AudioBuffer, String> {
    let src_len = usize::try_from(job.src_len).map_err(|_| "bad source length".to_string())?;
    if src_len == 0 {
        return Err("empty material".into());
    }
    soundcraft_model::validate_warp(&job.anchors, job.src_len)?;
    let a0 = usize::try_from(job.offset.max(0)).unwrap_or(0);
    let nch = job.source.buffer.channels.len().max(1);
    let material: Vec<Vec<f32>> = (0..nch)
        .map(|c| {
            let ch = job.source.buffer.channels.get(c).map(Vec::as_slice).unwrap_or(&[]);
            let mut v: Vec<f32> = ch.iter().skip(a0).take(src_len).copied().collect();
            v.resize(src_len, 0.0);
            v
        })
        .collect();
    let out_len = usize::try_from(warped_len(&job.anchors, job.src_len)).unwrap_or(1);
    let mut out: Vec<Vec<f32>> = vec![Vec::with_capacity(out_len); nch];
    let mut prev = WarpAnchor { src: 0, dst: 0 };
    let mut segments: Vec<(WarpAnchor, WarpAnchor)> = Vec::new();
    for a in &job.anchors {
        segments.push((prev, *a));
        prev = *a;
    }
    // The tail after the last anchor keeps its speed.
    let tail_src = job.src_len - prev.src;
    if tail_src > 0 {
        segments.push((prev, WarpAnchor { src: job.src_len, dst: prev.dst + tail_src }));
    }
    for (a, b) in segments {
        let (s0, s1) = (usize::try_from(a.src).unwrap_or(0), usize::try_from(b.src).unwrap_or(0).min(src_len));
        let want = usize::try_from(b.dst - a.dst).unwrap_or(0);
        if s1 <= s0 || want == 0 {
            continue;
        }
        let seg: Vec<Vec<f32>> = material.iter().map(|c| c[s0..s1].to_vec()).collect();
        let ratio = want as f64 / (s1 - s0) as f64;
        let mut stretched = if (ratio - 1.0).abs() < 1e-9 { seg } else { soundcraft_dsp::offline::time_stretch(&seg, ratio, job.sample_rate) };
        for (o, s) in out.iter_mut().zip(stretched.iter_mut()) {
            s.resize(want, 0.0);
            o.extend_from_slice(s);
        }
    }
    for o in &mut out {
        o.resize(out_len, 0.0);
        if o.iter().any(|x| !x.is_finite()) {
            return Err("the render produced non-finite samples".into());
        }
    }
    Ok(AudioBuffer { sample_rate: job.source.buffer.sample_rate, channels: out })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(len: usize, sr: f32, hz: f32) -> Arc<SourceAudio> {
        let ch: Vec<f32> = (0..len).map(|i| (2.0 * std::f32::consts::PI * hz * i as f32 / sr).sin() * 0.5).collect();
        Arc::new(SourceAudio::new(AudioBuffer { sample_rate: sr as u32, channels: vec![ch.clone(), ch] }))
    }

    fn zero_crossing_hz(x: &[f32], sr: f32) -> f32 {
        let n = x.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        n as f32 * sr / x.len() as f32
    }

    /// T-T3 / V11: the requested edit map yields the exact rendered length, the pitch is kept
    /// within the algorithm's declared tolerance, the source is untouched and the output finite.
    #[test]
    fn render_honours_the_map_and_keeps_pitch() {
        let sr = 48_000.0;
        let src = tone(48_000, sr, 220.0);
        let before = src.buffer.channels.clone();
        let anchors = vec![WarpAnchor { src: 12_000, dst: 18_000 }, WarpAnchor { src: 36_000, dst: 36_000 }];
        let job = Job { clip: ClipId(1), rev: 1, source: Arc::clone(&src), offset: 0, src_len: 48_000, anchors: anchors.clone(), sample_rate: sr };
        let out = render(&job).unwrap();
        assert_eq!(out.channels.len(), 2);
        assert_eq!(out.channels[0].len() as i64, warped_len(&anchors, 48_000));
        assert_eq!(out.channels[0].len(), 48_000, "last anchor lands on its source time: same total length");
        // First segment slowed 1.5×, second sped 0.75×: pitch within 2 % in both.
        let f1 = zero_crossing_hz(&out.channels[0][0..18_000], sr);
        let f2 = zero_crossing_hz(&out.channels[0][18_000..36_000], sr);
        assert!((f1 - 220.0).abs() < 4.4, "{f1}");
        assert!((f2 - 220.0).abs() < 4.4, "{f2}");
        assert!(out.channels.iter().flatten().all(|x| x.is_finite()));
        assert_eq!(src.buffer.channels, before, "the original is immutable");
        // Invalid maps are rejected before any work.
        let bad = Job { anchors: vec![WarpAnchor { src: 100, dst: 100 }, WarpAnchor { src: 90, dst: 300 }], ..job.clone() };
        assert!(render(&bad).is_err());
        let steep = Job { anchors: vec![WarpAnchor { src: 1_000, dst: 10_000 }], ..job };
        assert!(render(&steep).is_err(), "ratio 10 is outside the bound");
    }

    /// Bounded admission: one pending revision per clip (newer replaces), a queue ceiling,
    /// stale results are reported with their revision so the caller can drop them.
    #[test]
    fn admission_is_bounded_and_results_carry_revisions() {
        let sr = 48_000.0;
        let src = tone(4_800, sr, 220.0);
        let job = |clip: u64, rev: u64| Job {
            clip: ClipId(clip),
            rev,
            source: Arc::clone(&src),
            offset: 0,
            src_len: 4_800,
            anchors: vec![WarpAnchor { src: 2_400, dst: 2_800 }],
            sample_rate: sr,
        };
        let mut r = Renderer::default();
        r.submit(job(1, 1)).unwrap();
        // Clip 1 is active now; the next five queue, the sixth exceeds the ceiling.
        for c in 2..=5 {
            r.submit(job(c, 1)).unwrap();
        }
        assert!(r.submit(job(6, 1)).is_err());
        // A newer revision for a queued clip replaces it instead of growing the queue.
        r.submit(job(3, 2)).unwrap();
        assert_eq!(r.status(ClipId(3)), Some(Status::Queued { rev: 2 }));
        assert!(matches!(r.status(ClipId(1)), Some(Status::Rendering { rev: 1 })));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut done = Vec::new();
        while done.len() < 5 && std::time::Instant::now() < deadline {
            done.extend(r.poll());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(done.len(), 5, "every admitted job finishes");
        assert!(done.iter().any(|d| d.clip == ClipId(3) && d.rev == 2));
        assert!(done.iter().all(|d| d.result.is_ok()));
        assert!(!r.is_busy());
        // An error is remembered per clip until the next submission.
        let mut bad = job(9, 1);
        bad.anchors = vec![WarpAnchor { src: 10, dst: 10 }];
        r.submit(bad).unwrap();
        let mut got = Vec::new();
        while got.is_empty() && std::time::Instant::now() < deadline {
            got.extend(r.poll());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(matches!(r.status(ClipId(9)), Some(Status::Error { rev: 1, .. })));
        r.forget(ClipId(9));
        assert_eq!(r.status(ClipId(9)), None);
    }
}
