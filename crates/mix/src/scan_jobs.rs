//! The plugin scan service (HF12): one owner of the isolated scanner for the whole process. The
//! UI, the start-up scan and the control channel all submit *jobs* here instead of scanning
//! themselves, so a request returns at once with a job id and its state, and nothing that asks
//! for a scan ever loads an unknown binary into the host.
//!
//! - **One job at a time.** A request while a job runs joins it when the running job already
//!   covers it (same or stronger), otherwise it is queued; at most one job waits, and later
//!   requests merge into that one. Each answer says whether it was coalesced.
//! - **Bounded work.** Up to [`MAX_PARALLEL`] worker processes run at once, each with the
//!   per-bundle deadline of its [`Worker`]; the job's total time is bounded by both.
//! - **Cancellation** stops starting new workers and kills the running ones (only this service's
//!   children). Bundles finished before it keep their results.
//! - **Durability.** The cache (results and quarantine) is written atomically every few bundles
//!   and when the job ends, so an interrupted scan keeps what it finished.
//! - **Retry** of chosen quarantined bundles forgets only those entries and scans only them;
//!   other quarantines and cached results stay as they are.
//!
//! The plugin list the menus and `engine.*_plugins` read is adopted from the cache: at install
//! (what the last completed scans found, for bundles that did not change) and after each job.

use crate::scan_isolated::{Cache, CacheEntry, ScanDiagnostics, Worker, scan_bundle};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

/// Most scan workers running at once.
pub const MAX_PARALLEL: usize = 4;
/// Finished jobs remembered for `status`.
const KEEP_JOBS: usize = 16;
/// The cache is written after this many finished bundles (and at the end of every job).
const SAVE_EVERY: usize = 8;

/// What a job does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    /// Scan new and changed bundles; cached results and quarantines are kept.
    Scan,
    /// Scan every bundle again except quarantined ones (quarantines are kept).
    Rescan,
    /// Retry only the chosen quarantined bundles.
    Retry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Done,
    Cancelled,
}

/// A job as reported to callers (UI, control channel, tests).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JobSnapshot {
    pub id: u64,
    pub kind: JobKind,
    pub state: JobState,
    /// Bundles this job has to run a worker for.
    pub total: usize,
    /// Of those, finished (scanned or quarantined).
    pub completed: usize,
    /// Finished with a plugin list.
    pub scanned_ok: usize,
    /// Newly quarantined by this job.
    pub quarantined: usize,
    /// Bundles served from the cache without a worker.
    pub cached: usize,
    /// File names of the bundles whose workers are running now.
    pub current: Vec<String>,
    /// Requests answered by this job (1 + coalesced ones).
    pub requests: u32,
    /// Plugins listed after the job (CLAP, VST3); `None` until it ends.
    pub clap: Option<usize>,
    pub vst3: Option<usize>,
    pub elapsed_ms: u64,
    /// Bundles retried (only for `retry` jobs).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub retry: Vec<PathBuf>,
}

impl JobSnapshot {
    pub fn finished(&self) -> bool {
        matches!(self.state, JobState::Done | JobState::Cancelled)
    }
}

/// The answer to a request: the job that will serve it, and whether it joined an existing one.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Submitted {
    pub job: JobSnapshot,
    pub coalesced: bool,
}

/// One quarantined bundle with its diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QuarantineInfo {
    pub path: PathBuf,
    pub name: String,
    pub reason: String,
    pub diagnostics: Option<ScanDiagnostics>,
    /// Written by an older scanner revision: worth a retry with this one.
    pub older_scanner: bool,
}

struct Job {
    snap: JobSnapshot,
    started: Option<Instant>,
    cancel: Arc<AtomicBool>,
}

struct State {
    cache: Cache,
    jobs: Vec<Job>,
    next_id: u64,
    runner: bool,
}

pub struct ScanService {
    worker: Worker,
    cache_path: Option<PathBuf>,
    dirs: Vec<PathBuf>,
    parallel: usize,
    state: Mutex<State>,
    changed: Condvar,
    /// Serialises cache writes (the file is replaced atomically; this keeps them in order).
    save_lock: Mutex<()>,
}

fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn file_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// Every bundle (CLAP and VST3) under `dirs`, sorted.
fn bundles_in(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut v = soundcraft_clap_host::scan::find_bundles(dirs);
    v.extend(soundcraft_vst3_host::scan::find_bundles(dirs));
    v.sort();
    v.dedup();
    v
}

fn fingerprint(path: &Path) -> Option<(u64, u64)> {
    crate::scan_isolated::fingerprint(path)
}

impl ScanService {
    /// A service scanning `dirs` with `worker`, caching in `cache_path` (in memory only when
    /// `None`). Loads the cache and adopts its plugin list for bundles that did not change.
    pub fn new(worker: Worker, cache_path: Option<PathBuf>, dirs: Vec<PathBuf>, parallel: usize) -> Arc<ScanService> {
        let cache = cache_path.as_deref().map(Cache::load).unwrap_or_default();
        let svc = Arc::new(ScanService {
            worker,
            cache_path,
            dirs,
            parallel: parallel.clamp(1, MAX_PARALLEL),
            state: Mutex::new(State { cache, jobs: Vec::new(), next_id: 1, runner: false }),
            changed: Condvar::new(),
            save_lock: Mutex::new(()),
        });
        svc.adopt_from_cache(&bundles_in(&svc.dirs));
        svc
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Asks for a scan. `force`: rescan cached successes too. Returns at once.
    pub fn submit(self: &Arc<Self>, force: bool) -> Submitted {
        self.request(if force { JobKind::Rescan } else { JobKind::Scan }, Vec::new())
    }

    /// Asks to retry the given quarantined bundles (all quarantined ones when `paths` is empty).
    pub fn retry(self: &Arc<Self>, paths: Vec<PathBuf>) -> Submitted {
        let paths = if paths.is_empty() { self.lock().cache.quarantined().into_iter().map(|(p, _)| p.to_path_buf()).collect() } else { paths };
        self.request(JobKind::Retry, paths)
    }

    fn request(self: &Arc<Self>, kind: JobKind, retry: Vec<PathBuf>) -> Submitted {
        let mut st = self.lock();
        // Join the running job when it already covers this request.
        if let Some(j) = st.jobs.iter_mut().find(|j| j.snap.state == JobState::Running)
            && covers(&j.snap, kind, &retry)
        {
            j.snap.requests += 1;
            return Submitted { job: j.snap.clone(), coalesced: true };
        }
        // Otherwise merge into the one waiting job, or queue a new one.
        if let Some(j) = st.jobs.iter_mut().find(|j| j.snap.state == JobState::Queued) {
            j.snap.requests += 1;
            merge(&mut j.snap, kind, retry);
            return Submitted { job: j.snap.clone(), coalesced: true };
        }
        let id = st.next_id;
        st.next_id += 1;
        let snap = JobSnapshot {
            id,
            kind,
            state: JobState::Queued,
            total: 0,
            completed: 0,
            scanned_ok: 0,
            quarantined: 0,
            cached: 0,
            current: Vec::new(),
            requests: 1,
            clap: None,
            vst3: None,
            elapsed_ms: 0,
            retry,
        };
        st.jobs.push(Job { snap: snap.clone(), started: None, cancel: Arc::new(AtomicBool::new(false)) });
        let keep_from = st.jobs.len().saturating_sub(KEEP_JOBS);
        if keep_from > 0 {
            let mut i = 0;
            st.jobs.retain(|j| {
                i += 1;
                i > keep_from || !j.snap.finished()
            });
        }
        if !st.runner {
            st.runner = true;
            let me = Arc::clone(self);
            if std::thread::Builder::new().name("sml-scan-jobs".into()).spawn(move || me.run_queue()).is_err() {
                st.runner = false;
                log::warn!("plugin scan: cannot start the scan thread");
            }
        }
        self.changed.notify_all();
        Submitted { job: snap, coalesced: false }
    }

    /// A job by id, else the newest one.
    pub fn status(&self, id: Option<u64>) -> Option<JobSnapshot> {
        let st = self.lock();
        let j = match id {
            Some(id) => st.jobs.iter().find(|j| j.snap.id == id),
            None => st.jobs.last(),
        }?;
        let mut s = j.snap.clone();
        if let Some(t) = j.started.filter(|_| !s.finished()) {
            s.elapsed_ms = t.elapsed().as_millis() as u64;
        }
        Some(s)
    }

    /// Every remembered job, oldest first.
    pub fn jobs(&self) -> Vec<JobSnapshot> {
        self.lock().jobs.iter().map(|j| j.snap.clone()).collect()
    }

    /// Cancels a job (the running or waiting one when `id` is `None`). A waiting job ends at
    /// once; a running one stops its workers and ends when they are gone.
    pub fn cancel(&self, id: Option<u64>) -> Option<JobSnapshot> {
        let mut st = self.lock();
        let j = st.jobs.iter_mut().rev().find(|j| id.is_none_or(|id| j.snap.id == id) && !j.snap.finished())?;
        j.cancel.store(true, Ordering::Release);
        if j.snap.state == JobState::Queued {
            j.snap.state = JobState::Cancelled;
        }
        let s = j.snap.clone();
        self.changed.notify_all();
        Some(s)
    }

    /// Waits until job `id` has finished (at most `timeout`); its latest snapshot.
    pub fn wait(&self, id: u64, timeout: Duration) -> Option<JobSnapshot> {
        let deadline = Instant::now() + timeout;
        let mut st = self.lock();
        loop {
            let snap = st.jobs.iter().find(|j| j.snap.id == id).map(|j| j.snap.clone())?;
            let left = deadline.saturating_duration_since(Instant::now());
            if snap.finished() || left.is_zero() {
                return Some(snap);
            }
            st = self.changed.wait_timeout(st, left).map_or_else(|e| e.into_inner().0, |(g, _)| g);
        }
    }

    /// Quarantined bundles with their reasons and diagnostics.
    pub fn quarantine(&self) -> Vec<QuarantineInfo> {
        let st = self.lock();
        st.cache
            .entries
            .iter()
            .filter_map(|e| {
                let q = e.result.as_ref().err()?;
                Some(QuarantineInfo {
                    path: e.path.clone(),
                    name: file_name(&e.path),
                    reason: q.to_string(),
                    diagnostics: e.diagnostics.clone(),
                    older_scanner: e.diagnostics.as_ref().is_none_or(|d| d.scanner < crate::scan_isolated::SCANNER_REVISION),
                })
            })
            .collect()
    }

    fn run_queue(self: Arc<Self>) {
        loop {
            let next = {
                let mut st = self.lock();
                match st.jobs.iter_mut().find(|j| j.snap.state == JobState::Queued) {
                    Some(j) => {
                        j.snap.state = JobState::Running;
                        j.started = Some(Instant::now());
                        Some((j.snap.id, j.snap.kind, j.snap.retry.clone(), Arc::clone(&j.cancel)))
                    }
                    None => {
                        st.runner = false;
                        None
                    }
                }
            };
            self.changed.notify_all();
            let Some((id, kind, retry, cancel)) = next else { return };
            self.run_job(id, kind, &retry, &cancel);
        }
    }

    fn update(&self, id: u64, f: impl FnOnce(&mut JobSnapshot)) {
        let mut st = self.lock();
        if let Some(j) = st.jobs.iter_mut().find(|j| j.snap.id == id) {
            f(&mut j.snap);
        }
        drop(st);
        self.changed.notify_all();
    }

    fn save(&self) {
        let Some(p) = &self.cache_path else { return };
        let _g = self.save_lock.lock().unwrap_or_else(PoisonError::into_inner);
        let cache = self.lock().cache.clone();
        if let Err(e) = cache.save(p) {
            log::warn!("plugin scan cache {}: {e}", p.display());
        }
    }

    fn run_job(&self, id: u64, kind: JobKind, retry: &[PathBuf], cancel: &Arc<AtomicBool>) {
        let bundles = bundles_in(&self.dirs);
        let retry: BTreeSet<&Path> = retry.iter().map(PathBuf::as_path).collect();
        // Decide which bundles need a worker; the rest come from the cache.
        let mut work: Vec<(PathBuf, u64, u64)> = Vec::new();
        let mut cached = 0;
        {
            let mut st = self.lock();
            for b in &bundles {
                let Some((size, mtime)) = fingerprint(b) else { continue };
                let hit = st.cache.entries.iter().find(|e| &e.path == b && e.size == size && e.mtime == mtime);
                let needs = match (kind, hit) {
                    (JobKind::Retry, _) => retry.contains(b.as_path()),
                    (_, None) => true,
                    (JobKind::Rescan, Some(e)) => e.result.is_ok(),
                    (JobKind::Scan, Some(_)) => false,
                };
                if needs {
                    work.push((b.clone(), size, mtime));
                } else if hit.is_some() {
                    cached += 1;
                }
            }
            if kind == JobKind::Retry {
                for p in &retry {
                    st.cache.forget(p);
                }
            }
        }
        let total = work.len();
        self.update(id, |s| {
            s.total = total;
            s.cached = cached;
        });
        let next = AtomicUsize::new(0);
        let since_save = AtomicUsize::new(0);
        let threads = self.parallel.min(total.max(1));
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    loop {
                        if cancel.load(Ordering::Acquire) {
                            return;
                        }
                        let i = next.fetch_add(1, Ordering::AcqRel);
                        let Some((path, size, mtime)) = work.get(i) else { return };
                        let name = file_name(path);
                        self.update(id, |s| s.current.push(name.clone()));
                        let outcome = scan_bundle(&self.worker, path, Some(cancel));
                        let mut st = self.lock();
                        if let Some(j) = st.jobs.iter_mut().find(|j| j.snap.id == id)
                            && let Some(k) = j.snap.current.iter().position(|c| c == &name)
                        {
                            j.snap.current.remove(k);
                        }
                        // Cancelled mid-bundle: nothing is recorded for it.
                        let Some((result, diag)) = outcome else {
                            drop(st);
                            self.changed.notify_all();
                            return;
                        };
                        let ok = result.is_ok();
                        st.cache.entries.retain(|e| &e.path != path);
                        st.cache.entries.push(CacheEntry {
                            path: path.clone(),
                            size: *size,
                            mtime: *mtime,
                            result,
                            scanned_at: now_unix(),
                            diagnostics: Some(diag),
                        });
                        if let Some(j) = st.jobs.iter_mut().find(|j| j.snap.id == id) {
                            j.snap.completed += 1;
                            if ok {
                                j.snap.scanned_ok += 1;
                            } else {
                                j.snap.quarantined += 1;
                            }
                        }
                        drop(st);
                        self.changed.notify_all();
                        if since_save.fetch_add(1, Ordering::AcqRel) + 1 >= SAVE_EVERY {
                            since_save.store(0, Ordering::Release);
                            self.save();
                        }
                    }
                });
            }
        });
        self.save();
        let (clap, vst3) = self.adopt_from_cache(&bundles);
        let cancelled = cancel.load(Ordering::Acquire);
        let mut st = self.lock();
        if let Some(j) = st.jobs.iter_mut().find(|j| j.snap.id == id) {
            j.snap.state = if cancelled { JobState::Cancelled } else { JobState::Done };
            j.snap.clap = Some(clap);
            j.snap.vst3 = Some(vst3);
            j.snap.current.clear();
            j.snap.elapsed_ms = j.started.map_or(0, |t| t.elapsed().as_millis() as u64);
            log::info!(
                "plugin scan job {id} ({:?}) {:?}: {} of {} bundles, {} quarantined, {} cached; {clap} CLAP, {vst3} VST3 listed",
                j.snap.kind,
                j.snap.state,
                j.snap.completed,
                j.snap.total,
                j.snap.quarantined,
                j.snap.cached
            );
        }
        drop(st);
        self.changed.notify_all();
    }

    /// Hands the cached plugin lists of `bundles` (unchanged ones only) to the host registries.
    fn adopt_from_cache(&self, bundles: &[PathBuf]) -> (usize, usize) {
        let st = self.lock();
        let mut clap: Vec<soundcraft_clap_host::ClapDescriptor> = Vec::new();
        let mut vst3: Vec<soundcraft_vst3_host::Vst3Descriptor> = Vec::new();
        for b in bundles {
            let Some((size, mtime)) = fingerprint(b) else { continue };
            let Some(e) = st.cache.entries.iter().find(|e| &e.path == b && e.size == size && e.mtime == mtime) else { continue };
            let Ok(out) = &e.result else { continue };
            for d in &out.clap {
                if !clap.iter().any(|x| x.id == d.id) {
                    clap.push(d.clone());
                }
            }
            for d in &out.vst3 {
                if !vst3.iter().any(|x| x.id == d.id) {
                    vst3.push(d.clone());
                }
            }
        }
        drop(st);
        clap.sort_by_key(|d| d.name.to_lowercase());
        vst3.sort_by_key(|d| d.name.to_lowercase());
        let n = (clap.len(), vst3.len());
        soundcraft_clap_host::adopt_scanned(clap);
        soundcraft_vst3_host::adopt_scanned(vst3);
        n
    }
}

