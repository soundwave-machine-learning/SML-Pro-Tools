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
//! Diagnostics (HF13): the worker writes the step it is about to run to standard error
//! ([`soundcraft_vst3_host::STAGE_PREFIX`] lines), so when it dies the host records the last step
//! reached, the decoded exit status (Windows NTSTATUS codes such as `0xC0000005` are crashes, not
//! errors), the binary's architecture and whether the plugin list was already complete. The
//! worker runs the module exit itself before it ends: on Windows `std::process::exit` skips the C
//! exit handlers that would otherwise run it, so the plugin's DLL detach used to run without its
//! `ExitDll`.
//!
//! The worker is this program itself (`soundcraft --scan-plugin <path>`, `soundcraft-cli
//! scan-plugin <path>`), or any program given as [`Worker`] (tests use shell scripts to model a
//! crashing, hanging or failing worker).

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Revision of the scanner recorded in cache entries: quarantines written by an older revision
/// (0 = before HF13) are listed as such so the user can retry them with the current scanner.
pub const SCANNER_REVISION: u32 = 2;
/// Most bytes of a worker's standard error kept (the tail; the reader never stops reading, so a
/// chatty worker cannot block on a full pipe).
const MAX_STDERR: usize = 16 << 10;

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
    /// The binary cannot load in this build (architecture or missing dependency); not a crash.
    Unsupported(String),
    /// The worker reported the complete plugin list, then crashed while unloading the binary
    /// (module exit or DLL teardown). Kept quarantined: hosting runs the same exit.
    UnloadCrashed(String),
}

impl std::fmt::Display for Quarantine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Quarantine::Crashed(s) => write!(f, "crashed while loading ({s})"),
            Quarantine::TimedOut { seconds } => write!(f, "did not answer within {seconds} s"),
            Quarantine::Failed(s) => write!(f, "failed: {s}"),
            Quarantine::BadOutput(s) => write!(f, "unreadable scan result: {s}"),
            Quarantine::Unsupported(s) => write!(f, "cannot load in this build: {s}"),
            Quarantine::UnloadCrashed(s) => write!(f, "listed its plugins, then crashed while unloading ({s})"),
        }
    }
}

/// What the host learned about one worker run (HF13). Paths stay local (the cache file).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScanDiagnostics {
    /// The last step the worker announced (`vst3 load_library`, `vst3 class_info 2/5`, …).
    #[serde(default)]
    pub last_stage: Option<String>,
    /// The raw exit code (Windows: an NTSTATUS for crashes).
    #[serde(default)]
    pub exit_code: Option<i64>,
    /// What the exit code means (`access violation (0xC0000005)`), when known.
    #[serde(default)]
    pub exit_meaning: Option<String>,
    /// The worker printed a complete plugin list before it ended.
    #[serde(default)]
    pub output_complete: bool,
    /// Plugins in that list.
    #[serde(default)]
    pub classes_reported: usize,
    /// The binary's machine type read from its header (`x86_64`, `x86`, `arm64`, `universal(…)`).
    #[serde(default)]
    pub architecture: Option<String>,
    #[serde(default)]
    pub worker_pid: Option<u32>,
    #[serde(default)]
    pub elapsed_ms: u64,
    /// [`SCANNER_REVISION`] that produced the entry (0: an older scanner).
    #[serde(default)]
    pub scanner: u32,
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
    /// How the worker ended (absent in caches written before HF13).
    #[serde(default)]
    pub diagnostics: Option<ScanDiagnostics>,
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

pub(crate) fn fingerprint(path: &Path) -> Option<(u64, u64)> {
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
    scan_bundle(worker, bundle, None).map_or(Err(Quarantine::Failed("cancelled".into())), |(r, _)| r)
}

