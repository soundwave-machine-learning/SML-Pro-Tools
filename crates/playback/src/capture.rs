//! Bounded dry capture: input callback → preallocated lock-free ring → disk-writer thread → WAV.
//!
//! The input callback only copies samples into a fixed ring of atomics ([`CaptureRing::push`]):
//! no allocation, lock, file work or growth on the audio thread. A writer thread drains the ring
//! into a [`CaptureSink`] (a float WAV file in production) and, after each durable flush, updates
//! a small JSON journal next to the media with the number of frames known to be on disk. The
//! journal is always written *after* the media flush, so crash recovery ([`recover`]) never
//! claims more durable frames than the file actually holds.
//!
//! Failure policy (no silent gaps): if the ring cannot take a whole callback block (the writer
//! fell behind), the capture is marked `Overflow` and stops accepting further frames, so the file
//! never contains a hidden discontinuity. A sink error marks it `WriteFailed`. Both keep every
//! frame already committed and are reported to the caller as a stopped partial capture.
//!
//! Samples are captured exactly as delivered (dry, before any playback or mixer processing) and
//! written as 32-bit float PCM, so the file is bit-identical to the callback input.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};

/// Capture state, shared between the callback, the writer and the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CaptureState {
    Recording,
    /// Stopped normally; every accepted frame is on disk.
    Finalized,
    /// The ring overflowed (the writer could not keep up). Frames before the overflow are kept.
    Overflow,
    /// The sink returned an error (disk full, device removed…). Committed frames are kept.
    WriteFailed,
    /// Rebuilt from a journal after an interrupted session.
    Recovered,
    /// The input stream failed (device unplugged / driver error) during the take. Frames
    /// captured before the failure are kept.
    DeviceLost,
}

impl CaptureState {
    fn code(self) -> u8 {
        match self {
            CaptureState::Recording => 0,
            CaptureState::Finalized => 1,
            CaptureState::Overflow => 2,
            CaptureState::WriteFailed => 3,
            CaptureState::Recovered => 4,
            CaptureState::DeviceLost => 5,
        }
    }
    fn from_code(c: u8) -> CaptureState {
        match c {
            1 => CaptureState::Finalized,
            2 => CaptureState::Overflow,
            3 => CaptureState::WriteFailed,
            4 => CaptureState::Recovered,
            5 => CaptureState::DeviceLost,
            _ => CaptureState::Recording,
        }
    }
    /// True for the stopped-with-loss states the UI must show as a partial capture.
    pub fn is_partial(self) -> bool {
        matches!(self, CaptureState::Overflow | CaptureState::WriteFailed | CaptureState::DeviceLost)
    }
}

/// Single-producer / single-consumer ring of interleaved f32 samples stored as bits in atomics
/// (safe Rust, no locks). Capacity is fixed at construction.
pub struct CaptureRing {
    slots: Vec<AtomicU32>,
    channels: usize,
    /// Total samples ever written / read (monotonic; index = count % capacity).
    write: AtomicU64,
    read: AtomicU64,
    /// Producer accepted frames (only whole callback blocks are accepted).
    accepted_frames: AtomicU64,
    /// Frames offered after the capture stopped accepting (overflow / failure / disarmed).
    rejected_frames: AtomicU64,
    state: AtomicU8,
    armed: AtomicBool,
    /// Device stream frame at which the first accepted sample was captured (u64::MAX = none yet).
    first_stream_frame: AtomicU64,
    /// Stream generation of that first frame.
    generation: AtomicU64,
    /// Pushes in flight (so a stop can wait for the callback to leave `push`).
    pushing: AtomicU32,
}

impl CaptureRing {
    /// A ring holding `capacity_frames` frames of `channels` channels. Allocates (call off the
    /// audio thread).
    pub fn new(channels: usize, capacity_frames: usize) -> Arc<CaptureRing> {
        let ch = channels.clamp(1, 64);
        let frames = capacity_frames.clamp(64, 1 << 24);
        let mut slots = Vec::with_capacity(frames * ch);
        slots.resize_with(frames * ch, || AtomicU32::new(0));
        Arc::new(CaptureRing {
            slots,
            channels: ch,
            write: AtomicU64::new(0),
            read: AtomicU64::new(0),
            accepted_frames: AtomicU64::new(0),
            rejected_frames: AtomicU64::new(0),
            state: AtomicU8::new(CaptureState::Recording.code()),
            armed: AtomicBool::new(false),
            first_stream_frame: AtomicU64::new(u64::MAX),
            generation: AtomicU64::new(0),
            pushing: AtomicU32::new(0),
        })
    }

