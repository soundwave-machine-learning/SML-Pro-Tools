//! Sprint 3 task 9 (portable control for the native W1 run): the W1 workload (16 tracks, 2000
//! clips) with a documented hosted chain — the real CLAP fixture on every track and the same
//! plugin sandboxed on two tracks — rendered block by block through a realtime-style engine
//! (instances prepared off the audio thread and adopted) while autosave snapshots and
//! plugin-state captures run concurrently. Per-block render time against the block budget;
//! written to `SML_W1_HOSTED_CSV` when set. Not a native measurement (no device, no GUI).

use soundcraft_audio_io::AudioBuffer;
use soundcraft_engine::autosave::{SnapshotRequest, Store};
use soundcraft_model::{ChannelFormat, Clip, Insert, Session, SourceAudio, SourceId, TrackKind};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

const GAIN: &str = "clap:org.soundcraft.test.gain";

fn install_fixture() {
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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("engine-w1-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(target.join("debug").join(lib), dir.join("Fixture.clap")).unwrap();
    assert_eq!(soundcraft_clap_host::add_search_dir(&dir), 3);
}

fn w1_hosted() -> Session {
    let mut s = Session::default();
    s.pool.insert(SourceId(1), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![vec![0.1; 48_000]] })));
    for k in 0..16 {
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some(&format!("Track {k}")));
        for c in 0..125 {
            let id = s.new_clip_id();
            s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "c", SourceId(1), 0, c * 50_000, 40_000));
        }
        let plugin = if k < 2 { format!("sandbox:{GAIN}") } else { GAIN.to_string() };
        let mut ins = Insert::new(plugin);
        ins.params.insert("7".into(), 0.9);
        s.track_mut(t).unwrap().mixer.inserts[0] = Some(ins);
    }
    s.add_track(TrackKind::Master, ChannelFormat::Stereo, None);
    s
}

fn stats(mut v: Vec<f64>) -> (f64, f64, f64, f64) {
    v.sort_by(f64::total_cmp);
    let q = |p: f64| v[((v.len() as f64 - 1.0) * p).round() as usize];
    (q(0.5), q(0.95), q(0.99), *v.last().unwrap_or(&0.0))
}

#[test]
fn w1_hosted_chain_under_autosave_and_state_capture() {
    install_fixture();
    // The sandbox worker is the `sandbox` test binary of this crate (built by the same `cargo
    // test` invocation when both targets are named); without it the chain runs in-process.
    let deps = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
    let worker = std::fs::read_dir(&deps).ok().and_then(|rd| {
        rd.flatten()
            .map(|e| e.path())
            .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("sandbox-") && !n.ends_with(".d")))
            .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
    });
    let worker_ok = match worker {
        Some(w) => {
            soundcraft_mix::sandbox::set_worker(w, vec!["--plugin-worker".into()]);
            soundcraft_mix::sandbox::create(&format!("sandbox:{GAIN}")).is_some()
        }
        None => false,
    };
    let mut s = w1_hosted();
    if !worker_ok {
        // The sandbox test binary is not built in this invocation: run the chain in-process only.
        for t in s.tracks.iter_mut().take(2) {
            t.mixer.inserts[0] = Some(Insert::new(GAIN));
        }
    }
    let block = 256usize;
    let sr = 48_000.0f32;
    let budget_us = block as f64 / f64::from(sr) * 1e6;
    let mut eng = soundcraft_mix::MixEngine::new(sr, block);
    eng.set_external_instances(true);
    let t0 = Instant::now();
    let instances = soundcraft_mix::prepare_instances(&s, sr, block);
    let prepare_ms = t0.elapsed().as_secs_f64() * 1e3;
    assert_eq!(instances.len(), 16);
    eng.adopt(instances);
    eng.ensure_synced(&s);
    // Autosave snapshots in the background while rendering (the writer is a separate thread, as
    // in the app), and plugin-state captures every 2 s of audio.
    let dir = std::env::temp_dir().join(format!("sml-w1-hosted-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::new(&dir);
    let snap_session = Arc::new(s.clone());
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let snaps = {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut n = 0u64;
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                store.snapshot(&SnapshotRequest { session: Arc::clone(&snap_session), revision: n, plugin_state_age_s: Some(0) }).unwrap();
                n += 1;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            n
        })
    };
    let seconds = 20usize;
    let blocks = seconds * 48_000 / block;
    let mut out = vec![vec![0.0f32; block]; 2];
    let mut times = Vec::with_capacity(blocks);
    let mut captures = 0;
    let mut capture_ms = Vec::new();
    for k in 0..blocks {
        let t = Instant::now();
        eng.render(&s, (k * block) as i64, block, &mut out);
        times.push(t.elapsed().as_secs_f64() * 1e6);
        if k % (2 * 48_000 / block) == 0 {
            let t = Instant::now();
            let states = eng.capture_states();
            capture_ms.push(t.elapsed().as_secs_f64() * 1e3);
            assert_eq!(states.len(), 16, "every hosted instance answers a state capture");
            captures += 1;
        }
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let snapshots = snaps.join().unwrap();
    assert!(out[0].iter().any(|x| x.abs() > 1e-4), "audio flowed");
    let (p50, p95, p99, max) = stats(times.clone());
    let over = times.iter().filter(|t| **t > budget_us).count();
    let (c50, _, _, cmax) = if capture_ms.is_empty() { (0.0, 0.0, 0.0, 0.0) } else { stats(capture_ms.clone()) };
    let faults = soundcraft_mix::sandbox::faults();
    eprintln!(
        "W1 hosted: {blocks} blocks of {block} @ {sr} Hz; render p50 {p50:.0} / p95 {p95:.0} / p99 {p99:.0} / max {max:.0} us (budget {budget_us:.0}); over budget {over}; prepare {prepare_ms:.1} ms; {captures} state captures (p50 {c50:.1} ms max {cmax:.1} ms); {snapshots} autosave snapshots; sandbox faults {}; sandboxed {}",
        faults.len(),
        worker_ok
    );
    assert!(faults.is_empty(), "{faults:?}");
    assert!(p99 < budget_us * 4.0, "p99 {p99} us far over budget {budget_us} us (debug build; see the CSV)");
    if let Some(path) = std::env::var_os("SML_W1_HOSTED_CSV") {
        use std::io::Write;
        let mut f = std::fs::File::create(path).unwrap();
        writeln!(f, "workload,chain,block_frames,blocks,render_p50_us,render_p95_us,render_p99_us,render_max_us,budget_us,blocks_over_budget,prepare_ms,state_captures,capture_p50_ms,capture_max_ms,autosave_snapshots,sandbox_faults,provenance").unwrap();
        writeln!(
            f,
            "W1 (16 tracks / 2000 clips),CLAP fixture gain on 16 tracks ({}),{block},{blocks},{p50:.1},{p95:.1},{p99:.1},{max:.1},{budget_us:.1},{over},{prepare_ms:.1},{captures},{c50:.2},{cmax:.2},{snapshots},{},headless debug build in container P: no device callback no GUI; native W1/W2 pending",
            if worker_ok { "2 of them sandboxed" } else { "all in-process; sandbox worker binary absent" },
            faults.len()
        )
        .unwrap();
    }
    let _ = std::fs::remove_dir_all(&dir);
}
