//! Isolated plugin scanning (R07): every CLAP/VST3 bundle is loaded in a separate worker
//! process that prints its descriptors as JSON; the host never maps an unknown library while
//! scanning. A worker that crashes, hangs (timeout) or errors puts the bundle in quarantine with
//! the reason; quarantined bundles are skipped until the user re-enables them or rescans.
//! Results are cached by file fingerprint (path, size, modification time) so a later scan only
//! spawns workers for new or changed bundles.
//!
//! This isolates *scanning only*. Instantiation for playback still loads the bundle in this
//! process (in-process hosting; see E03 for runtime containment).
//!
//! The worker is this program itself (`soundcraft --scan-plugin <path>`, `soundcraft-cli
//! scan-plugin <path>`), or any program given as [`Worker`] (tests use shell scripts to model a
//! crashing, hanging or failing worker).

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Default per-bundle worker timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);
/// Most bytes accepted from one worker (hostile or runaway output).
const MAX_OUTPUT: usize = 4 << 20;

/// What a worker prints for one bundle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkerOutput {
    #[serde(default)]
    pub clap: Vec<soundcraft_clap_host::ClapDescriptor>,
    #[serde(default)]
    pub vst3: Vec<soundcraft_vst3_host::Vst3Descriptor>,
}

/// Why a bundle is quarantined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quarantine {
    /// The worker died with a signal / abnormal exit (plugin crashed while loading).
    Crashed(String),
    /// The worker exceeded the timeout and was killed.
    TimedOut { seconds: u64 },
    /// The worker reported an error (unloadable, no entry point, …).
    Failed(String),
    /// The worker's output was not parseable.
    BadOutput(String),
}

impl std::fmt::Display for Quarantine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Quarantine::Crashed(s) => write!(f, "crashed while loading ({s})"),
            Quarantine::TimedOut { seconds } => write!(f, "did not answer within {seconds} s"),
            Quarantine::Failed(s) => write!(f, "failed: {s}"),
            Quarantine::BadOutput(s) => write!(f, "unreadable scan result: {s}"),
        }
    }
}

/// One bundle's cached scan result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheEntry {
    pub path: PathBuf,
    pub size: u64,
    /// Modification time (seconds since the Unix epoch).
    pub mtime: u64,
    pub result: Result<WorkerOutput, Quarantine>,
    /// Seconds since the epoch when this entry was written.
    pub scanned_at: u64,
}

/// The scan cache (one JSON file).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Cache {
    pub format: u32,
    pub entries: Vec<CacheEntry>,
}

impl Cache {
    pub fn load(path: &Path) -> Cache {
        std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
        std::fs::write(&tmp, serde_json::to_string_pretty(self).map_err(std::io::Error::other)?)?;
        std::fs::rename(&tmp, path)
    }

    /// Bundles currently quarantined, with their reasons.
    pub fn quarantined(&self) -> Vec<(&Path, &Quarantine)> {
        self.entries.iter().filter_map(|e| e.result.as_ref().err().map(|q| (e.path.as_path(), q))).collect()
    }

    /// Forgets `path`'s entry so the next scan tries it again (re-enable after a quarantine).
    pub fn forget(&mut self, path: &Path) -> bool {
        let n = self.entries.len();
        self.entries.retain(|e| e.path != path);
        n != self.entries.len()
    }
}

/// How to run the worker: a program plus the arguments that precede the bundle path.
#[derive(Debug, Clone)]
pub struct Worker {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub timeout: Duration,
}

impl Worker {
    /// This executable as its own scan worker (`<exe> --scan-plugin <path>`).
    pub fn this_program() -> Option<Worker> {
        let program = std::env::current_exe().ok()?;
        Some(Worker { program, args: vec!["--scan-plugin".into()], timeout: DEFAULT_TIMEOUT })
    }
}