    /// Clears counters and state for a new take. Only while disarmed and idle (returns false
    /// otherwise, changing nothing).
    pub fn reset(&self) -> bool {
        if self.is_armed() || self.pushing.load(Ordering::Acquire) != 0 {
            return false;
        }
        self.write.store(0, Ordering::Release);
        self.read.store(0, Ordering::Release);
        self.accepted_frames.store(0, Ordering::Release);
        self.rejected_frames.store(0, Ordering::Release);
        self.first_stream_frame.store(u64::MAX, Ordering::Release);
        self.generation.store(0, Ordering::Release);
        self.state.store(CaptureState::Recording.code(), Ordering::Release);
        true
    }

    /// Waits (off the audio thread) until no callback is inside `push`.
    fn quiesce(&self) {
        for _ in 0..2000 {
            if self.pushing.load(Ordering::Acquire) == 0 {
                return;
            }
            std::thread::sleep(std::time::Duration::from_micros(250));
        }
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn capacity_samples(&self) -> usize {
        self.slots.len()
    }

    pub fn arm(&self, on: bool) {
        self.armed.store(on, Ordering::Release);
    }

    pub fn is_armed(&self) -> bool {
        self.armed.load(Ordering::Acquire)
    }

    pub fn state(&self) -> CaptureState {
        CaptureState::from_code(self.state.load(Ordering::Acquire))
    }

    fn fail(&self, s: CaptureState) {
        // First failure wins.
        let _ = self.state.compare_exchange(CaptureState::Recording.code(), s.code(), Ordering::AcqRel, Ordering::Acquire);
    }

    pub fn accepted_frames(&self) -> u64 {
        self.accepted_frames.load(Ordering::Acquire)
    }

    pub fn rejected_frames(&self) -> u64 {
        self.rejected_frames.load(Ordering::Acquire)
    }

    /// Stream position of the first captured frame, with its stream generation.
    pub fn first_frame(&self) -> Option<(u64, u64)> {
        let f = self.first_stream_frame.load(Ordering::Acquire);
        (f != u64::MAX).then(|| (self.generation.load(Ordering::Acquire), f))
    }

    /// Realtime side: offer one interleaved callback block captured at device stream frame
    /// `stream_frame` of stream `generation`. Accepts the whole block or none of it. Never
    /// allocates, locks or blocks. Returns whether the block was accepted.
    pub fn push(&self, interleaved: &[f32], generation: u64, stream_frame: u64) -> bool {
        self.pushing.fetch_add(1, Ordering::AcqRel);
        let ok = self.push_inner(interleaved, generation, stream_frame);
        self.pushing.fetch_sub(1, Ordering::AcqRel);
        ok
    }

    fn push_inner(&self, interleaved: &[f32], generation: u64, stream_frame: u64) -> bool {
        let ch = self.channels;
        let frames = interleaved.len() / ch;
        if frames == 0 || !self.armed.load(Ordering::Acquire) {
            return false;
        }
        if self.state() != CaptureState::Recording {
            self.rejected_frames.fetch_add(frames as u64, Ordering::AcqRel);
            return false;
        }
        let n = frames * ch;
        let w = self.write.load(Ordering::Relaxed);
        let r = self.read.load(Ordering::Acquire);
        let cap = self.slots.len() as u64;
        if cap.saturating_sub(w.saturating_sub(r)) < n as u64 {
            self.fail(CaptureState::Overflow);
            self.rejected_frames.fetch_add(frames as u64, Ordering::AcqRel);
            return false;
        }
        for (k, v) in interleaved.iter().take(n).enumerate() {
            let idx = ((w + k as u64) % cap) as usize;
            if let Some(s) = self.slots.get(idx) {
                s.store(v.to_bits(), Ordering::Relaxed);
            }
        }
        if self.first_stream_frame.load(Ordering::Relaxed) == u64::MAX {
            self.generation.store(generation, Ordering::Release);
            self.first_stream_frame.store(stream_frame, Ordering::Release);
        }
        self.write.store(w + n as u64, Ordering::Release);
        self.accepted_frames.fetch_add(frames as u64, Ordering::AcqRel);
        true
    }

    /// Writer side: moves up to `out.len()` (rounded down to whole frames) samples out of the
    /// ring. Returns the number of samples copied.
    pub fn pop(&self, out: &mut [f32]) -> usize {
        let ch = self.channels;
        let r = self.read.load(Ordering::Relaxed);
        let w = self.write.load(Ordering::Acquire);
        let avail = w.saturating_sub(r) as usize;
        let n = avail.min(out.len()) / ch * ch;
        let cap = self.slots.len() as u64;
        for (k, o) in out.iter_mut().take(n).enumerate() {
            let idx = ((r + k as u64) % cap) as usize;
            *o = self.slots.get(idx).map_or(0.0, |s| f32::from_bits(s.load(Ordering::Relaxed)));
        }
        self.read.store(r + n as u64, Ordering::Release);
        n
    }

    /// Samples waiting for the writer.
    pub fn queued_samples(&self) -> usize {
        self.write.load(Ordering::Acquire).saturating_sub(self.read.load(Ordering::Acquire)) as usize
    }
}

/// Where the writer puts captured samples.
pub trait CaptureSink: Send {
    /// Append interleaved samples (whole frames).
    fn write(&mut self, interleaved: &[f32]) -> std::io::Result<()>;
    /// Make everything written so far durable (flush + fsync for files).
    fn flush_durable(&mut self) -> std::io::Result<()>;
    /// Patch headers so the file is a complete, valid file of `frames` frames.
    fn finalize(&mut self, frames: u64) -> std::io::Result<()>;
}

/// Journal written next to the media after every durable flush.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CaptureJournal {
    pub media: PathBuf,
    pub channels: usize,
    pub sample_rate: u32,
    /// Frames known to be flushed to the media file.
    pub durable_frames: u64,
    pub state: CaptureState,
    /// Stream generation and device frame of the first captured frame (placement provenance).
    pub generation: Option<u64>,
    pub first_stream_frame: Option<u64>,
    pub error: Option<String>,
}

