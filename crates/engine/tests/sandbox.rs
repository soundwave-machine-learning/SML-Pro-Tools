//! Sandboxed hosting (R31 compatibility mode, E03 follow-up), end to end with the real CLAP
//! fixture. This test binary is also the worker (`--plugin-worker <id> [dirs…]`), so the tests
//! need no other executable. Harness-less: the checks run in sequence from `main`.
//!
//! Writes the measurements to `SML_SANDBOX_CSV` when set.

use serde_json::json;
use soundcraft_audio_io::AudioBuffer;
use soundcraft_engine::Engine;
use soundcraft_mix::sandbox::{self, Fault};
use soundcraft_model::{ChannelFormat, Clip, SourceAudio, SourceId, TrackKind};
use soundcraft_time::Range;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

const GAIN: &str = "clap:org.soundcraft.test.gain";

fn install_fixture() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../clap-host/tests/fixture/Cargo.toml");
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("clap-fixture-target");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let st = Command::new(cargo)
        .args(["build", "--quiet", "--manifest-path"])
        .arg(&manifest)
        .arg("--target-dir")
        .arg(&target)
        .env_remove("RUSTFLAGS")
        .status()
        .unwrap();
    assert!(st.success());
    let lib = if cfg!(target_os = "macos") {
        "libclap_fixture.dylib"
    } else if cfg!(windows) {
        "clap_fixture.dll"
    } else {
        "libclap_fixture.so"
    };
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("engine-sandbox-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(target.join("debug").join(lib), dir.join("Fixture.clap")).unwrap();
    assert_eq!(soundcraft_clap_host::add_search_dir(&dir), 3);
    dir
}

fn dc_session(level: f32, n: usize) -> (Engine, soundcraft_model::TrackId) {
    let mut e = Engine::default();
    let s = e.session_mut();
    let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
    s.pool.insert(SourceId(70), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![vec![level; n]] })));
    let id = s.new_clip_id();
    s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "dc", SourceId(70), 0, 0, n as i64));
    (e, t)
}

struct Measured {
    block: usize,
    blocks: u64,
    late: u64,
    p50_us: f64,
    p99_us: f64,
    max_us: f64,
    rss_kib: u64,
}

/// Output parity: the sandboxed gain equals the in-process gain, delayed by exactly one block
/// (its declared added latency), and PDC removes that delay in an offline render.
fn parity_and_latency(block: usize) -> Measured {
    let n = 48_000;
    let (mut e, t) = dc_session(0.25, n);
    e.execute("mix.insert", &json!({"track": t.0, "plugin": GAIN, "params": {"7": 2.0}})).unwrap();
    let plain = soundcraft_mix::render_range(e.session(), Range::new(0, n as i64), block);
    let r = e.execute("mix.insert", &json!({"track": t.0, "slot": 0, "plugin": format!("sandbox:{GAIN}"), "params": {"7": 2.0}})).unwrap();
    assert_eq!(r["plugin"], format!("sandbox:{GAIN}"));
    assert_eq!(e.session().track(t).unwrap().mixer.inserts[0].as_ref().unwrap().plugin, format!("sandbox:{GAIN}"));
    // Raw latency: the sandboxed instance declares one block plus the plugin's 3 samples.
    let p = soundcraft_mix::create_instance(&format!("sandbox:{GAIN}"), None, 48_000.0, block, 1).expect("sandboxed instance");
    assert_eq!(p.latency(), block + 3, "declared added latency is one block");
    drop(p);
    // Timing of the exchange itself (debug build, container): per-block round trip.
    let mut p = soundcraft_mix::create_instance(&format!("sandbox:{GAIN}"), None, 48_000.0, block, 2).unwrap();
    let mut io = vec![vec![0.1f32; block]; 2];
    // Paced like a callback (one block period between calls), so the worker gets the CPU time
    // a real host gives it; the exchange time is what the callback would spend.
    let period = Duration::from_secs_f64(block as f64 / 48_000.0);
    let reps = if block >= 256 { 800 } else { 2000 };
    let mut times = Vec::with_capacity(reps);
    let mut next = Instant::now();
    for _ in 0..reps {
        let t0 = Instant::now();
        p.process(&mut io, block);
        times.push(t0.elapsed().as_secs_f64() * 1e6);
        next += period;
        if let Some(d) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(d);
        }
    }
    times.sort_by(f64::total_cmp);
    let st = sandbox::status().into_iter().find(|s| s.plugin == format!("sandbox:{GAIN}")).expect("status row");
    assert!(st.blocks + st.late_blocks >= reps as u64, "{st:?}");
    let m = Measured {
        block,
        blocks: st.blocks,
        late: st.late_blocks,
        p50_us: times[times.len() / 2],
        p99_us: times[times.len() * 99 / 100],
        max_us: times[times.len() - 1],
        rss_kib: st.rss_kib,
    };
    assert_eq!(st.fault, Fault::None);
    drop(p);
    // Offline render through the mix: PDC removes the block of lead; same result as in-process.
    let boxed = soundcraft_mix::render_range(e.session(), Range::new(0, n as i64), block);
    let diff = plain[0].iter().zip(&boxed[0]).skip(block * 2).take(n - block * 4).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
    assert!(diff < 1e-5, "block {block}: sandboxed render differs from in-process by {diff}");
    assert!(boxed[0][n / 2].abs() > 0.1, "signal present");
    m
}

