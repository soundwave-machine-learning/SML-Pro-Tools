//! Project autosave with rolling recovery generations (R04 / E02).
//!
//! A generation is written to a unique temporary sibling, flushed (`sync_all`), re-read and
//! validated (parse + checksum), renamed into place, and only then published by rewriting the
//! index (temporary file, flush, rename). The previous valid generation is untouched until the
//! new one is committed, and never deleted before the index names a newer one. A failure at any
//! step leaves the previous generation and index intact. Retention keeps the newest `retain`
//! generations and a byte budget, always keeping the last known-good one.
//!
//! Platform limits (documented, not promised away): `rename` is atomic on POSIX and NTFS for a
//! same-volume move; durability of the *directory entry* after a power loss depends on the file
//! system (the directory is `sync_all`ed on Unix). An abrupt process kill is the tested failure
//! model (see the fault-injection tests); OS/power failure between `sync_all` and the rename can
//! lose the newest generation but not an older committed one.
//!
//! Media are not duplicated here: unsaved recorded sources are written once per source id under
//! `media/` (same policy as `io::save_copy`), and finished/partial takes keep their own capture
//! journal (`soundcraft_playback::capture`). Third-party plugin state may be stale — the index
//! records when it was last captured.

use crate::{Engine, EngineError, Result};
use soundcraft_model::Session;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Default autosave interval (seconds) and retention.
pub const DEFAULT_INTERVAL_S: f64 = 60.0;
pub const DEFAULT_RETAIN: usize = 20;
/// Default byte budget for one session's generations (media excluded): 256 MiB.
pub const DEFAULT_BUDGET_BYTES: u64 = 256 << 20;

pub const INDEX_FILE: &str = "index.json";

/// Where the writer can be made to fail, for E02 fault-injection tests: each point models the
/// process being killed right there (files left exactly as they were).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// `write` to the temporary file fails with "no space left" (disk full).
    DiskFull,
    /// Killed after the temporary file was fully written and flushed, before the rename.
    BeforeRename,
    /// Killed after the generation was renamed into place, before the index was rewritten.
    BeforeIndex,
    /// Killed after the temporary index was written, before it replaced the index.
    BeforeIndexRename,
}

/// One committed generation, as the index records it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Generation {
    pub number: u64,
    /// File name inside the store directory.
    pub file: String,
    pub bytes: u64,
    /// FNV-1a 64 of the file contents (hex).
    pub checksum: String,
    /// Seconds since the Unix epoch when it was committed.
    pub unix_time: u64,
    pub session_name: String,
    /// Engine revision the snapshot was taken at.
    pub revision: u64,
    /// Seconds between the last third-party plugin-state capture and this snapshot, if known;
    /// `None` = never captured in this run (hosted plugin state may be stale).
    pub plugin_state_age_s: Option<u64>,
}

/// The store index: the newest committed generation first.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Index {
    pub format: u32,
    pub generations: Vec<Generation>,
}

/// A session's autosave store (one directory per session).
#[derive(Debug, Clone)]
pub struct Store {
    pub dir: PathBuf,
    pub retain: usize,
    pub budget_bytes: u64,
}

/// What a snapshot writer needs from the document, taken on the UI thread.
#[derive(Debug, Clone)]
pub struct SnapshotRequest {
    pub session: std::sync::Arc<Session>,
    pub revision: u64,
    pub plugin_state_age_s: Option<u64>,
}