/// The machine type of a plugin binary from its header (PE, ELF or Mach-O), `None` when unknown.
pub fn binary_architecture(binary: &Path) -> Option<String> {
    let mut f = std::fs::File::open(binary).ok()?;
    let mut buf = Vec::new();
    f.by_ref().take(64 << 10).read_to_end(&mut buf).ok()?;
    let u16le = |o: usize| buf.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32le = |o: usize| buf.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let u32be = |o: usize| buf.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let mach = |cpu: u32| match cpu {
        0x0100_0007 => "x86_64",
        0x0100_000C => "arm64",
        7 => "x86",
        _ => "other",
    };
    if buf.starts_with(b"MZ") {
        let pe = usize::try_from(u32le(0x3C)?).ok()?;
        if buf.get(pe..pe + 4)? != b"PE\0\0" {
            return None;
        }
        return Some(
            match u16le(pe + 4)? {
                0x8664 => "x86_64",
                0x014C => "x86",
                0xAA64 => "arm64",
                0xA641 => "arm64ec",
                0x01C4 => "arm",
                _ => "other",
            }
            .to_string(),
        );
    }
    if buf.starts_with(b"\x7FELF") {
        let le = buf.get(5) == Some(&1);
        let m = buf.get(18..20).map(|b| if le { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) })?;
        return Some(
            match m {
                62 => "x86_64",
                3 => "x86",
                183 => "arm64",
                40 => "arm",
                243 => "riscv",
                _ => "other",
            }
            .to_string(),
        );
    }
    match u32be(0)? {
        0xCFFA_EDFE => Some(mach(u32le(4)?).to_string()),
        0xCAFE_BABE => {
            let n = u32be(4)?.min(8) as usize;
            let arches: Vec<&str> = (0..n).filter_map(|i| u32be(8 + i * 20).map(mach)).collect();
            Some(format!("universal({})", arches.join(",")))
        }
        _ => None,
    }
}

/// Whether a binary of `arch` can load in this build (`None`/`other`: not known, so it is tried).
fn arch_compatible(arch: &str) -> bool {
    let host = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        a => a,
    };
    match arch {
        "other" => true,
        a if a.starts_with("universal(") => a.contains(host),
        // An ARM64X/EC binary is ARM code; x64 hosts cannot load it.
        a => a == host,
    }
}

/// What a Windows exit code (NTSTATUS) means, and whether it is a missing dependency or
/// architecture problem (`true`) rather than a crash. `None` for ordinary exit codes.
pub fn ntstatus_meaning(code: i64) -> Option<(&'static str, bool)> {
    let c = u32::try_from(code & 0xFFFF_FFFF).ok()?;
    Some(match c {
        0xC000_0005 => ("access violation (0xC0000005)", false),
        0xC000_0409 => ("stack buffer overrun / fail-fast (0xC0000409)", false),
        0xC000_00FD => ("stack overflow (0xC00000FD)", false),
        0xC000_0374 => ("heap corruption (0xC0000374)", false),
        0xC000_001D => ("illegal instruction (0xC000001D)", false),
        0xC000_0094 => ("integer division by zero (0xC0000094)", false),
        0x8000_0003 => ("breakpoint (0x80000003)", false),
        0xE06D_7363 => ("unhandled C++ exception (0xE06D7363)", false),
        0xC000_0420 => ("assertion failure (0xC0000420)", false),
        0x4000_0015 => ("fatal application exit (0x40000015)", false),
        0xC000_0142 => ("a DLL failed to initialise (0xC0000142)", false),
        0xC000_0135 => ("a required DLL was not found (0xC0000135)", true),
        0xC000_0139 => ("a required DLL entry point was not found (0xC0000139)", true),
        0xC000_007B => ("not a valid image for this architecture (0xC000007B)", true),
        _ if c >= 0xC000_0000 => ("unhandled exception", false),
        _ => return None,
    })
}

/// Splits a worker's standard error into the last stage it announced and the other lines.
fn split_stderr(err: &str) -> (Option<String>, String) {
    let mut stage = None;
    let mut rest = Vec::new();
    for l in err.lines() {
        match l.strip_prefix(soundcraft_vst3_host::STAGE_PREFIX) {
            Some(s) => stage = Some(s.trim().to_string()),
            None if !l.trim().is_empty() => rest.push(l.trim()),
            None => {}
        }
    }
    let msg = rest.join("; ");
    (stage, msg.chars().take(600).collect())
}

/// Stages after the plugin list was written: a crash there is a crash while unloading.
fn is_unload_stage(stage: &str) -> bool {
    ["output_written", "release_factory", "module_exit", "entry_deinit", "worker_exit"].iter().any(|s| stage.ends_with(s))
}