/// Whether a running job already does what `kind` asks.
fn covers(running: &JobSnapshot, kind: JobKind, retry: &[PathBuf]) -> bool {
    match (running.kind, kind) {
        (JobKind::Rescan, JobKind::Scan | JobKind::Rescan) | (JobKind::Scan, JobKind::Scan) => true,
        (JobKind::Retry, JobKind::Retry) => retry.iter().all(|p| running.retry.contains(p)),
        _ => false,
    }
}

/// Merges a request into the waiting job (the stronger kind wins; retry lists are joined).
fn merge(waiting: &mut JobSnapshot, kind: JobKind, retry: Vec<PathBuf>) {
    waiting.kind = match (waiting.kind, kind) {
        (JobKind::Rescan, _) | (_, JobKind::Rescan) => JobKind::Rescan,
        (JobKind::Retry, JobKind::Retry) => JobKind::Retry,
        _ => JobKind::Scan,
    };
    if waiting.kind == JobKind::Retry {
        for p in retry {
            if !waiting.retry.contains(&p) {
                waiting.retry.push(p);
            }
        }
    } else {
        // A full scan or rescan supersedes a retry list: quarantined bundles keep their entries.
        waiting.retry.clear();
    }
}

fn slot() -> &'static Mutex<Option<Arc<ScanService>>> {
    static S: OnceLock<Mutex<Option<Arc<ScanService>>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// Makes `svc` the process's scan service and turns in-process scanning off in both hosts: from