/// The result of one isolated scan.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScanReport {
    pub clap: Vec<soundcraft_clap_host::ClapDescriptor>,
    pub vst3: Vec<soundcraft_vst3_host::Vst3Descriptor>,
    /// Bundles scanned by a worker in this run (not served from the cache).
    pub scanned: usize,
    pub cached: usize,
    pub quarantined: Vec<(PathBuf, Quarantine)>,
}

fn fingerprint(path: &Path) -> Option<(u64, u64)> {
    let m = std::fs::metadata(path).ok()?;
    let mtime = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some((dir_size(path, &m), mtime))
}

/// A bundle may be a directory (macOS/Linux `.vst3`, `.clap` bundles): size = sum of its files
/// (bounded walk), else the file size.
fn dir_size(path: &Path, m: &std::fs::Metadata) -> u64 {
    if !m.is_dir() {
        return m.len();
    }
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    let mut visited = 0;
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            visited += 1;
            if visited > 10_000 {
                return total;
            }
            let p = e.path();
            match e.metadata() {
                Ok(m) if m.is_dir() => stack.push(p),
                Ok(m) => total = total.saturating_add(m.len()),
                Err(_) => {}
            }
        }
    }
    total
}

fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Runs the worker for one bundle: spawns it, waits up to the timeout (polling, never blocking
/// a UI thread forever), kills a hung one, and classifies the outcome.
pub fn scan_one(worker: &Worker, bundle: &Path) -> Result<WorkerOutput, Quarantine> {
    let mut child = Command::new(&worker.program)
        .args(&worker.args)
        .arg(bundle)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Quarantine::Failed(format!("cannot start scan worker: {e}")))?;
    // Read stdout on a helper thread so a chatty worker cannot block us on a full pipe. The
    // thread is never joined: a grandchild of a killed worker may keep the pipe open, and that
    // must not hold the scan (the thread ends when the last writer closes).
    let mut stdout = child.stdout.take();
    let (otx, orx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut o) = stdout.take() {
            let _ = o.by_ref().take(MAX_OUTPUT as u64).read_to_end(&mut buf);
        }
        let _ = otx.send(buf);
    });
    let mut stderr = child.stderr.take();
    let (etx, erx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut err = String::new();
        if let Some(mut e) = stderr.take() {
            let _ = e.by_ref().take(4096).read_to_string(&mut err);
        }
        let _ = etx.send(err);
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) if start.elapsed() >= worker.timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(Quarantine::Failed(format!("waiting for the scan worker: {e}"))),
        }
    };
    let Some(status) = status else {
        return Err(Quarantine::TimedOut { seconds: worker.timeout.as_secs() });
    };
    // The process ended: its own output is complete; wait briefly for the readers, no longer.
    let out = orx.recv_timeout(Duration::from_secs(2)).unwrap_or_default();
    let err = erx.recv_timeout(Duration::from_millis(500)).unwrap_or_default();
    match status.code() {
        Some(0) => serde_json::from_slice::<WorkerOutput>(&out).map_err(|e| Quarantine::BadOutput(e.to_string())),
        Some(code) => Err(Quarantine::Failed(format!("exit code {code}: {}", err.trim()))),
        // No exit code: terminated by a signal (SIGSEGV, SIGABRT, …).
        None => Err(Quarantine::Crashed(format!("{status}"))),
    }
}

