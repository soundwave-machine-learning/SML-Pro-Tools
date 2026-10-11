//! Background plugin scanning (R06): CLAP, VST3 and (macOS) AU registries are scanned on a
//! worker thread so opening a plugin menu never blocks the UI. A scan is not started while
//! recording (its disk/library loading could disturb the capture writer) and can be cancelled
//! between formats (a single format's scan runs to completion once begun). Results land in the
//! host crates' own caches; menus read them only after the scan finished.
//!
//! Known limit: plugin libraries are loaded on the worker thread for scanning; a plugin that
//! insists on main-thread scanning may misbehave there (in-process hosting has no crash
//! containment — see the user guide).

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
    pub fn start(&self, recording: bool) -> bool {
        self.start_with(recording, default_formats())
    }

    /// Starts scanning `formats` (each a blocking scan function) on a worker thread.
    pub fn start_with(&self, recording: bool, formats: Vec<fn()>) -> bool {
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