/// Result of a committed snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct Committed {
    pub generation: Generation,
    pub media_written: usize,
    /// Generations removed by retention / budget after the commit.
    pub pruned: usize,
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> Store {
        Store { dir: dir.into(), retain: DEFAULT_RETAIN, budget_bytes: DEFAULT_BUDGET_BYTES }
    }

    /// The store for `session_name` under `root` (the app's autosave folder).
    pub fn for_session(root: &Path, session_name: &str) -> Store {
        Store::new(root.join(crate::io::sanitize_name(session_name)))
    }

    pub fn index_path(&self) -> PathBuf {
        self.dir.join(INDEX_FILE)
    }

    /// Reads the index; a missing or unreadable index is an empty one (recovery can still scan
    /// generation files, see [`Store::recover`]).
    pub fn index(&self) -> Index {
        std::fs::read_to_string(self.index_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    /// Writes one generation (see the module docs). `fault` injects a failure for tests.
    pub fn snapshot(&self, req: &SnapshotRequest) -> Result<Committed> {
        self.snapshot_with(req, None)
    }

    pub fn snapshot_with(&self, req: &SnapshotRequest, fault: Option<Fault>) -> Result<Committed> {
        std::fs::create_dir_all(&self.dir).map_err(|e| io_err(&self.dir, e))?;
        let mut index = self.index();
        // Past every generation the index knows AND every file on disk (an interrupted index
        // write leaves a newer file than the index names; it must not be overwritten).
        let number = index.generations.first().map_or(0, |g| g.number).max(self.highest_on_disk()) + 1;
        // Media first (unsaved recorded sources, one file per source id; existing files kept).
        let (session, media_written) = self.write_media(&req.session)?;
        let text = session.to_json().map_err(|e| EngineError::Io(e.to_string()))?;
        let file = format!("gen-{number:06}.scraft");
        let final_path = self.dir.join(&file);
        let tmp = self.dir.join(format!("{file}.tmp-{}", std::process::id()));
        // 1. Temporary sibling, written and flushed.
        {
            let mut f = std::fs::File::create(&tmp).map_err(|e| io_err(&tmp, e))?;
            if fault == Some(Fault::DiskFull) {
                let _ = std::fs::remove_file(&tmp);
                return Err(EngineError::Io(format!("{}: no space left on device", tmp.display())));
            }
            f.write_all(text.as_bytes()).map_err(|e| io_err(&tmp, e))?;
            f.sync_all().map_err(|e| io_err(&tmp, e))?;
        }
        // 2. Validate what is on disk before anything points at it.
        let on_disk = std::fs::read(&tmp).map_err(|e| io_err(&tmp, e))?;
        let checksum = fnv1a64(&on_disk);
        if Session::from_json(std::str::from_utf8(&on_disk).unwrap_or("")).is_err() {
            let _ = std::fs::remove_file(&tmp);
            return Err(EngineError::Io(format!("{}: written snapshot does not parse", tmp.display())));
        }
        if fault == Some(Fault::BeforeRename) {
            return Err(EngineError::Io("fault: killed before rename".into()));
        }
        // 3. Into place (atomic on the same volume), directory flushed.
        std::fs::rename(&tmp, &final_path).map_err(|e| io_err(&final_path, e))?;
        sync_dir(&self.dir);
        if fault == Some(Fault::BeforeIndex) {
            return Err(EngineError::Io("fault: killed before index".into()));
        }
        // 4. Publish through the index (temporary, flushed, renamed).
        let generation = Generation {
            number,
            file,
            bytes: on_disk.len() as u64,
            checksum,
            unix_time: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()),
            session_name: session.name.clone(),
            revision: req.revision,
            plugin_state_age_s: req.plugin_state_age_s,
        };
        index.format = 1;
        index.generations.insert(0, generation.clone());
        let itmp = self.dir.join(format!("{INDEX_FILE}.tmp-{}", std::process::id()));
        {
            let mut f = std::fs::File::create(&itmp).map_err(|e| io_err(&itmp, e))?;
            let text = serde_json::to_string_pretty(&index).map_err(|e| EngineError::Io(e.to_string()))?;
            f.write_all(text.as_bytes()).map_err(|e| io_err(&itmp, e))?;
            f.sync_all().map_err(|e| io_err(&itmp, e))?;
        }
        if fault == Some(Fault::BeforeIndexRename) {
            return Err(EngineError::Io("fault: killed before index rename".into()));
        }
        std::fs::rename(&itmp, self.index_path()).map_err(|e| io_err(&self.index_path(), e))?;
        sync_dir(&self.dir);
        // 5. Only now: retention (never the newest, never below one known-good generation).
        let pruned = self.prune(&mut index);
        Ok(Committed { generation, media_written, pruned })
    }

    /// Keeps the newest `retain` generations within the byte budget; rewrites the index when
    /// something was removed. Never touches the newest generation.
    fn prune(&self, index: &mut Index) -> usize {
        let mut keep = Vec::new();
        let mut total = 0u64;
        let mut removed = 0usize;
        for (i, g) in index.generations.iter().enumerate() {
            total = total.saturating_add(g.bytes);
            if i == 0 || (keep.len() < self.retain && total <= self.budget_bytes) {
                keep.push(g.clone());
            } else {
                let _ = std::fs::remove_file(self.dir.join(&g.file));
                removed += 1;
            }
        }
        if removed > 0 {
            index.generations = keep;
            let itmp = self.dir.join(format!("{INDEX_FILE}.prune-{}", std::process::id()));
            if let Ok(text) = serde_json::to_string_pretty(index)
                && std::fs::write(&itmp, text).is_ok()
            {
                let _ = std::fs::rename(&itmp, self.index_path());
            }
        }
        removed
    }

    /// Unsaved recorded sources go to `media/<name>_<id>.wav` once (existing files are reused);
    /// returns the session with media paths pointing there.
    fn write_media(&self, session: &Session) -> Result<(Session, usize)> {
        let media_dir = self.dir.join("media");
        let mut s = session.clone();
        let mut written = 0;
        for src in &mut s.sources {
            let abs = Path::new(&src.path).is_absolute();
            if !src.unsaved && (abs || std::fs::metadata(&src.path).is_ok()) {
                continue;
            }
            let Some(audio) = s.pool.get(src.id) else { continue };
            let target = media_dir.join(format!("{}_{}.wav", crate::io::sanitize_name(&src.name), src.id.0));
            if !target.exists() {
                std::fs::create_dir_all(&media_dir).map_err(|e| io_err(&media_dir, e))?;
                let opts = soundcraft_audio_io::EncodeOptions {
                    format: soundcraft_audio_io::FileFormat::Wav,
                    bit_depth: soundcraft_audio_io::BitDepth::Float32,
                    dither: false,
                    bwf: None,
                };
                let bytes = soundcraft_audio_io::encode(&audio.buffer, &opts).map_err(|e| EngineError::Io(e.to_string()))?;
                let tmp = media_dir.join(format!("{}_{}.wav.tmp-{}", crate::io::sanitize_name(&src.name), src.id.0, std::process::id()));
                {
                    let mut f = std::fs::File::create(&tmp).map_err(|e| io_err(&tmp, e))?;
                    f.write_all(&bytes).map_err(|e| io_err(&tmp, e))?;
                    f.sync_all().map_err(|e| io_err(&tmp, e))?;
                }
                std::fs::rename(&tmp, &target).map_err(|e| io_err(&target, e))?;
                written += 1;
            }
            src.path = target.to_string_lossy().into_owned();
            src.unsaved = false;
        }
        if written > 0 {
            sync_dir(&media_dir);
        }
        Ok((s, written))
    }

    /// The newest generation that is really on disk, parses and matches its checksum. Starts
    /// from the index; when the index is missing or stale, falls back to scanning `gen-*.scraft`
    /// files (newest number first) — a generation that was renamed into place but whose index
    /// write was interrupted is still recoverable, as it was fully validated before the rename.
    pub fn recover(&self) -> Option<Recovered> {
        let index = self.index();
        let from_index = index.generations.iter().find_map(|g| self.validate(&self.dir.join(&g.file), Some(g)));
        // Files the index does not name yet (index write interrupted): validated before their
        // rename, re-parsed now; the newest valid one wins when it is newer than the index's.
        let mut files = self.generation_files();
        files.sort_by_key(|f| std::cmp::Reverse(f.0));
        let newest_index = from_index.as_ref().map_or(0, |r| r.number);
        let from_disk = files.into_iter().filter(|(n, _)| *n > newest_index).find_map(|(_, p)| self.validate(&p, None));
        from_disk.or(from_index)
    }

    /// `(number, path)` of every `gen-*.scraft` file in the store.
    fn generation_files(&self) -> Vec<(u64, PathBuf)> {
        std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                let name = p.file_name()?.to_str()?;
                let n = name.strip_prefix("gen-")?.strip_suffix(".scraft")?.parse::<u64>().ok()?;
                Some((n, p))
            })
            .collect()
    }

    fn highest_on_disk(&self) -> u64 {
        self.generation_files().into_iter().map(|(n, _)| n).max().unwrap_or(0)
    }

    fn validate(&self, path: &Path, g: Option<&Generation>) -> Option<Recovered> {
        let bytes = std::fs::read(path).ok()?;
        if let Some(g) = g
            && (g.bytes != bytes.len() as u64 || g.checksum != fnv1a64(&bytes))
        {
            return None;
        }
        let session = Session::from_json(std::str::from_utf8(&bytes).ok()?).ok()?;
        let number = g.map(|g| g.number).or_else(|| path.file_name()?.to_str()?.strip_prefix("gen-")?.strip_suffix(".scraft")?.parse().ok())?;
        Some(Recovered {
            path: path.to_path_buf(),
            number,
            session,
            unix_time: g.map_or(0, |g| g.unix_time),
            plugin_state_age_s: g.and_then(|g| g.plugin_state_age_s),
        })
    }
}