/// Classifies a finished worker run (pure, so every platform's codes are testable anywhere).
/// `code` is the exit code (`None`: ended by a signal, described by `signal`).
pub fn classify(code: Option<i64>, signal: &str, out: &[u8], err: &str, diag: &mut ScanDiagnostics) -> Result<WorkerOutput, Quarantine> {
    let (stage, msg) = split_stderr(err);
    diag.last_stage = stage.clone();
    diag.exit_code = code;
    let parsed = serde_json::from_slice::<WorkerOutput>(out);
    if let Ok(o) = &parsed {
        diag.output_complete = true;
        diag.classes_reported = o.clap.len() + o.vst3.len();
    }
    let at = stage.as_deref().map_or(String::new(), |s| format!(" during `{s}`"));
    let crashed = |what: String, diag: &ScanDiagnostics| {
        if diag.output_complete && stage.as_deref().is_some_and(is_unload_stage) {
            Quarantine::UnloadCrashed(format!("{what}{at}; {} plugin(s) were listed", diag.classes_reported))
        } else {
            Quarantine::Crashed(format!("{what}{at}"))
        }
    };
    match code {
        Some(0) => parsed.map_err(|e| Quarantine::BadOutput(e.to_string())),
        Some(c) => match ntstatus_meaning(c) {
            Some((meaning, dependency)) => {
                diag.exit_meaning = Some(meaning.to_string());
                if dependency { Err(Quarantine::Unsupported(format!("{meaning}{at}"))) } else { Err(crashed(meaning.to_string(), diag)) }
            }
            None => Err(Quarantine::Failed(format!("exit code {c}: {msg}"))),
        },
        None => {
            diag.exit_meaning = Some(signal.to_string());
            Err(crashed(signal.to_string(), diag))
        }
    }
}