pub fn journal_path(media: &Path) -> PathBuf {
    let mut s = media.as_os_str().to_owned();
    s.push(".journal.json");
    PathBuf::from(s)
}

/// Writes `j` atomically (temp file + rename) and syncs it.
pub fn write_journal(j: &CaptureJournal) -> std::io::Result<()> {
    let path = journal_path(&j.media);
    let tmp = path.with_extension("tmp");
    let data = serde_json::to_vec_pretty(j).map_err(std::io::Error::other)?;
    {
        let mut f = File::create(&tmp)?;
        f.write_all(&data)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, &path)
}

pub fn read_journal(media: &Path) -> std::io::Result<CaptureJournal> {
    let data = std::fs::read(journal_path(media))?;
    serde_json::from_slice(&data).map_err(std::io::Error::other)
}

const WAV_HEADER: u64 = 44;

/// A 32-bit float WAV file written incrementally. Created with `create_new`, so an existing file
/// is never overwritten.
pub struct WavCaptureFile {
    file: File,
    channels: u16,
    rate: u32,
    frames: u64,
}

fn wav_header(channels: u16, rate: u32, frames: u64) -> [u8; 44] {
    let block = u32::from(channels) * 4;
    let data = u32::try_from(frames.saturating_mul(u64::from(block))).unwrap_or(u32::MAX - 36);
    let mut h = [0u8; 44];
    let mut put = |at: usize, b: &[u8]| {
        if let Some(d) = h.get_mut(at..at + b.len()) {
            d.copy_from_slice(b);
        }
    };
    put(0, b"RIFF");
    put(4, &(36u32.saturating_add(data)).to_le_bytes());
    put(8, b"WAVEfmt ");
    put(16, &16u32.to_le_bytes());
    put(20, &3u16.to_le_bytes()); // IEEE float
    put(22, &channels.to_le_bytes());
    put(24, &rate.to_le_bytes());
    put(28, &(rate.saturating_mul(block)).to_le_bytes());
    put(32, &(u16::try_from(block).unwrap_or(u16::MAX)).to_le_bytes());
    put(34, &32u16.to_le_bytes());
    put(36, b"data");
    put(40, &data.to_le_bytes());
    h
}

impl WavCaptureFile {
    pub fn create(path: &Path, channels: usize, rate: u32) -> std::io::Result<WavCaptureFile> {
        let ch = u16::try_from(channels.clamp(1, 64)).unwrap_or(2);
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(&wav_header(ch, rate, 0))?;
        Ok(WavCaptureFile { file, channels: ch, rate, frames: 0 })
    }
}