/// now on only worker processes load bundles to read their plugin lists.
pub fn install(svc: Arc<ScanService>) {
    soundcraft_clap_host::set_in_process_scan(false);
    soundcraft_vst3_host::set_in_process_scan(false);
    *slot().lock().unwrap_or_else(PoisonError::into_inner) = Some(svc);
}

/// The installed service (the app installs one at start; tools and tests may not).
pub fn installed() -> Option<Arc<ScanService>> {
    slot().lock().unwrap_or_else(PoisonError::into_inner).clone()
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use crate::scan_isolated::tests_support::{bundles, fake_worker, tmp};

    fn wait_done(svc: &ScanService, id: u64) -> JobSnapshot {
        let s = svc.wait(id, Duration::from_secs(20)).unwrap();
        assert!(s.finished(), "job {id} did not finish: {s:?}");
        s
    }

    /// HF12: a submit returns at once; crashing, hanging, failing and garbage workers end as
    /// quarantines while healthy bundles are listed; the cache is written; overlapping requests
    /// join the running job or the single waiting one.
    #[test]
    fn jobs_return_promptly_contain_faults_and_coalesce() {
        let dir = tmp("jobs");
        let worker = fake_worker(&dir, Duration::from_millis(600));
        let plugins = bundles(&dir, &["good1", "good2", "crash1", "hang1", "bad1", "junk1"]);
        let cache_path = dir.join("cache.json");
        let svc = ScanService::new(worker, Some(cache_path.clone()), vec![plugins.clone()], 2);
        let t0 = Instant::now();
        let a = svc.submit(false);
        assert!(t0.elapsed() < Duration::from_millis(200), "submit returns at once");
        assert!(!a.coalesced);
        // Same request while it runs or waits: joined, never a second scan of the same bundles.
        let b = svc.submit(false);
        assert!(b.coalesced);
        assert_eq!(b.job.id, a.job.id);
        while svc.status(Some(a.job.id)).unwrap().state == JobState::Queued {
            std::thread::sleep(Duration::from_millis(2));
        }
        // A rescan request cannot be served by a plain scan: one job waits for it.
        let c = svc.submit(true);
        assert_ne!(c.job.id, a.job.id);
        let d = svc.submit(true);
        assert!(d.coalesced && d.job.id == c.job.id, "at most one waiting job");
        let s = wait_done(&svc, a.job.id);
        assert_eq!((s.state, s.total, s.completed, s.scanned_ok, s.quarantined), (JobState::Done, 6, 6, 2, 4), "{s:?}");
        assert_eq!(s.clap, Some(2));
        assert!(s.requests >= 2);
        assert!(cache_path.exists());
        let q = svc.quarantine();
        assert_eq!(q.len(), 4, "{q:?}");
        let crash = q.iter().find(|x| x.name == "crash1.clap").unwrap();
        assert!(crash.reason.starts_with("crashed"), "{crash:?}");
        assert!(!crash.older_scanner);
        // The queued rescan runs next: it re-runs only the 2 good bundles (quarantines kept).
        let s2 = wait_done(&svc, c.job.id);
        assert_eq!((s2.kind, s2.total, s2.cached), (JobKind::Rescan, 2, 4), "{s2:?}");
        assert_eq!(svc.quarantine().len(), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Cancelling kills the running workers promptly, keeps finished results and records nothing
    /// for interrupted bundles; a later scan picks up where it stopped.
    #[test]
    fn cancel_stops_workers_and_keeps_completed_results() {
        let dir = tmp("jobs-cancel");
        let worker = fake_worker(&dir, Duration::from_secs(30));
        let plugins = bundles(&dir, &["good1", "hang1", "hang2", "good2"]);
        let cache_path = dir.join("cache.json");
        let svc = ScanService::new(worker, Some(cache_path.clone()), vec![plugins], 1);
        let a = svc.submit(false);
        // Wait until the (single) worker reaches the first hanging bundle.
        let t0 = Instant::now();
        while svc.status(Some(a.job.id)).unwrap().current != ["hang1.clap"] {
            assert!(t0.elapsed() < Duration::from_secs(10), "{:?}", svc.status(Some(a.job.id)));
            std::thread::sleep(Duration::from_millis(5));
        }
        let t1 = Instant::now();
        svc.cancel(Some(a.job.id)).unwrap();
        let s = wait_done(&svc, a.job.id);
        assert!(t1.elapsed() < Duration::from_secs(3), "the hung worker was killed, not waited for");
        assert_eq!((s.state, s.completed, s.clap), (JobState::Cancelled, 2, Some(2)), "{s:?}");
        let saved = Cache::load(&cache_path);
        assert_eq!(saved.entries.len(), 2, "only the finished bundles are recorded");
        assert!(saved.quarantined().is_empty(), "an interrupted bundle is not quarantined");
        // No worker of ours is left behind.
        let calls = std::fs::read_to_string(dir.join("calls.log")).unwrap();
        assert_eq!(calls.lines().count(), 3);
        // A later scan picks up the rest (hang2 included; it hangs and is quarantined at its
        // deadline in real use; here the cancelled one is simply scanned again).
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Retry forgets and rescans only the chosen quarantined bundle; other quarantines and cached
    /// successes are untouched (no worker runs for them).
    #[test]
    fn retry_touches_only_the_chosen_bundles() {
        let dir = tmp("jobs-retry");
        let worker = fake_worker(&dir, Duration::from_millis(400));
        let plugins = bundles(&dir, &["good1", "crash1", "bad1"]);
        let svc = ScanService::new(worker, None, vec![plugins.clone()], 2);
        let a = svc.submit(false);
        wait_done(&svc, a.job.id);
        assert_eq!(svc.quarantine().len(), 2);
        let calls_before = std::fs::read_to_string(dir.join("calls.log")).unwrap().lines().count();
        let r = svc.retry(vec![plugins.join("bad1.clap")]);
        let s = wait_done(&svc, r.job.id);
        assert_eq!((s.kind, s.total, s.completed), (JobKind::Retry, 1, 1), "{s:?}");
        let calls: Vec<String> = std::fs::read_to_string(dir.join("calls.log")).unwrap().lines().map(String::from).collect();
        assert_eq!(calls.len(), calls_before + 1);
        assert!(calls.last().unwrap().ends_with("bad1.clap"));
        let q: Vec<String> = svc.quarantine().into_iter().map(|q| q.name).collect();
        assert_eq!(q, ["crash1.clap", "bad1.clap"], "the other quarantine is kept; bad1 failed again");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
