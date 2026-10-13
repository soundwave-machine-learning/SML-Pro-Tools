//! Plugin scan jobs (HF12): start, follow, cancel and retry the isolated scan through the one
//! scan service of the process (`soundcraft_mix::scan_jobs`). Every command returns at once; a
//! scan never runs inside the command. The app installs the service at start; a process without
//! one (the CLI, tests) reports that instead of scanning in-process.

use super::*;
use crate::cmd;
use serde_json::json;

const NO_SERVICE: &str = "the isolated plugin scanner is not running in this process (SML Studio starts it; tools without it list plugins with engine.vst3_plugins / engine.clap_plugins)";

#[cfg(not(target_arch = "wasm32"))]
fn service() -> Result<std::sync::Arc<soundcraft_mix::scan_jobs::ScanService>> {
    soundcraft_mix::scan_jobs::installed().ok_or_else(|| EngineError::Unsupported(NO_SERVICE.into()))
}

/// The browser build has no plugin hosting: a stand-in with the same calls that are never made.
#[cfg(target_arch = "wasm32")]
struct NoService;
#[cfg(target_arch = "wasm32")]
impl NoService {
    fn submit(&self, _: bool) -> Value {
        Value::Null
    }
    fn status(&self, _: Option<u64>) -> Value {
        Value::Null
    }
    fn jobs(&self) -> Value {
        Value::Null
    }
    fn cancel(&self, _: Option<u64>) -> Value {
        Value::Null
    }
    fn retry(&self, _: Vec<std::path::PathBuf>) -> Value {
        Value::Null
    }
    fn quarantine(&self) -> Value {
        Value::Null
    }
}
#[cfg(target_arch = "wasm32")]
fn service() -> Result<NoService> {
    Err(EngineError::Unsupported(NO_SERVICE.into()))
}

/// Starts (or joins) a scan job when the service is installed; `None` without one.
#[cfg(not(target_arch = "wasm32"))]
pub fn submit_if_installed(force: bool) -> Option<Value> {
    soundcraft_mix::scan_jobs::installed().map(|s| json!(s.submit(force)))
}

#[cfg(target_arch = "wasm32")]
pub fn submit_if_installed(_force: bool) -> Option<Value> {
    None
}

fn job_param(p: &Value) -> Option<u64> {
    p.get("job").and_then(Value::as_u64)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "engine.plugin_scan_start",
            "Start Plugin Scan",
            [],
            None,
            "{rescan?: bool} → {job: {id, kind, state, total, completed, scanned_ok, quarantined, cached, current, requests, clap, vst3, elapsed_ms}, coalesced}. Returns at once. rescan: scan cached successes again (quarantines are kept). A request the running job already covers joins it; otherwise one job waits and later requests join that one.",
            always,
            |_, p| {
                let force = p.get("rescan").and_then(Value::as_bool).unwrap_or(false);
                let svc = service()?;
                Ok(json!(svc.submit(force)))
            }
        ),
        cmd!(
            query "engine.plugin_scan_status",
            "Plugin Scan Status",
            [],
            None,
            "{job?: int} → {job: {…} | null, jobs: [{…}]} (the newest job when none is given)",
            always,
            |_, p| {
                let svc = service()?;
                Ok(json!({"job": svc.status(job_param(p)), "jobs": svc.jobs()}))
            }
        ),
        cmd!(
            query "engine.plugin_scan_cancel",
            "Cancel Plugin Scan",
            [],
            None,
            "{job?: int} → {job: {…} | null}. Stops starting workers and kills the running ones (only the scanner's own); finished bundles keep their results, interrupted ones are not recorded.",
            always,
            |_, p| {
                let svc = service()?;
                Ok(json!({"job": svc.cancel(job_param(p))}))
            }
        ),
        cmd!(
            query "engine.plugin_scan_retry",
            "Retry Quarantined Plugins",
            [],
            None,
            "{paths: [string]} | {all: true} → {job, coalesced}. Forgets and rescans only those quarantined bundles; other quarantines and cached results stay.",
            always,
            |_, p| {
                let svc = service()?;
                let paths: Vec<std::path::PathBuf> =
                    p.get("paths").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).take(4096).map(std::path::PathBuf::from).collect()).unwrap_or_default();
                let all = p.get("all").and_then(Value::as_bool).unwrap_or(false);
                if paths.is_empty() && !all {
                    return Err(EngineError::BadParams("engine.plugin_scan_retry".into(), "give `paths` (quarantined bundles) or `all: true`".into()));
                }
                Ok(json!(svc.retry(paths)))
            }
        ),
        cmd!(
            query "engine.plugin_scan_quarantine",
            "List Quarantined Plugins",
            [],
            None,
            "{} → [{path, name, reason, diagnostics: {last_stage, exit_code, exit_meaning, output_complete, classes_reported, architecture, worker_pid, elapsed_ms, scanner} | null, older_scanner}]",
            always,
            |_, _| {
                let svc = service()?;
                Ok(json!(svc.quarantine()))
            }
        ),
    ]
}