impl CaptureSink for WavCaptureFile {
    fn write(&mut self, interleaved: &[f32]) -> std::io::Result<()> {
        let mut bytes = Vec::with_capacity(interleaved.len() * 4);
        for v in interleaved {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        self.file.write_all(&bytes)?;
        self.frames += (interleaved.len() / usize::from(self.channels).max(1)) as u64;
        Ok(())
    }
    fn flush_durable(&mut self) -> std::io::Result<()> {
        self.file.flush()?;
        self.file.sync_data()
    }
    fn finalize(&mut self, frames: u64) -> std::io::Result<()> {
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&wav_header(self.channels, self.rate, frames))?;
        self.file.seek(SeekFrom::End(0))?;
        self.file.sync_all()
    }
}

/// Result of a finished capture.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CaptureReport {
    pub media: PathBuf,
    pub state: CaptureState,
    pub channels: usize,
    pub sample_rate: u32,
    /// Frames written and finalized in the media file.
    pub frames: u64,
    /// Frames the callback offered that were not captured (after overflow/failure).
    pub rejected_frames: u64,
    pub generation: Option<u64>,
    pub first_stream_frame: Option<u64>,
    pub error: Option<String>,
}

/// Writer configuration.
#[derive(Debug, Clone, Copy)]
pub struct WriterConfig {
    /// Durable flush + journal update every this many frames (and at the end).
    pub flush_every_frames: u64,
    /// Writer poll interval when the ring is empty.
    pub poll: std::time::Duration,
    /// Chunk size (frames) moved per write.
    pub chunk_frames: usize,
}

impl Default for WriterConfig {
    fn default() -> Self {
        WriterConfig { flush_every_frames: 48_000, poll: std::time::Duration::from_millis(5), chunk_frames: 8192 }
    }
}

/// The writer loop: drains `ring` into `sink` until `stop` is set and the ring is empty (or a
/// failure occurs), journaling durable progress. Runs on its own (non-realtime) thread.
pub fn run_writer(
    ring: &CaptureRing,
    sink: &mut dyn CaptureSink,
    journal: &mut CaptureJournal,
    stop: &AtomicBool,
    cfg: WriterConfig,
) -> CaptureReport {
    let ch = ring.channels();
    let mut buf = vec![0.0f32; cfg.chunk_frames.max(1) * ch];
    let mut written: u64 = 0;
    let mut since_flush: u64 = 0;
    let mut error: Option<String> = None;
    let checkpoint = |written: u64, sink: &mut dyn CaptureSink, journal: &mut CaptureJournal| -> std::io::Result<()> {
        // Media first, journal second: the journal never runs ahead of the data.
        sink.flush_durable()?;
        journal.durable_frames = written;
        if let Some((g, f)) = ring.first_frame() {
            journal.generation = Some(g);
            journal.first_stream_frame = Some(f);
        }
        write_journal(journal)
    };
    loop {
        let n = ring.pop(&mut buf);
        if n > 0 {
            if let Err(e) = sink.write(buf.get(..n).unwrap_or(&[])) {
                ring.fail(CaptureState::WriteFailed);
                error = Some(e.to_string());
                break;
            }
            let frames = (n / ch) as u64;
            written += frames;
            since_flush += frames;
            if since_flush >= cfg.flush_every_frames {
                if let Err(e) = checkpoint(written, sink, journal) {
                    ring.fail(CaptureState::WriteFailed);
                    error = Some(e.to_string());
                    break;
                }
                since_flush = 0;
            }
            continue;
        }
        let failed = ring.state() != CaptureState::Recording;
        if stop.load(Ordering::Acquire) || failed {
            // Drain anything pushed between the last pop and the stop flag.
            if ring.queued_samples() > 0 {
                continue;
            }
            break;
        }
        std::thread::sleep(cfg.poll);
    }
    ring.arm(false);
    // Commit what we have, even after an overflow (those frames are good and contiguous).
    let fin = sink.finalize(written).and_then(|_| sink.flush_durable());
    let mut state = ring.state();
    if let Err(e) = fin {
        state = CaptureState::WriteFailed;
        error.get_or_insert(e.to_string());
    }
    if state == CaptureState::Recording {
        state = CaptureState::Finalized;
    }
    journal.state = state;
    journal.error = error.clone();
    let _ = checkpoint(written, sink, journal);
    let first = ring.first_frame();
    CaptureReport {
        media: journal.media.clone(),
        state,
        channels: ch,
        sample_rate: journal.sample_rate,
        frames: written,
        rejected_frames: ring.rejected_frames(),
        generation: first.map(|f| f.0),
        first_stream_frame: first.map(|f| f.1),
        error,
    }
}

/// A capture in progress: the ring the callback pushes into plus the writer thread.
pub struct ActiveCapture {
    pub ring: Arc<CaptureRing>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<CaptureReport>>,
    media: PathBuf,
}