/// A recoverable generation.
#[derive(Debug)]
pub struct Recovered {
    pub path: PathBuf,
    pub number: u64,
    pub session: Session,
    pub unix_time: u64,
    pub plugin_state_age_s: Option<u64>,
}

/// Opens a recovered generation into the engine (media decoded like a normal open); the engine
/// has no path afterwards, so the user chooses where to save.
pub fn open_recovered(e: &mut Engine, r: Recovered) -> Result<Vec<String>> {
    let missing = crate::io::open_session(e, &r.path.to_string_lossy())?;
    e.path = None;
    e.mark_dirty();
    Ok(missing)
}

/// FNV-1a 64-bit (dependency-free content check; not a security hash).
pub fn fnv1a64(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = std::fs::File::open(dir) {
        let _ = d.sync_all();
    }
    // Windows: a directory cannot be opened and flushed like this (std has no handle for it);
    // the rename itself is journaled by NTFS. Durability across power loss there is W pending.
    #[cfg(not(unix))]
    let _ = dir;
}

fn io_err(p: &Path, e: std::io::Error) -> EngineError {
    EngineError::Io(format!("{}: {e}", p.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sml-autosave-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn req(e: &Engine) -> SnapshotRequest {
        SnapshotRequest { session: e.session_arc(), revision: e.revision, plugin_state_age_s: Some(0) }
    }

    /// E02: every fault point leaves the previous generation and index intact; recovery returns
    /// the last committed, validated generation and never a partial file.
    #[test]
    fn faults_at_every_boundary_keep_the_previous_generation() {
        let dir = tmp("faults");
        let store = Store::new(&dir);
        let mut e = crate::demo::demo_engine();
        let c1 = store.snapshot(&req(&e)).unwrap();
        assert_eq!(c1.generation.number, 1);
        let g1 = store.recover().unwrap();
        assert_eq!(g1.number, 1);
        assert_eq!(g1.session.tracks.len(), e.session().tracks.len());
        for (k, fault) in [Fault::DiskFull, Fault::BeforeRename, Fault::BeforeIndex, Fault::BeforeIndexRename].into_iter().enumerate() {
            e.execute("mix.volume", &serde_json::json!({"track": "Kick", "db": -(k as f64 + 1.0)})).unwrap();
            let err = store.snapshot_with(&req(&e), Some(fault)).unwrap_err().to_string();
            assert!(!err.is_empty(), "{fault:?} fails visibly");
            let r = store.recover().unwrap();
            match fault {
                // The generation reached disk fully validated: recoverable even without the index.
                Fault::BeforeIndex | Fault::BeforeIndexRename => {
                    assert!(r.number > 1, "{fault:?}: validated generation on disk is recovered ({})", r.number);
                }
                _ => assert_eq!(r.number, 1, "{fault:?}: previous generation untouched"),
            }
            // No temporary file is ever reported, and the previous generation file is intact.
            assert!(!r.path.to_string_lossy().contains(".tmp"));
            let g1_bytes = std::fs::read(dir.join("gen-000001.scraft")).unwrap();
            assert_eq!(fnv1a64(&g1_bytes), c1.generation.checksum, "{fault:?}: gen 1 unchanged");
        }
        // After a stale index (BeforeIndexRename left index at an older state), a normal
        // snapshot continues the numbering past every file on disk.
        let c = store.snapshot(&req(&e)).unwrap();
        assert!(c.generation.number >= 2);
        assert_eq!(store.recover().unwrap().number, c.generation.number);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A corrupt newest generation (truncated after commit, e.g. a partial write the OS never
    /// flushed) fails its checksum and recovery falls back to the one before it.
    #[test]
    fn corrupt_newest_generation_falls_back() {
        let dir = tmp("corrupt");
        let store = Store::new(&dir);
        let mut e = crate::demo::demo_engine();
        store.snapshot(&req(&e)).unwrap();
        e.execute("mix.volume", &serde_json::json!({"track": "Kick", "db": -9.0})).unwrap();
        let c2 = store.snapshot(&req(&e)).unwrap();
        let p = dir.join(&c2.generation.file);
        let bytes = std::fs::read(&p).unwrap();
        std::fs::write(&p, &bytes[..bytes.len() / 2]).unwrap();
        let r = store.recover().unwrap();
        assert_eq!(r.number, 1, "truncated generation 2 rejected by checksum");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Retention keeps the newest `retain` generations and respects the byte budget, never
    /// removing the newest one; the index always lists existing files only.
    #[test]
    fn retention_and_budget_keep_the_newest() {
        let dir = tmp("retain");
        let mut store = Store::new(&dir);
        store.retain = 3;
        let mut e = crate::demo::demo_engine();
        for k in 0..6 {
            e.execute("mix.volume", &serde_json::json!({"track": "Kick", "db": -(k as f64)})).unwrap();
            store.snapshot(&req(&e)).unwrap();
        }
        let idx = store.index();
        assert_eq!(idx.generations.len(), 3);
        assert_eq!(idx.generations[0].number, 6);
        for g in &idx.generations {
            assert!(dir.join(&g.file).exists());
        }
        assert!(!dir.join("gen-000001.scraft").exists());
        // Tiny budget: only the newest survives.
        store.budget_bytes = 1;
        let c = store.snapshot(&req(&e)).unwrap();
        assert_eq!(store.index().generations.len(), 1);
        assert_eq!(store.recover().unwrap().number, c.generation.number);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Unsaved recorded media is written once per source (reused by later generations) and the
    /// recovered session points at it; opening the recovery decodes it.
    #[test]
    fn recorded_media_is_written_once_and_recoverable() {
        use soundcraft_model::{ChannelFormat, TrackKind};
        let dir = tmp("media");
        let store = Store::new(&dir);
        let mut e = Engine::default();
        let t = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
        e.session_mut().track_mut(t).unwrap().mixer.record_arm = true;
        crate::io::add_recording(&mut e, 1000, vec![vec![0.25; 4800]], 48_000).unwrap();
        let c1 = store.snapshot(&req(&e)).unwrap();
        assert_eq!(c1.media_written, 1);
        let c2 = store.snapshot(&req(&e)).unwrap();
        assert_eq!(c2.media_written, 0, "reused");
        let r = store.recover().unwrap();
        let mut e2 = Engine::default();
        let missing = open_recovered(&mut e2, r).unwrap();
        assert!(missing.is_empty(), "{missing:?}");
        assert!(e2.path.is_none() && e2.is_dirty());
        let src = e2.session().sources[0].id;
        assert_eq!(e2.session().pool.get(src).unwrap().channel(0).unwrap()[10], 0.25);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