/// Scans every bundle under `dirs` through the worker, using and updating `cache` (written to
/// `cache_path` at the end when given). `force` ignores cache hits (rescan) but keeps
/// quarantine entries unless `clear_quarantine`.
pub fn scan_dirs(worker: &Worker, dirs: &[PathBuf], cache: &mut Cache, cache_path: Option<&Path>, force: bool) -> ScanReport {
    let mut bundles: Vec<PathBuf> = soundcraft_clap_host::scan::find_bundles(dirs);
    bundles.extend(soundcraft_vst3_host::scan::find_bundles(dirs));
    bundles.sort();
    bundles.dedup();
    let mut report = ScanReport::default();
    for b in bundles {
        let Some((size, mtime)) = fingerprint(&b) else { continue };
        let hit = cache.entries.iter().position(|e| e.path == b && e.size == size && e.mtime == mtime);
        let entry = match hit {
            Some(i) if !force || cache.entries[i].result.is_err() => {
                report.cached += 1;
                cache.entries[i].clone()
            }
            _ => {
                let result = scan_one(worker, &b);
                report.scanned += 1;
                let e = CacheEntry { path: b.clone(), size, mtime, result, scanned_at: now_unix() };
                cache.entries.retain(|x| x.path != b);
                cache.entries.push(e.clone());
                e
            }
        };
        match entry.result {
            Ok(out) => {
                for d in out.clap {
                    if !report.clap.iter().any(|x| x.id == d.id) {
                        report.clap.push(d);
                    }
                }
                for d in out.vst3 {
                    if !report.vst3.iter().any(|x| x.id == d.id) {
                        report.vst3.push(d);
                    }
                }
            }
            Err(q) => report.quarantined.push((entry.path, q)),
        }
    }
    report.clap.sort_by_key(|d| d.name.to_lowercase());
    report.vst3.sort_by_key(|d| d.name.to_lowercase());
    if let Some(p) = cache_path
        && let Err(e) = cache.save(p)
    {
        log::warn!("plugin scan cache {}: {e}", p.display());
    }
    report
}

/// Hands a report's descriptors to the host registries (menus read them from there).
pub fn adopt(report: &ScanReport) {
    soundcraft_clap_host::adopt_scanned(report.clap.clone());
    soundcraft_vst3_host::adopt_scanned(report.vst3.clone());
}

/// The default search directories of both hosts.
pub fn default_dirs() -> Vec<PathBuf> {
    let mut v = soundcraft_clap_host::scan::default_search_paths();
    v.extend(soundcraft_vst3_host::scan::default_search_paths());
    v
}

/// Worker side: loads one bundle in *this* process and prints its descriptors as JSON. Returns
/// the process exit code. Any crash inside the plugin takes only this process down.
pub fn worker_main(bundle: &Path) -> i32 {
    let ext = bundle.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    let result: Result<WorkerOutput, String> = match ext.as_str() {
        "clap" => soundcraft_clap_host::load_bundle(bundle).map(|clap| WorkerOutput { clap, vst3: Vec::new() }).map_err(|e| e.to_string()),
        "vst3" => soundcraft_vst3_host::load_bundle(bundle).map(|vst3| WorkerOutput { clap: Vec::new(), vst3 }).map_err(|e| e.to_string()),
        _ => Err(format!("not a plugin bundle: {}", bundle.display())),
    };
    match result {
        Ok(out) => match serde_json::to_string(&out) {
            Ok(s) => {
                println!("{s}");
                0
            }
            Err(e) => {
                eprintln!("{e}");
                2
            }
        },
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sml-scan-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A shell "worker" whose behaviour depends on the bundle name: `good*` prints one CLAP
    /// descriptor, `crash*` dies with SIGSEGV, `hang*` sleeps past the timeout, `bad*` exits 3,
    /// `junk*` prints garbage. Every run appends to `calls.log`.
    fn fake_worker(dir: &Path, timeout: Duration) -> Worker {
        let script = dir.join("worker.sh");
        let log = dir.join("calls.log");
        std::fs::write(
            &script,
            format!(
                r#"#!/bin/sh
b="$1"
echo "$b" >> "{log}"
case "$(basename "$b")" in
  good*) printf '{{"clap":[{{"id":"clap:test.fake.%s","plugin_id":"test.fake","name":"Fake %s","vendor":"T","version":"1","description":"","features":["audio-effect"],"category":"Other","is_instrument":false,"path":"%s"}}],"vst3":[]}}\n' "$(basename "$b")" "$(basename "$b")" "$b"; exit 0;;
  crash*) kill -SEGV $$;;
  hang*) sleep 30;;
  bad*) echo "no entry point" >&2; exit 3;;
  junk*) echo "not json"; exit 0;;