impl ActiveCapture {
    /// Creates a new take file (never overwriting) under `dir` and starts its writer. `seconds`
    /// of ring headroom are preallocated.
    pub fn start(dir: &Path, base: &str, channels: usize, sample_rate: u32, ring_seconds: f64, cfg: WriterConfig) -> std::io::Result<ActiveCapture> {
        std::fs::create_dir_all(dir)?;
        let (media, file) = create_unique(dir, base, channels, sample_rate)?;
        let frames = ((f64::from(sample_rate) * ring_seconds.clamp(0.05, 60.0)) as usize).max(1024);
        let ring = CaptureRing::new(channels, frames);
        Self::start_with_sink(ring, Box::new(file), media, sample_rate, cfg)
    }

    /// Starts a take on an existing (reused, idle) ring — the realtime host keeps one ring for
    /// the life of its input stream so the callback never needs a new one.
    pub fn start_on_ring(ring: Arc<CaptureRing>, dir: &Path, base: &str, sample_rate: u32, cfg: WriterConfig) -> std::io::Result<ActiveCapture> {
        if !ring.reset() {
            return Err(std::io::Error::other("capture ring is busy"));
        }
        std::fs::create_dir_all(dir)?;
        let (media, file) = create_unique(dir, base, ring.channels(), sample_rate)?;
        Self::start_with_sink(ring, Box::new(file), media, sample_rate, cfg)
    }

    /// Starts a writer for an arbitrary sink (used by failure-injection tests).
    pub fn start_with_sink(
        ring: Arc<CaptureRing>,
        mut sink: Box<dyn CaptureSink>,
        media: PathBuf,
        sample_rate: u32,
        cfg: WriterConfig,
    ) -> std::io::Result<ActiveCapture> {
        let stop = Arc::new(AtomicBool::new(false));
        let mut journal = CaptureJournal {
            media: media.clone(),
            channels: ring.channels(),
            sample_rate,
            durable_frames: 0,
            state: CaptureState::Recording,
            generation: None,
            first_stream_frame: None,
            error: None,
        };
        write_journal(&journal)?;
        let r2 = Arc::clone(&ring);
        let s2 = Arc::clone(&stop);
        let handle =
            std::thread::Builder::new().name("sml-capture-writer".into()).spawn(move || run_writer(&r2, sink.as_mut(), &mut journal, &s2, cfg))?;
        ring.arm(true);
        Ok(ActiveCapture { ring, stop, handle: Some(handle), media })
    }

    pub fn media(&self) -> &Path {
        &self.media
    }

    /// Stops accepting input, drains the ring, finalizes the file and returns the report.
    pub fn finish(mut self) -> CaptureReport {
        self.ring.arm(false);
        self.ring.quiesce();
        self.stop.store(true, Ordering::Release);
        let fallback = CaptureReport {
            media: self.media.clone(),
            state: CaptureState::WriteFailed,
            channels: self.ring.channels(),
            sample_rate: 0,
            frames: 0,
            rejected_frames: self.ring.rejected_frames(),
            generation: None,
            first_stream_frame: None,
            error: Some("capture writer thread failed".into()),
        };
        self.handle.take().and_then(|h| h.join().ok()).unwrap_or(fallback)
    }
}