/// [`scan_one`] with diagnostics and cancellation: `None` when `cancel` was raised (the worker is
/// killed and nothing is recorded for the bundle).
pub fn scan_bundle(worker: &Worker, bundle: &Path, cancel: Option<&AtomicBool>) -> Option<(Result<WorkerOutput, Quarantine>, ScanDiagnostics)> {
    let start = Instant::now();
    let mut diag = ScanDiagnostics { scanner: SCANNER_REVISION, ..Default::default() };
    let binary = soundcraft_vst3_host::scan::bundle_binary(bundle).or_else(|| soundcraft_clap_host::scan::bundle_executable(bundle));
    diag.architecture = binary.as_deref().and_then(binary_architecture);
    if let Some(a) = diag.architecture.as_deref().filter(|a| !arch_compatible(a)) {
        let host = std::env::consts::ARCH;
        return Some((Err(Quarantine::Unsupported(format!("the binary is built for {a}; this build hosts {host} plugins only"))), diag));
    }
    let mut child =
        match Command::new(&worker.program).args(&worker.args).arg(bundle).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
        {
            Ok(c) => c,
            Err(e) => return Some((Err(Quarantine::Failed(format!("cannot start scan worker: {e}"))), diag)),
        };
    diag.worker_pid = Some(child.id());
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
    // Standard error is read to the end (keeping the tail), so stage lines never fill the pipe.
    let mut stderr = child.stderr.take();
    let (etx, erx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut tail: Vec<u8> = Vec::new();
        if let Some(mut e) = stderr.take() {
            let mut chunk = [0u8; 4096];
            while let Ok(n) = e.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                tail.extend_from_slice(chunk.get(..n).unwrap_or_default());
                if tail.len() > MAX_STDERR {
                    let cut = tail.len() - MAX_STDERR;
                    tail.drain(..cut);
                }
            }
        }
        let _ = etx.send(String::from_utf8_lossy(&tail).into_owned());
    });
    let status = loop {
        if cancel.is_some_and(|c| c.load(Ordering::Acquire)) {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) if start.elapsed() >= worker.timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Some((Err(Quarantine::Failed(format!("waiting for the scan worker: {e}"))), diag)),
        }
    };
    let Some(status) = status else {
        diag.elapsed_ms = start.elapsed().as_millis() as u64;
        diag.last_stage = split_stderr(&erx.recv_timeout(Duration::from_millis(500)).unwrap_or_default()).0;
        let at = diag.last_stage.as_deref().map_or(String::new(), |s| format!(" (last step `{s}`)"));
        log::warn!("plugin scan: {} timed out{at}", bundle.display());
        return Some((Err(Quarantine::TimedOut { seconds: worker.timeout.as_secs() }), diag));
    };
    // The process ended: its own output is complete; wait briefly for the readers, no longer.
    let out = orx.recv_timeout(Duration::from_secs(2)).unwrap_or_default();
    let err = erx.recv_timeout(Duration::from_millis(500)).unwrap_or_default();
    diag.elapsed_ms = start.elapsed().as_millis() as u64;
    let result = classify(status.code().map(i64::from), &format!("{status}"), &out, &err, &mut diag);
    if let Err(q) = &result {
        log::warn!("plugin scan: {} quarantined: {q} [worker {:?}, {} ms]", bundle.display(), diag.worker_pid, diag.elapsed_ms);
    }
    Some((result, diag))
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
                let (result, diag) =
                    scan_bundle(worker, &b, None).unwrap_or_else(|| (Err(Quarantine::Failed("cancelled".into())), ScanDiagnostics::default()));
                report.scanned += 1;
                let e = CacheEntry { path: b.clone(), size, mtime, result, scanned_at: now_unix(), diagnostics: Some(diag) };
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
/// the process exit code. Any crash inside the plugin takes only this process down. Each step is
/// announced on standard error first (HF13), and the binary is unloaded here, module exit
/// included, before the caller exits the process.
pub fn worker_main(bundle: &Path) -> i32 {
    soundcraft_vst3_host::set_stage_trace(true);
    soundcraft_clap_host::set_stage_trace(true);
    let ext = bundle.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    let result: Result<WorkerOutput, String> = match ext.as_str() {
        "clap" => soundcraft_clap_host::load_bundle(bundle).map(|clap| WorkerOutput { clap, vst3: Vec::new() }).map_err(|e| e.to_string()),
        "vst3" => soundcraft_vst3_host::load_bundle(bundle).map(|vst3| WorkerOutput { clap: Vec::new(), vst3 }).map_err(|e| e.to_string()),
        _ => Err(format!("not a plugin bundle: {}", bundle.display())),
    };
    let code = match result {
        Ok(out) => match serde_json::to_string(&out) {
            Ok(s) => {
                use std::io::Write;
                let mut o = std::io::stdout().lock();
                let _ = writeln!(o, "{s}");
                let _ = o.flush();
                soundcraft_vst3_host::trace_stage("output_written");
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
    };
    // `std::process::exit` on Windows ends the process without the C exit handlers, so the
    // module exit (`ExitDll`) would never run and the plugin's DLL teardown would run without it.
    // Run it here, on the thread that ran the entry, like the app does at quit.
    soundcraft_vst3_host::shutdown();
    soundcraft_clap_host::release_unused();
    soundcraft_vst3_host::trace_stage("worker_exit");
    code
}

#[cfg(test)]
#[cfg(unix)]
pub(crate) mod tests_support {
    use super::*;

    pub(crate) fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sml-scan-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A shell "worker" whose behaviour depends on the bundle name: `good*` prints one CLAP
    /// descriptor, `crash*` dies with SIGSEGV, `hang*` sleeps past the timeout, `bad*` exits 3,
    /// `junk*` prints garbage. Every run appends to `calls.log`.
    pub(crate) fn fake_worker(dir: &Path, timeout: Duration) -> Worker {
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

    pub(crate) fn bundles(dir: &Path, names: &[&str]) -> PathBuf {
        let plugins = dir.join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        for n in names {
            std::fs::write(plugins.join(format!("{n}.clap")), b"x").unwrap();
        }
        plugins
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    use super::tests_support::*;

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

#[cfg(test)]
mod classify_tests {
    use super::*;

    const LIST: &[u8] = br#"{"clap":[],"vst3":[{"id":"vst3:00","class_id":"00","name":"P","vendor":"V","version":"1","sdk_version":"VST 3.7","sub_categories":["Fx"],"category":"Other","is_instrument":false,"path":"/x.vst3"}]}"#;

    fn run(code: Option<i64>, out: &[u8], err: &str) -> (Result<WorkerOutput, Quarantine>, ScanDiagnostics) {
        let mut d = ScanDiagnostics::default();
        let r = classify(code, "signal: 11 (SIGSEGV)", out, err, &mut d);
        (r, d)
    }

    /// HF13: the owner's cache recorded `exit code -1073741819` as an ordinary failure. It is an
    /// access violation (0xC0000005): a crash, with the step that was running.
    #[test]
    fn windows_access_violation_is_a_crash_with_its_stage() {
        let err = "SML-SCAN-STAGE vst3 locate_binary\nSML-SCAN-STAGE vst3 load_library\nSML-SCAN-STAGE vst3 module_entry InitDll\n";
        let (r, d) = run(Some(-1_073_741_819), b"", err);
        assert_eq!(r, Err(Quarantine::Crashed("access violation (0xC0000005) during `vst3 module_entry InitDll`".into())));
        assert_eq!(d.last_stage.as_deref(), Some("vst3 module_entry InitDll"));
        assert_eq!(d.exit_meaning.as_deref(), Some("access violation (0xC0000005)"));
        assert!(!d.output_complete);
    }

    /// A crash after the complete list was printed is a crash while unloading: reported as such
    /// (with the plugin count), and still quarantined.
    #[test]
    fn crash_after_the_list_is_an_unload_crash() {
        let err = "SML-SCAN-STAGE vst3 class_info 0/1\nSML-SCAN-STAGE vst3 output_written\nSML-SCAN-STAGE vst3 release_factory\nSML-SCAN-STAGE vst3 module_exit\n";
        let (r, d) = run(Some(0xC000_0005), LIST, err);
        match r {
            Err(Quarantine::UnloadCrashed(m)) => assert!(m.contains("during `vst3 module_exit`") && m.contains("1 plugin(s)"), "{m}"),
            other => panic!("{other:?}"),
        }
        assert!(d.output_complete);
        assert_eq!(d.classes_reported, 1);
    }

    /// Missing dependencies and wrong architecture are not crashes.
    #[test]
    fn dependency_and_architecture_codes_are_unsupported() {
        let (r, _) = run(Some(0xC000_0135), b"", "SML-SCAN-STAGE vst3 load_library\n");
        assert!(matches!(r, Err(Quarantine::Unsupported(ref m)) if m.contains("required DLL was not found")), "{r:?}");
        let (r, _) = run(Some(0xC000_007B), b"", "");
        assert!(matches!(r, Err(Quarantine::Unsupported(_))), "{r:?}");
    }

    #[test]
    fn ordinary_failures_and_signals() {
        let (r, _) = run(Some(1), b"", "SML-SCAN-STAGE vst3 get_factory\ncannot load VST3 plugin x: not a VST3 plugin\n");
        assert_eq!(r, Err(Quarantine::Failed("exit code 1: cannot load VST3 plugin x: not a VST3 plugin".into())));
        let (r, d) = run(None, b"", "SML-SCAN-STAGE clap entry_init\n");
        assert_eq!(r, Err(Quarantine::Crashed("signal: 11 (SIGSEGV) during `clap entry_init`".into())));
        assert_eq!(d.last_stage.as_deref(), Some("clap entry_init"));
        let (r, d) = run(Some(0), LIST, "SML-SCAN-STAGE vst3 worker_exit\n");
        assert_eq!(r.map(|o| o.vst3.len()), Ok(1));
        assert!(d.output_complete);
    }

    /// Old cache files (before HF13) still load; their entries have no diagnostics.
    #[test]
    fn caches_written_before_diagnostics_still_load() {
        let old = r#"{"format":0,"entries":[{"path":"C:/x.vst3","size":1,"mtime":2,"result":{"Err":{"Failed":"exit code -1073741819: "}},"scanned_at":3}]}"#;
        let c: Cache = serde_json::from_str(old).unwrap();
        assert_eq!(c.entries[0].diagnostics, None);
        assert_eq!(c.quarantined().len(), 1);
    }

    /// Architecture is read from PE, ELF and Mach-O headers; unknown files are tried, not refused.
    #[test]
    fn binary_architecture_from_headers() {
        let dir = std::env::temp_dir().join(format!("sml-arch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut pe = vec![0u8; 0x100];
        pe[0] = b'M';
        pe[1] = b'Z';
        pe[0x3C] = 0x80;
        pe[0x80..0x84].copy_from_slice(b"PE\0\0");
        for (machine, want) in [(0x8664u16, "x86_64"), (0x014C, "x86"), (0xAA64, "arm64")] {
            pe[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
            let p = dir.join(format!("{want}.vst3"));
            std::fs::write(&p, &pe).unwrap();
            assert_eq!(binary_architecture(&p).as_deref(), Some(want));
        }
        let mut elf = vec![0u8; 64];
        elf[..4].copy_from_slice(b"\x7FELF");
        elf[5] = 1;
        elf[18] = 62;
        std::fs::write(dir.join("e.so"), &elf).unwrap();
        assert_eq!(binary_architecture(&dir.join("e.so")).as_deref(), Some("x86_64"));
        std::fs::write(dir.join("t.txt"), b"hello").unwrap();
        assert_eq!(binary_architecture(&dir.join("t.txt")), None);
        assert!(arch_compatible("other"));
        let host = if std::env::consts::ARCH == "aarch64" { "arm64" } else { std::env::consts::ARCH };
        assert!(arch_compatible(host));
        assert!(arch_compatible(&format!("universal(x86,{host})")));
        assert!(!arch_compatible(if host == "x86" { "x86_64" } else { "x86" }));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