/// A worker that aborts after some blocks: the slot goes silent (no stall, no crash here), the
/// fault is logged, and the instance comes back with its state and parameters when
/// auto-restart is on; while "recording" (auto-restart off) it stays silent and flagged.
fn crash_is_contained_and_restarted() -> Duration {
    let before = sandbox::faults().len();
    sandbox::set_fault_injection(Some("crash:30"));
    let mut p = soundcraft_mix::create_instance(&format!("sandbox:{GAIN}"), None, 48_000.0, 256, 1).unwrap();
    p.set_param("7", 1.5);
    let mut io = vec![vec![0.5f32; 256]];
    let mut first_silent = None;
    let mut crashed_at = None;
    let mut blocks_until_silent = 0;
    for k in 0..400 {
        io[0].iter_mut().for_each(|x| *x = 0.5);
        p.process(&mut io, 256);
        let st = sandbox::status().into_iter().find(|s| s.plugin == format!("sandbox:{GAIN}")).unwrap();
        if st.fault != Fault::None && crashed_at.is_none() {
            crashed_at = Some((k, Instant::now()));
        }
        if io[0][100].abs() < 1e-6 && first_silent.is_none() && k > 2 {
            first_silent = Some(k);
            blocks_until_silent = k;
        }
        if crashed_at.is_some() {
            break;
        }
    }
    let (k_crash, _t_crash) = crashed_at.expect("the crash was detected");
    assert!(blocks_until_silent <= k_crash + 1, "silence follows the crash within a block");
    let ev = sandbox::faults();
    assert!(ev.len() > before && ev.last().unwrap().fault == Fault::Crashed, "{ev:?}");
    // Recording: no silent resume. Off: the restarter brings it back with the parameter.
    sandbox::set_fault_injection(None);
    sandbox::set_auto_restart(false);
    for _ in 0..10 {
        io[0].iter_mut().for_each(|x| *x = 0.5);
        p.process(&mut io, 256);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(io[0].iter().all(|x| x.abs() < 1e-6), "stays silent while recording");
    assert_eq!(sandbox::status().into_iter().find(|s| s.plugin == format!("sandbox:{GAIN}")).unwrap().fault, Fault::Crashed);
    sandbox::set_auto_restart(true);
    let t_restart = Instant::now();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut back = None;
    while Instant::now() < deadline {
        io[0].iter_mut().for_each(|x| *x = 0.5);
        p.process(&mut io, 256);
        if io[0][100] > 0.7 {
            back = Some(t_restart.elapsed());
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let restart_time = back.expect("the worker was restarted and the parameter (1.5) re-applied");
    let st = sandbox::status().into_iter().find(|s| s.plugin == format!("sandbox:{GAIN}")).unwrap();
    assert_eq!(st.fault, Fault::None);
    assert_eq!(st.restarts, 1);
    assert!(sandbox::faults().last().unwrap().restarted);
    restart_time
}

/// A worker that stops answering: late blocks are counted and silent (never a stall longer
/// than half a block), then it is declared hung, killed and restarted.
fn hang_is_contained() -> Duration {
    sandbox::set_fault_injection(Some("hang:20"));
    let mut p = soundcraft_mix::create_instance(&format!("sandbox:{GAIN}"), None, 48_000.0, 256, 1).unwrap();
    sandbox::set_fault_injection(None);
    let mut io = vec![vec![0.5f32; 256]];
    let mut worst = Duration::ZERO;
    let t0 = Instant::now();
    let mut hung_at = None;
    for _ in 0..1200 {
        io[0].iter_mut().for_each(|x| *x = 0.5);
        let t = Instant::now();
        p.process(&mut io, 256);
        worst = worst.max(t.elapsed());
        std::thread::sleep(Duration::from_millis(2));
        let st = sandbox::status().into_iter().find(|s| s.plugin == format!("sandbox:{GAIN}")).unwrap();
        if st.fault == Fault::Hung {
            hung_at = Some(t0.elapsed());
            assert!(st.late_blocks >= 1);
            break;
        }
    }
    let detected = hung_at.expect("the hang was detected");
    assert!(worst < Duration::from_millis(50), "a callback never waited long for a hung worker: {worst:?}");
    assert!(sandbox::faults().last().unwrap().fault == Fault::Hung);
    drop(p);
    detected
}

/// State round trip through the worker, and the last checkpoint survives a crash.
fn state_round_trip() {
    let mut p = soundcraft_mix::create_instance(&format!("sandbox:{GAIN}"), None, 48_000.0, 256, 2).unwrap();
    p.set_param("7", 0.75);
    let mut io = vec![vec![0.0f32; 256]; 2];
    p.process(&mut io, 256);
    let blob = p.save_state().expect("state from the worker");
    assert!(!blob.is_empty());
    let mut q = soundcraft_mix::create_instance(&format!("sandbox:{GAIN}"), Some(&blob), 48_000.0, 256, 2).unwrap();
    let mut io = vec![vec![1.0f32; 256]; 2];
    let mut seen = Vec::new();
    for _ in 0..6 {
        io.iter_mut().for_each(|c| c.iter_mut().for_each(|x| *x = 1.0));
        q.process(&mut io, 256);
        seen.push(io[0][10]);
        std::thread::sleep(Duration::from_millis(5));
    }
    let st = sandbox::status();
    assert!((io[0][10] - 0.75).abs() < 1e-5, "restored gain 0.75 applies: {seen:?} status {st:?} faults {:?}", sandbox::faults());
    assert_eq!(q.save_state().as_deref(), Some(blob.as_slice()));
}

/// Through the engine: the fault query lists the instance; a sandboxed insert survives
/// save/reopen and offline export keeps working (one block of lead removed by PDC).
fn engine_queries() {
    let (mut e, t) = dc_session(0.25, 9600);
    e.execute("mix.insert", &json!({"track": t.0, "plugin": format!("sandbox:{GAIN}")})).unwrap();
    let text = serde_json::to_string(e.session()).unwrap();
    let back: soundcraft_model::Session = serde_json::from_str(&text).unwrap();
    assert_eq!(back.track(t).unwrap().mixer.inserts[0].as_ref().unwrap().plugin, format!("sandbox:{GAIN}"));
    let v = e.execute("session.plugin_versions", &json!({})).unwrap();
    assert_eq!(v[0]["status"], "same");
    let lat = e.execute("mix.latency_report", &json!({})).unwrap();
    assert!(
        lat["latent_processors"].as_array().unwrap().iter().any(|r| r["plugin"] == format!("sandbox:{GAIN}") && r["latency_samples"] == 512 + 3),
        "{lat}"
    );
    let f = e.execute("engine.host_faults", &json!({})).unwrap();
    assert!(f["sandboxed"].is_array() && f["faults"].as_array().unwrap().len() >= 2);
    assert_eq!(f["auto_restart"], true);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--plugin-worker") {
        let id = args.get(i + 1).cloned().unwrap_or_default();
        let dirs: Vec<PathBuf> = args.iter().skip(i + 2).map(PathBuf::from).collect();
        std::process::exit(sandbox::worker_main(&id, &dirs));
    }
    if args.iter().any(|a| a == "--list") {
        println!("sandbox: test");
        return;
    }
    let _dir = install_fixture();
    sandbox::set_worker(std::env::current_exe().unwrap(), vec!["--plugin-worker".into()]);
    assert!(soundcraft_mix::is_third_party(&format!("sandbox:{GAIN}")));
    assert!(!soundcraft_mix::is_missing_plugin(&format!("sandbox:{GAIN}")));
    let mut rows = Vec::new();
    for block in [64usize, 128, 256, 1024] {
        rows.push(parity_and_latency(block));
    }
    let restart = crash_is_contained_and_restarted();
    let hang = hang_is_contained();
    state_round_trip();
    engine_queries();
    if let Some(path) = std::env::var_os("SML_SANDBOX_CSV") {
        use std::io::Write;
        let mut f = std::fs::File::create(path).unwrap();
        writeln!(f, "case,block_frames,blocks,late_blocks,p50_us,p99_us,max_us,added_latency_samples,worker_rss_kib,provenance").unwrap();
        for m in &rows {
            writeln!(
                f,
                "exchange,{},{},{},{:.1},{:.1},{:.1},{},{},debug build in container P (not native; E03 release numbers remain the admission reference)",
                m.block, m.blocks, m.late, m.p50_us, m.p99_us, m.max_us, m.block, m.rss_kib
            )
            .unwrap();
        }
        writeln!(
            f,
            "crash restart,256,,,,,{:.1},,,time from auto-restart enabled to audio back with the re-applied parameter",
            restart.as_secs_f64() * 1e6
        )
        .unwrap();
        writeln!(
            f,
            "hang detection,256,,,,,{:.1},,,time from first block to the worker declared hung (no reply for 1 s; silent meanwhile)",
            hang.as_secs_f64() * 1e6
        )
        .unwrap();
    }
    for m in &rows {
        println!(
            "sandbox block {}: {} blocks, {} late, p50 {:.1} us p99 {:.1} us max {:.1} us rss {} KiB",
            m.block, m.blocks, m.late, m.p50_us, m.p99_us, m.max_us, m.rss_kib
        );
    }
    println!("sandbox: restart {restart:?}, hang detected in {hang:?}");
    println!("sandbox: ok");
}
