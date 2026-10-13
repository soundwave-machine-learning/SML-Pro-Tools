//! HF13: the real scan worker path against the VST3 fixture plugin. This binary is its own
//! worker (`--scan-plugin <bundle>`, ending with `std::process::exit` exactly like the app). The
//! worker announces each step, prints the plugin list and then runs the module exit itself; the
//! fixture aborts the process if its statics are torn down without that exit, which is what
//! happened on Windows, where `std::process::exit` skips the exit handler that used to run it.
//! `harness = false`: the process exit is part of what is tested.
#![cfg(not(target_arch = "wasm32"))]

use soundcraft_mix::scan_isolated::{Worker, scan_bundle};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

fn fixture_bundle() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../vst3-host/tests/fixture/Cargo.toml");
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("vst3-fixture-target");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let st = Command::new(cargo)
        .args(["build", "--quiet", "--manifest-path"])
        .arg(&manifest)
        .arg("--target-dir")
        .arg(&target)
        .env_remove("RUSTFLAGS")
        .status()
        .expect("run cargo");
    assert!(st.success(), "building the VST3 fixture failed");
    let (lib, folder, exe) = if cfg!(target_os = "macos") {
        ("libvst3_fixture.dylib", "MacOS", "Fixture")
    } else if cfg!(windows) {
        ("vst3_fixture.dll", "x86_64-win", "Fixture.vst3")
    } else {
        ("libvst3_fixture.so", if cfg!(target_arch = "aarch64") { "aarch64-linux" } else { "x86_64-linux" }, "Fixture.so")
    };
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("scan-worker-fixture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let bundle = dir.join("Fixture.vst3");
    let bin = bundle.join("Contents").join(folder);
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::copy(target.join("debug").join(lib), bin.join(exe)).unwrap();
    bundle
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--scan-plugin") {
        std::process::exit(soundcraft_mix::scan_isolated::worker_main(Path::new(&args[i + 1])));
    }
    let bundle = fixture_bundle();
    let worker = Worker { program: std::env::current_exe().unwrap(), args: vec!["--scan-plugin".into()], timeout: Duration::from_secs(60) };
    let (result, diag) = scan_bundle(&worker, &bundle, None).expect("not cancelled");
    let out = result.unwrap_or_else(|q| panic!("fixture quarantined: {q} {diag:?}"));
    assert_eq!(out.vst3.len(), 3, "{out:?}");
    assert!(diag.output_complete && diag.classes_reported == 3, "{diag:?}");
    assert_eq!(diag.last_stage.as_deref(), Some("vst3 worker_exit"), "the worker ran the module exit and ended cleanly: {diag:?}");
    assert_eq!(diag.exit_code, Some(0));
    let host = if std::env::consts::ARCH == "aarch64" { "arm64" } else { std::env::consts::ARCH };
    assert_eq!(diag.architecture.as_deref(), Some(host));
    let _ = std::fs::remove_dir_all(bundle.parent().unwrap());
    println!("scan_worker_fixture: ok ({} ms)", diag.elapsed_ms);
}
