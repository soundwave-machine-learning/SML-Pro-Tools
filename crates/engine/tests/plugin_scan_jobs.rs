//! HF12: plugin scans requested through commands (the control channel's path) are isolated jobs
//! that return at once. The legacy `engine.vst3_plugins {rescan: true}` used to load every bundle
//! inside the host on the UI thread (owner report: control timeout, then the app exited). With the
//! scan service installed it returns a job instead, and nothing is loaded in this process.
//! One test in this binary: the service is process-wide.
#![cfg(unix)]

use serde_json::{Value, json};
use soundcraft_engine::Engine;
use soundcraft_mix::scan_isolated::Worker;
use soundcraft_mix::scan_jobs::{ScanService, install};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn fake_worker(dir: &Path) -> Worker {
    let script = dir.join("worker.sh");
    std::fs::write(
        &script,
        format!(
            r#"#!/bin/sh
b="$1"
echo "$b" >> "{log}"
case "$(basename "$b")" in
  good*) printf '{{"clap":[{{"id":"clap:t.%s","plugin_id":"t","name":"Fake %s","vendor":"T","version":"1","description":"","features":["audio-effect"],"category":"Other","is_instrument":false,"path":"%s"}}],"vst3":[]}}\n' "$(basename "$b")" "$(basename "$b")" "$b"; exit 0;;
  crash*) echo "SML-SCAN-STAGE clap entry_init" >&2; kill -SEGV $$;;
  slow*) sleep 1; exit 3;;
esac
"#,
            log = dir.join("calls.log").display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    Worker { program: script, args: Vec::new(), timeout: Duration::from_secs(5) }
}

fn wait(e: &mut Engine, id: u64) -> Value {
    let t0 = Instant::now();
    loop {
        let s = e.execute("engine.plugin_scan_status", &json!({"job": id})).unwrap();
        if matches!(s["job"]["state"].as_str(), Some("done" | "cancelled")) {
            return s["job"].clone();
        }
        assert!(t0.elapsed() < Duration::from_secs(30), "{s}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn control_path_scans_are_isolated_jobs_that_return_at_once() {
    let dir = std::env::temp_dir().join(format!("sml-scan-cmd-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let plugins = dir.join("plugins");
    std::fs::create_dir_all(&plugins).unwrap();
    for n in ["good1", "good2", "crash1", "slow1", "slow2"] {
        std::fs::write(plugins.join(format!("{n}.clap")), b"x").unwrap();
    }
    let mut e = Engine::default();
    // Without a service the new commands say so (the CLI and tools).
    let err = e.execute("engine.plugin_scan_start", &json!({})).unwrap_err().to_string();
    assert!(err.contains("not running in this process"), "{err}");

    install(ScanService::new(fake_worker(&dir), Some(dir.join("cache.json")), vec![plugins.clone()], 2));
    // The legacy rescan (the owner's control request) answers at once with a job.
    let t0 = Instant::now();
    let r = e.execute("engine.vst3_plugins", &json!({"rescan": true})).unwrap();
    assert!(t0.elapsed() < Duration::from_millis(500), "returned in {:?}", t0.elapsed());
    let id = r["job"]["id"].as_u64().expect("a job");
    // The overlapping CLAP rescan (the owner's second request) joins it.
    let r2 = e.execute("engine.clap_plugins", &json!({"rescan": true})).unwrap();
    assert_eq!(r2["job"]["id"].as_u64(), Some(id), "{r2}");
    assert_eq!(r2["coalesced"], json!(true));
    let job = wait(&mut e, id);
    assert_eq!(job["state"], "done");
    assert_eq!((job["total"].as_u64(), job["scanned_ok"].as_u64(), job["quarantined"].as_u64()), (Some(5), Some(2), Some(3)), "{job}");
    // The listing comes from the isolated results; nothing was loaded in this process.
    let list = e.execute("engine.clap_plugins", &json!({})).unwrap();
    assert_eq!(list.as_array().map(Vec::len), Some(2), "{list}");
    let q = e.execute("engine.plugin_scan_quarantine", &json!({})).unwrap();
    let crash = q.as_array().unwrap().iter().find(|x| x["name"] == "crash1.clap").unwrap().clone();
    assert_eq!(crash["diagnostics"]["last_stage"], "clap entry_init", "{crash}");
    assert!(crash["reason"].as_str().unwrap().starts_with("crashed"), "{crash}");
    // Retry needs an explicit choice, and touches only that bundle.
    assert!(e.execute("engine.plugin_scan_retry", &json!({})).is_err());
    let before = std::fs::read_to_string(dir.join("calls.log")).unwrap().lines().count();
    let r = e.execute("engine.plugin_scan_retry", &json!({"paths": [plugins.join("crash1.clap")]})).unwrap();
    wait(&mut e, r["job"]["id"].as_u64().unwrap());
    let calls: Vec<PathBuf> = std::fs::read_to_string(dir.join("calls.log")).unwrap().lines().map(PathBuf::from).collect();
    assert_eq!(calls.len(), before + 1);
    // A started job can be cancelled; status reports it.
    let r = e.execute("engine.plugin_scan_start", &json!({"rescan": true})).unwrap();
    let id = r["job"]["id"].as_u64().unwrap();
    let c = e.execute("engine.plugin_scan_cancel", &json!({"job": id})).unwrap();
    assert_eq!(c["job"]["id"].as_u64(), Some(id));
    let job = wait(&mut e, id);
    assert_eq!(job["state"], "cancelled", "{job}");
    let _ = std::fs::remove_dir_all(&dir);
}