impl Drop for ActiveCapture {
    fn drop(&mut self) {
        self.ring.arm(false);
        self.stop.store(true, Ordering::Release);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// `<dir>/<base>_<n>.wav` for the first `n` that does not exist; created atomically so two takes
/// can never share a file.
pub fn create_unique(dir: &Path, base: &str, channels: usize, rate: u32) -> std::io::Result<(PathBuf, WavCaptureFile)> {
    let clean: String = base.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    for n in 1..100_000u32 {
        let p = dir.join(format!("{clean}_{n:03}.wav"));
        match WavCaptureFile::create(&p, channels, rate) {
            Ok(f) => return Ok((p, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("no free take file name"))
}

/// What [`recover`] found.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RecoveryReport {
    pub media: PathBuf,
    /// Frames the journal guarantees.
    pub durable_frames: u64,
    /// Whole frames physically present in the file's data region.
    pub file_frames: u64,
    /// Frames kept in the recovered file (= durable frames; never more).
    pub kept_frames: u64,
    /// Frames present but not journaled (written after the last checkpoint); discarded because
    /// their durability is unknown.
    pub discarded_unjournaled_frames: u64,
}

/// Rebuilds a capture interrupted before finalization: truncates the media to the journal's
/// durable frame count (never more than the file holds), patches the header and marks the
/// journal `Recovered`. The source samples that are kept are not modified.
pub fn recover(media: &Path) -> std::io::Result<RecoveryReport> {
    let mut j = read_journal(media)?;
    let mut f = OpenOptions::new().read(true).write(true).open(media)?;
    let len = f.metadata()?.len();
    let mut head = [0u8; 44];
    f.read_exact(&mut head)?;
    if head.get(0..4) != Some(b"RIFF") || head.get(8..12) != Some(b"WAVE") {
        return Err(std::io::Error::other("not a capture WAV file"));
    }
    let block = (j.channels.max(1) * 4) as u64;
    let file_frames = len.saturating_sub(WAV_HEADER) / block;
    let kept = j.durable_frames.min(file_frames);
    f.set_len(WAV_HEADER + kept * block)?;
    f.seek(SeekFrom::Start(0))?;
    f.write_all(&wav_header(u16::try_from(j.channels).unwrap_or(2), j.sample_rate, kept))?;
    f.sync_all()?;
    j.durable_frames = kept;
    if !matches!(j.state, CaptureState::Finalized) {
        j.state = CaptureState::Recovered;
    }
    write_journal(&j)?;
    Ok(RecoveryReport {
        media: media.to_path_buf(),
        durable_frames: j.durable_frames,
        file_frames,
        kept_frames: kept,
        discarded_unjournaled_frames: file_frames.saturating_sub(kept),
    })
}

/// Reads a finalized (or recovered) float capture file back as planar channels.
pub fn read_capture(media: &Path) -> std::io::Result<(u32, Vec<Vec<f32>>)> {
    read_capture_selected(media, &|_| true)
}

/// Like [`read_capture`], keeping only the channels `keep` selects (the others come back empty,
/// at their index). Streams the file, so memory is the kept channels only — a take from a
/// many-input interface does not load every input when one track was armed.
pub fn read_capture_selected(media: &Path, keep: &dyn Fn(usize) -> bool) -> std::io::Result<(u32, Vec<Vec<f32>>)> {
    use std::io::Read;
    let mut f = std::io::BufReader::with_capacity(1 << 16, std::fs::File::open(media)?);
    let mut head = [0u8; 44];
    f.read_exact(&mut head).map_err(|_| std::io::Error::other("short WAV header"))?;
    let get16 = |at: usize| head.get(at..at + 2).and_then(|b| b.try_into().ok()).map(u16::from_le_bytes);
    let get32 = |at: usize| head.get(at..at + 4).and_then(|b| b.try_into().ok()).map(u32::from_le_bytes);
    let (Some(ch), Some(rate), Some(fmt)) = (get16(22), get32(24), get16(20)) else {
        return Err(std::io::Error::other("short WAV header"));
    };
    if fmt != 3 {
        return Err(std::io::Error::other("not a float capture file"));
    }
    let ch = usize::from(ch).max(1);
    let on_disk = std::fs::metadata(media)?.len().saturating_sub(44);
    let declared = get32(40).map_or(on_disk, |d| u64::from(d).min(on_disk));
    let frames = usize::try_from(declared / (4 * ch as u64)).unwrap_or(0);
    let kept: Vec<bool> = (0..ch).map(keep).collect();
    let mut out: Vec<Vec<f32>> = kept.iter().map(|k| if *k { Vec::with_capacity(frames) } else { Vec::new() }).collect();
    let mut frame = vec![0u8; 4 * ch];
    for _ in 0..frames {
        f.read_exact(&mut frame)?;
        for ((o, k), b) in out.iter_mut().zip(&kept).zip(frame.chunks_exact(4)) {
            if *k {
                o.push(f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            }
        }
    }
    Ok((rate, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sml-capture-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Deterministic ramp/impulse fixture: channel c of frame i carries a unique value.
    fn fixture(frames: usize, ch: usize) -> Vec<f32> {
        (0..frames * ch)
            .map(|k| {
                let (i, c) = (k / ch, k % ch);
                if i % 997 == 0 { 1.0 - c as f32 * 0.25 } else { (i as f32 * 1.0e-5 + c as f32 * 0.5) % 0.9 }
            })
            .collect()
    }

    fn fast() -> WriterConfig {
        WriterConfig { flush_every_frames: 4096, poll: std::time::Duration::from_millis(1), chunk_frames: 1024 }
    }

    /// Irregular callback sizes at 44.1/48 kHz, mono and stereo: the file is bit-identical to
    /// the delivered input, in order, with the exact frame count and channel map.
    #[test]
    fn capture_is_bit_exact_for_irregular_callbacks() {
        for (rate, ch) in [(44_100u32, 1usize), (48_000, 2), (48_000, 1), (44_100, 2)] {
            let dir = tmpdir(&format!("exact{rate}{ch}"));
            let cap = ActiveCapture::start(&dir, "Vox", ch, rate, 1.0, fast()).unwrap();
            let input = fixture(rate as usize * 2 + 17, ch);
            let sizes = [32usize, 64, 127, 128, 256, 511, 1024, 7];
            let (mut at, mut k, mut frame) = (0usize, 0usize, 0u64);
            while at < input.len() {
                let n = (sizes[k % sizes.len()] * ch).min(input.len() - at);
                // Pace like a device so the writer keeps up.
                while cap.ring.capacity_samples() - cap.ring.queued_samples() < n {
                    std::thread::sleep(std::time::Duration::from_micros(200));
                }
                assert!(cap.ring.push(&input[at..at + n], 7, frame));
                frame += (n / ch) as u64;
                at += n;
                k += 1;
            }
            let rep = cap.finish();
            assert_eq!(rep.state, CaptureState::Finalized);
            assert_eq!(rep.frames as usize, input.len() / ch);
            assert_eq!(rep.first_stream_frame, Some(0));
            assert_eq!(rep.generation, Some(7));
            let (r, planar) = read_capture(&rep.media).unwrap();
            assert_eq!(r, rate);
            assert_eq!(planar.len(), ch);
            for (i, fr) in input.chunks_exact(ch).enumerate() {
                for c in 0..ch {
                    assert_eq!(planar[c][i].to_bits(), fr[c].to_bits(), "frame {i} ch {c}");
                }
            }
            let j = read_journal(&rep.media).unwrap();
            assert_eq!(j.durable_frames, rep.frames);
            assert_eq!(j.state, CaptureState::Finalized);
        }
    }

    #[test]
    fn takes_never_overwrite_existing_files() {
        let dir = tmpdir("unique");
        let a = ActiveCapture::start(&dir, "Take", 1, 48_000, 0.1, fast()).unwrap();
        let b = ActiveCapture::start(&dir, "Take", 1, 48_000, 0.1, fast()).unwrap();
        assert_ne!(a.media(), b.media());
        a.ring.push(&[0.5; 64], 1, 0);
        let ra = a.finish();
        let rb = b.finish();
        assert_eq!(ra.frames, 64);
        assert_eq!(rb.frames, 0);
        assert_eq!(read_capture(&ra.media).unwrap().1[0], vec![0.5; 64], "second take left the first intact");
    }

    #[test]
    fn disarmed_ring_records_nothing() {
        let ring = CaptureRing::new(2, 1024);
        assert!(!ring.push(&[1.0; 64], 0, 0));
        assert_eq!(ring.queued_samples(), 0);
    }

    /// Queue saturation: the writer is stalled, the ring fills, the capture stops explicitly as
    /// Overflow; every frame before the overflow is kept, nothing after it is (no silent gap).
    #[test]
    fn overflow_stops_capture_explicitly_without_gaps() {
        let ring = CaptureRing::new(1, 1024);
        ring.arm(true);
        let block: Vec<f32> = (0..256).map(|i| i as f32).collect();
        let mut accepted = 0;
        for k in 0..10 {
            if ring.push(&block, 1, k * 256) {
                accepted += 1;
            }
        }
        assert_eq!(accepted, 4, "exactly the ring's capacity was accepted");
        assert_eq!(ring.state(), CaptureState::Overflow);
        assert_eq!(ring.rejected_frames(), 6 * 256);
        // Space frees up, but the capture stays stopped: later blocks would leave a hole.
        let mut out = vec![0.0; 2048];
        assert_eq!(ring.pop(&mut out), 1024);
        assert!(!ring.push(&block, 1, 99_999));
        assert_eq!(ring.state(), CaptureState::Overflow);
    }

    struct FailingSink {
        written: Vec<f32>,
        fail_after: usize,
    }
    impl CaptureSink for FailingSink {
        fn write(&mut self, x: &[f32]) -> std::io::Result<()> {
            if self.written.len() + x.len() > self.fail_after {
                return Err(std::io::Error::other("simulated disk full"));
            }
            self.written.extend_from_slice(x);
            Ok(())
        }
        fn flush_durable(&mut self) -> std::io::Result<()> {
            Ok(())
        }
        fn finalize(&mut self, _frames: u64) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Simulated disk full: explicit WriteFailed, committed frames reported, later input rejected.
    #[test]
    fn writer_error_is_explicit_and_keeps_committed_frames() {
        let dir = tmpdir("diskfull");
        let ring = CaptureRing::new(1, 1 << 16);
        let media = dir.join("full.wav");
        let cap =
            ActiveCapture::start_with_sink(ring, Box::new(FailingSink { written: Vec::new(), fail_after: 3000 }), media, 48_000, fast()).unwrap();
        for k in 0..20u64 {
            cap.ring.push(&[0.25; 512], 1, k * 512);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let rep = cap.finish();
        assert_eq!(rep.state, CaptureState::WriteFailed);
        assert!(rep.error.as_deref().unwrap_or("").contains("disk full"));
        assert!(rep.frames <= 3000);
        let j = read_journal(&rep.media).unwrap();
        assert_eq!(j.state, CaptureState::WriteFailed);
        assert!(j.durable_frames <= rep.frames);
    }

    /// Process interruption: a file whose writer died after a checkpoint is recovered to exactly
    /// the journaled durable frames; unjournaled tail frames are reported as discarded.
    #[test]
    fn crash_recovery_keeps_exactly_the_durable_frames() {
        let dir = tmpdir("recover");
        let media = dir.join("crash.wav");
        let mut f = WavCaptureFile::create(&media, 2, 48_000).unwrap();
        let input = fixture(10_000, 2);
        f.write(&input[..6000 * 2]).unwrap();
        f.flush_durable().unwrap();
        write_journal(&CaptureJournal {
            media: media.clone(),
            channels: 2,
            sample_rate: 48_000,
            durable_frames: 6000,
            state: CaptureState::Recording,
            generation: Some(3),
            first_stream_frame: Some(480),
            error: None,
        })
        .unwrap();
        // More data reached the file after the checkpoint, then the process died (no finalize).
        f.write(&input[6000 * 2..]).unwrap();
        drop(f);
        let rep = recover(&media).unwrap();
        assert_eq!(rep.kept_frames, 6000);
        assert_eq!(rep.file_frames, 10_000);
        assert_eq!(rep.discarded_unjournaled_frames, 4000);
        let (_, planar) = read_capture(&media).unwrap();
        assert_eq!(planar[0].len(), 6000);
        for i in 0..6000 {
            assert_eq!(planar[1][i].to_bits(), input[i * 2 + 1].to_bits());
        }
        assert_eq!(read_journal(&media).unwrap().state, CaptureState::Recovered);
        // A journal claiming more than the file holds can never inflate the recovered take.
        let media2 = dir.join("short.wav");
        let mut g = WavCaptureFile::create(&media2, 1, 48_000).unwrap();
        g.write(&[0.1; 100]).unwrap();
        drop(g);
        write_journal(&CaptureJournal {
            media: media2.clone(),
            channels: 1,
            sample_rate: 48_000,
            durable_frames: 5000,
            state: CaptureState::Recording,
            generation: None,
            first_stream_frame: None,
            error: None,
        })
        .unwrap();
        assert_eq!(recover(&media2).unwrap().kept_frames, 100);
    }

    /// The ring never grows: capacity is fixed whatever is pushed.
    #[test]
    fn ring_memory_is_bounded() {
        let ring = CaptureRing::new(2, 4096);
        let cap = ring.capacity_samples();
        ring.arm(true);
        for k in 0..1000 {
            ring.push(&[0.0; 256], 0, k);
            let mut out = [0.0; 256];
            ring.pop(&mut out);
        }
        assert_eq!(ring.capacity_samples(), cap);
        assert_eq!(ring.accepted_frames(), 128_000);
    }

    /// A selected read keeps exactly the chosen channels (bit-exact), others empty, and agrees
    /// with the full read.
    #[test]
    fn selected_read_keeps_only_the_chosen_channels() {
        let d = tmpdir("selected");
        let (frames, ch) = (5000, 6);
        let data = fixture(frames, ch);
        let mut w = WavCaptureFile::create(&d.join("t.wav"), ch, 48_000).unwrap();
        w.write(&data).unwrap();
        w.finalize(frames as u64).unwrap();
        let (_, full) = read_capture(&d.join("t.wav")).unwrap();
        let (rate, sel) = read_capture_selected(&d.join("t.wav"), &|c| c == 2 || c == 5).unwrap();
        assert_eq!(rate, 48_000);
        assert_eq!(sel.len(), ch);
        for c in 0..ch {
            if c == 2 || c == 5 {
                assert_eq!(sel[c], full[c]);
                assert_eq!(sel[c].len(), frames);
            } else {
                assert!(sel[c].is_empty());
            }
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
