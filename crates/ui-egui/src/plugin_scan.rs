//! Background plugin scanning (R06): CLAP, VST3 and (macOS) AU registries are scanned on a
//! worker thread so opening a plugin menu never blocks the UI. A scan is not started while
//! recording (its disk/library loading could disturb the capture writer) and can be cancelled
//! between formats (a single format's scan runs to completion once begun). Results land in the
//! host crates' own caches; menus read them only after the scan finished.
//!
//! Scanning is isolated (R07, HF12): when the process has a scan service
//! (`soundcraft_mix::scan_jobs`, installed by the app at start) this scanner submits a job to it,
//! like the control channel does, so the UI, the start-up scan and remote requests share one
//! owner: each bundle is loaded by a separate worker process (`soundcraft --scan-plugin`), and a
//! plugin that crashes or hangs while loading is quarantined instead of taking the app down.
//! Without a service (agents' UI test runs) the in-process scan is used — a documented
//! compatibility mode, not containment.
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
    /// The scan-service job serving the last isolated scan (for progress display).
    pub job: Arc<std::sync::atomic::AtomicU64>,
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
        if let Some(svc) = soundcraft_mix::scan_jobs::installed() {
            if recording || matches!(self.phase(), Phase::Running | Phase::Done) {
                return false;
            }
            self.isolated.store(true, Ordering::Release);
            let submitted = svc.submit(false);
            self.job.store(submitted.job.id, Ordering::Release);
            return self.follow(svc, submitted.job.id);
        }
        self.isolated.store(false, Ordering::Release);
        self.start_with(recording, default_formats())
    }

    /// Follows a service job on a waiting thread: the phase becomes Done when it ends and the
    /// quarantine list is refreshed.
    #[cfg(not(target_arch = "wasm32"))]
    fn follow(&self, svc: Arc<soundcraft_mix::scan_jobs::ScanService>, id: u64) -> bool {
        let quarantined = Arc::clone(&self.quarantined);
        let job: Box<dyn FnOnce() + Send> = Box::new(move || {
            // Bounded per bundle by the worker deadline; the wait itself only watches.
            let _ = svc.wait(id, std::time::Duration::from_secs(24 * 3600));
            if let Ok(mut q) = quarantined.lock() {
                *q = svc.quarantine().into_iter().map(|q| (q.path, q.reason)).collect();
            }
        });
        self.start_jobs(false, vec![job])
    }

    /// The service job behind the current isolated scan (progress for the menus).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn job_status(&self) -> Option<soundcraft_mix::scan_jobs::JobSnapshot> {
        let id = self.job.load(Ordering::Acquire);
        if id == 0 {
            return None;
        }
        soundcraft_mix::scan_jobs::installed()?.status(Some(id))
    }

    /// Retries the quarantined bundle `path` now (a retry job on the scan service; other
    /// quarantines and cached results are kept). False without a service or while scanning.
    pub fn re_enable(&self, path: &std::path::Path) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(svc) = soundcraft_mix::scan_jobs::installed() {
            if self.phase() == Phase::Running {
                return false;
            }
            if let Ok(mut q) = self.quarantined.lock() {
                q.retain(|(p, _)| p != path);
            }
            let submitted = svc.retry(vec![path.to_path_buf()]);
            self.job.store(submitted.job.id, Ordering::Release);
            self.phase.store(0, Ordering::Release);
            return self.follow(svc, submitted.job.id);
        }
        let _ = path;
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
