//! Background plugin scanning (R06): CLAP, VST3 and (macOS) AU registries are scanned on a
//! worker thread so opening a plugin menu never blocks the UI. A scan is not started while
//! recording (its disk/library loading could disturb the capture writer) and can be cancelled
//! between formats (a single format's scan runs to completion once begun). Results land in the
//! host crates' own caches; menus read them only after the scan finished.
//!
//! Scanning is isolated (R07): with a cache path configured, each bundle is loaded by a separate
//! worker process (`soundcraft --scan-plugin`), so a plugin that crashes or hangs while loading
//! is quarantined instead of taking the app down. Without a cache path (agents' test runs, the
//! CLI) the in-process scan is used — a documented compatibility mode, not containment.
//! Hosting for playback is in-process either way (runtime containment: E03).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// Scan phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Running,
    Done,
    Cancelled,
}

#[derive(Debug, Default, Clone)]
pub struct Scanner {
    phase: Arc<AtomicU8>,
    cancel: Arc<AtomicBool>,
    /// Formats finished so far (0..=3), for progress display.
    progress: Arc<AtomicU8>,
    /// Scan cache file; `Some` enables the isolated (out-of-process) scan.
    pub cache_path: Option<std::path::PathBuf>,
    /// Quarantined bundles of the last isolated scan (path, reason), for the UI.
    pub quarantined: Arc<std::sync::Mutex<Vec<(std::path::PathBuf, String)>>>,
    /// True while the last scan used worker processes.
    pub isolated: Arc<AtomicBool>,
}

impl Scanner {
    pub fn phase(&self) -> Phase {
        match self.phase.load(Ordering::Acquire) {
            1 => Phase::Running,
            2 => Phase::Done,
            3 => Phase::Cancelled,
            _ => Phase::Idle,
        }
    }

    pub fn progress(&self) -> u8 {
        self.progress.load(Ordering::Acquire)
    }

    /// Starts a scan unless one is running/done or `recording` is true. Returns whether started.
    /// Isolated when a cache path is set and this program can act as its own worker.
    pub fn start(&self, recording: bool) -> bool {
        if soundcraft_mix::hosted_plugins_disabled() {
            return false;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(cache_path) = self.cache_path.clone()
            && let Some(worker) = soundcraft_mix::scan_isolated::Worker::this_program()
        {
            let quarantined = Arc::clone(&self.quarantined);
            let isolated = Arc::clone(&self.isolated);
            isolated.store(true, Ordering::Release);
            let job: Box<dyn FnOnce() + Send> = Box::new(move || {
                let mut cache = soundcraft_mix::scan_isolated::Cache::load(&cache_path);
                let dirs = soundcraft_mix::scan_isolated::default_dirs();
                let report = soundcraft_mix::scan_isolated::scan_dirs(&worker, &dirs, &mut cache, Some(&cache_path), false);
                soundcraft_mix::scan_isolated::adopt(&report);
                if let Ok(mut q) = quarantined.lock() {
                    *q = report.quarantined.iter().map(|(p, q)| (p.clone(), q.to_string())).collect();
                }
                log::info!(
                    "isolated plugin scan: {} CLAP, {} VST3, {} scanned, {} cached, {} quarantined",
                    report.clap.len(),
                    report.vst3.len(),
                    report.scanned,
                    report.cached,
                    report.quarantined.len()
                );
            });
            return self.start_jobs(recording, vec![job]);
        }
        self.isolated.store(false, Ordering::Release);
        self.start_with(recording, default_formats())
    }

    /// Clears the quarantine of `path` in the cache so the next scan retries it.
    pub fn re_enable(&self, path: &std::path::Path) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(cache_path) = &self.cache_path {
            let mut cache = soundcraft_mix::scan_isolated::Cache::load(cache_path);
            let removed = cache.forget(path);
            let _ = cache.save(cache_path);
            if let Ok(mut q) = self.quarantined.lock() {
                q.retain(|(p, _)| p != path);
            }
            return removed;
        }
        false
    }

    /// Starts scanning `formats` (each a blocking scan function) on a worker thread.
    pub fn start_with(&self, recording: bool, formats: Vec<fn()>) -> bool {
        let jobs: Vec<Box<dyn FnOnce() + Send>> = formats.into_iter().map(|f| Box::new(f) as Box<dyn FnOnce() + Send>).collect();
        self.start_jobs(recording, jobs)
    }

    fn start_jobs(&self, recording: bool, formats: Vec<Box<dyn FnOnce() + Send>>) -> bool {
        if recording || matches!(self.phase(), Phase::Running | Phase::Done) {
            return false;
        }
        self.cancel.store(false, Ordering::Release);
        self.progress.store(0, Ordering::Release);
        self.phase.store(1, Ordering::Release);
        let (phase, cancel, progress) = (Arc::clone(&self.phase), Arc::clone(&self.cancel), Arc::clone(&self.progress));
        let spawned = std::thread::Builder::new().name("sml-plugin-scan".into()).spawn(move || {
            for f in formats {
                if cancel.load(Ordering::Acquire) {
                    phase.store(3, Ordering::Release);
                    return;
                }
                f();
                progress.fetch_add(1, Ordering::AcqRel);
            }
            phase.store(2, Ordering::Release);
        });
        if spawned.is_err() {
            self.phase.store(0, Ordering::Release);
            return false;
        }
        true
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    /// Runs the scan again (not while one is running): new or changed bundles are scanned,
    /// cached ones reused, quarantines kept unless re-enabled.
    pub fn rescan(&self) -> bool {
        if self.phase() == Phase::Running {
            return false;
        }
        self.phase.store(0, Ordering::Release);
        self.start(false)
    }
}

fn default_formats() -> Vec<fn()> {
    let mut v: Vec<fn()> = vec![
        || {
            let _ = soundcraft_clap_host::scan();
        },
        || {
            let _ = soundcraft_vst3_host::scan();
        },
    ];
    if cfg!(target_os = "macos") {
        v.push(|| {
            let _ = soundcraft_au_host::scan();
        });
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait(s: &Scanner) {
        for _ in 0..500 {
            if matches!(s.phase(), Phase::Done | Phase::Cancelled) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    #[test]
    fn scan_runs_in_background_and_is_not_started_while_recording() {
        let s = Scanner::default();
        assert!(!s.start_with(true, vec![|| {}]), "never during recording");
        assert_eq!(s.phase(), Phase::Idle);
        assert!(s.start_with(false, vec![|| {}, || {}]));
        wait(&s);
        assert_eq!((s.phase(), s.progress()), (Phase::Done, 2));
        assert!(!s.start_with(false, vec![|| {}]), "done scans are cached");
    }

    #[test]
    fn scan_can_be_cancelled_between_formats() {
        let s = Scanner::default();
        s.start_with(false, vec![|| std::thread::sleep(std::time::Duration::from_millis(50)), || {}, || {}]);
        s.cancel();
        wait(&s);
        assert_eq!(s.phase(), Phase::Cancelled);
        assert!(s.progress() < 3);
        assert!(s.start_with(false, vec![|| {}]), "a cancelled scan can be restarted");
    }
}