esac
"#,
                log = log.display()
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        Worker { program: script, args: Vec::new(), timeout }
    }

    fn bundles(dir: &Path, names: &[&str]) -> PathBuf {
        let plugins = dir.join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        for n in names {
            std::fs::write(plugins.join(format!("{n}.clap")), b"x").unwrap();
        }
        plugins
    }

    /// R07: a crashing, hanging, failing or garbage worker never takes the host down; each such
    /// bundle is quarantined with its reason, good bundles are listed, the cache avoids
    /// re-spawning workers, and re-enabling a quarantined bundle retries it.
    #[test]
    fn isolated_scan_contains_faults_and_caches() {
        let dir = tmp("faults");
        let worker = fake_worker(&dir, Duration::from_millis(400));
        let plugins = bundles(&dir, &["good1", "crash1", "hang1", "bad1", "junk1"]);
        let cache_path = dir.join("cache.json");
        let mut cache = Cache::default();
        let t0 = Instant::now();
        let r = scan_dirs(&worker, std::slice::from_ref(&plugins), &mut cache, Some(&cache_path), false);
        assert!(t0.elapsed() < Duration::from_secs(5), "the hung worker was killed at the timeout");
        assert_eq!(r.scanned, 5);
        assert_eq!(r.clap.len(), 1, "quarantined: {:?}", r.quarantined);
        assert_eq!(r.clap[0].id, "clap:test.fake.good1.clap");
        let q: Vec<String> = r.quarantined.iter().map(|(p, q)| format!("{} {q}", p.file_name().unwrap().to_str().unwrap())).collect();
        assert_eq!(q.len(), 4, "{q:?}");
        assert!(q.iter().any(|s| s.starts_with("crash1.clap crashed")), "{q:?}");
        assert!(q.iter().any(|s| s.starts_with("hang1.clap did not answer")), "{q:?}");
        assert!(q.iter().any(|s| s.starts_with("bad1.clap failed: exit code 3: no entry point")), "{q:?}");
        assert!(q.iter().any(|s| s.starts_with("junk1.clap unreadable")), "{q:?}");
        assert!(cache_path.exists());
        // Second scan: everything (results and quarantines) comes from the cache; no worker runs.
        let calls_before = std::fs::read_to_string(dir.join("calls.log")).unwrap().lines().count();
        let mut cache2 = Cache::load(&cache_path);
        let r2 = scan_dirs(&worker, std::slice::from_ref(&plugins), &mut cache2, Some(&cache_path), false);
        assert_eq!((r2.scanned, r2.cached), (0, 5));
        assert_eq!(r2.clap, r.clap);
        assert_eq!(r2.quarantined.len(), 4);
        assert_eq!(std::fs::read_to_string(dir.join("calls.log")).unwrap().lines().count(), calls_before, "no worker spawned");
        // A changed bundle is rescanned; a re-enabled quarantined one is retried.
        std::fs::write(plugins.join("good1.clap"), b"xyz").unwrap();
        assert!(cache2.forget(&plugins.join("bad1.clap")));
        let r3 = scan_dirs(&worker, std::slice::from_ref(&plugins), &mut cache2, Some(&cache_path), false);
        assert_eq!(r3.scanned, 2, "changed + re-enabled");
        assert_eq!(cache2.quarantined().len(), 4);
        // Rescan (force) retries good bundles but keeps quarantines unless forgotten.
        let r4 = scan_dirs(&worker, &[plugins], &mut cache2, None, true);
        assert_eq!(r4.scanned, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn worker_main_rejects_non_bundles() {
        assert_eq!(worker_main(Path::new("/nonexistent/thing.txt")), 1);
    }
}
