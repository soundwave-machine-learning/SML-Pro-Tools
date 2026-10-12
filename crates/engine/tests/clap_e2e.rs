//! A hosted CLAP plugin through the whole stack: `mix.insert` / `mix.insert_param` accept a
//! `clap:` id and the mix engine renders through it. Uses the test plugin built from
//! `crates/clap-host/tests/fixture` (a gain whose param "7" is linear gain).

use serde_json::json;
use soundcraft_audio_io::AudioBuffer;
use soundcraft_engine::Engine;
use soundcraft_model::{ChannelFormat, Clip, SourceAudio, SourceId, TrackKind};
use soundcraft_time::Range;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

fn install_fixture() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(install_fixture_now);
}

fn install_fixture_now() {
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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("engine-clap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(target.join("debug").join(lib), dir.join("Fixture.clap")).unwrap();
    assert_eq!(soundcraft_clap_host::add_search_dir(&dir), 3);
}

#[test]
fn clap_insert_renders_through_the_mix() {
    install_fixture();
    let mut e = Engine::default();
    let list = e.execute("engine.clap_plugins", &json!({})).unwrap();
    assert!(list.as_array().unwrap().iter().any(|d| d["id"] == "clap:org.soundcraft.test.gain"));

    let s = e.session_mut();
    let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
    s.pool.insert(SourceId(77), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![vec![0.25; 4800]] })));
    let id = s.new_clip_id();
    s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "dc", SourceId(77), 0, 0, 4800));
    let dry = soundcraft_mix::render_range(e.session(), Range::new(0, 4800), 512)[0][2000];
    assert!(dry.abs() > 0.01);

    let r = e.execute("mix.insert", &json!({"track": t.0, "plugin": "clap:org.soundcraft.test.gain", "params": {"7": 2.0}})).unwrap();
    assert_eq!(r["plugin"], "clap:org.soundcraft.test.gain");
    let wet = soundcraft_mix::render_range(e.session(), Range::new(0, 4800), 512)[0][2000];
    assert!((wet - 2.0 * dry).abs() < 1e-4, "dry {dry} wet {wet}");

    e.execute("mix.insert_param", &json!({"track": t.0, "slot": 0, "param": "7", "value": 0.5})).unwrap();
    let half = soundcraft_mix::render_range(e.session(), Range::new(0, 4800), 512)[0][2000];
    assert!((half - 0.5 * dry).abs() < 1e-4, "dry {dry} half {half}");
    assert!(e.execute("mix.insert_param", &json!({"track": t.0, "slot": 0, "param": "Gain", "value": 1.0})).is_err());
}

#[test]
fn clap_state_is_captured_stored_and_restored_before_the_first_block() {
    install_fixture();
    let gain = "clap:org.soundcraft.test.gain";
    let mut e = Engine::default();
    let s = e.session_mut();
    let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
    s.pool.insert(SourceId(78), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![vec![0.25; 4800]] })));
    let id = s.new_clip_id();
    s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "dc", SourceId(78), 0, 0, 4800));
    e.execute("mix.insert", &json!({"track": t.0, "plugin": gain, "params": {"7": 2.0}})).unwrap();

    // A realtime-style engine: the instance is created "on the UI thread" from the specs and
    // adopted; its live state (gain 1.5, changed behind the session's back as a plugin GUI
    // would) is captured and stored on the insert.
    let mut eng = soundcraft_mix::MixEngine::new(48_000.0, 512);
    eng.set_external_instances(true);
    let specs = soundcraft_mix::instance_specs(e.session());
    assert_eq!(specs.len(), 1);
    let mut p = soundcraft_mix::create_instance(&specs[0].id, None, 48_000.0, 512, specs[0].ch).unwrap();
    p.set_param("7", 1.5);
    let mut io = vec![vec![0.0f32; 64]];
    p.process(&mut io, 64);
    eng.adopt(vec![soundcraft_mix::PreparedInstance { track: t, slot: 0, id: gain.into(), ch: 1, plugin: p, applied: Vec::new() }]);
    eng.ensure_synced(e.session());
    let states = eng.capture_states();
    assert_eq!(states.len(), 1);
    let (tt, slot, blob) = &states[0];
    assert_eq!((*tt, *slot), (t, 0));
    e.execute("mix.insert_state", &json!({"track": t.0, "slot": slot, "state": soundcraft_model::b64::encode(blob)})).unwrap();
    assert_eq!(eng.take_retired().len(), 0);

    // Offline render (synchronous creation): the state is restored before the first block.
    // With the gain absent from the session's params, the state's 1.5 is what plays.
    let gain_of = |s: &soundcraft_model::Session| soundcraft_mix::render_range(s, Range::new(0, 4800), 512)[0][2000];
    let mut s2 = e.session().clone();
    s2.track_mut(t).unwrap().mixer.inserts[0].as_mut().unwrap().params.remove("7");
    let restored = gain_of(&s2);
    let mut s3 = e.session().clone();
    let ins = s3.track_mut(t).unwrap().mixer.inserts[0].as_mut().unwrap();
    ins.state = None;
    ins.params.insert("7".into(), 1.5);
    let reference = gain_of(&s3);
    assert!((restored - reference).abs() < 1e-5, "state {restored} vs param 1.5 {reference}");
    // Session params apply on top of the state.
    e.execute("mix.insert_param", &json!({"track": t.0, "slot": 0, "param": "7", "value": 0.5})).unwrap();
    let half = gain_of(e.session());
    assert!((half - reference / 3.0).abs() < 1e-5, "{half} vs {}", reference / 3.0);
}

/// SL-T6: the real CLAP fixture hosted in a dedicated Tape slot loads, processes, has its live
/// state captured (realtime path, slot instance index), stored on the slot, survives
/// save/reopen and is restored before the first offline block. Plumbing only — not a claim
/// that arbitrary plugins are compatible (owner plugins = H2).
#[test]
fn clap_provider_in_tape_slot_round_trips_state_and_renders() {
    install_fixture();
    let gain = "clap:org.soundcraft.test.gain";
    let mut e = Engine::default();
    let s = e.session_mut();
    let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
    let src = soundcraft_engine::io::add_source(
        s,
        "dc",
        AudioBuffer { sample_rate: 48_000, channels: vec![vec![0.25; 4800]] },
        None,
        soundcraft_audio_io::FileFormat::Wav,
    );
    let id = s.new_clip_id();
    s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "dc", src, 0, 0, 4800));
    let dry = soundcraft_mix::render_range(e.session(), Range::new(0, 4800), 512)[0][2000];
    e.execute("mix.slot", &json!({"track": t.0, "slot": "tape", "plugin": gain})).unwrap();
    let specs = soundcraft_mix::instance_specs(e.session());
    let spec = specs.iter().find(|x| x.slot == soundcraft_model::SlotKind::Tape.instance_index()).expect("slot instance spec");
    let mut eng = soundcraft_mix::MixEngine::new(48_000.0, 512);
    eng.set_external_instances(true);
    let mut p = soundcraft_mix::create_instance(&spec.id, None, 48_000.0, 512, spec.ch).unwrap();
    p.set_param("7", 1.5);
    let mut io = vec![vec![0.0f32; 64]];
    p.process(&mut io, 64);
    eng.adopt(vec![soundcraft_mix::PreparedInstance { track: t, slot: spec.slot, id: gain.into(), ch: spec.ch, plugin: p, applied: Vec::new() }]);
    eng.ensure_synced(e.session());
    let states = eng.capture_states();
    let (_, _, blob) = states.iter().find(|(tt, sl, _)| *tt == t && *sl == spec.slot).expect("slot state captured");
    e.execute("mix.slot_state", &json!({"track": t.0, "slot": "tape", "state": soundcraft_model::b64::encode(blob)})).unwrap();
    let dir = std::env::temp_dir().join(format!("sml-slot-clap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("s.scraft");
    e.execute("session.save", &json!({"path": path.to_string_lossy()})).unwrap();
    let mut f = Engine::default();
    f.execute("session.open", &json!({"path": path.to_string_lossy()})).unwrap();
    let wet = soundcraft_mix::render_range(f.session(), Range::new(0, 4800), 512)[0][2000];
    assert!((wet - 1.5 * dry).abs() < 1e-4, "restored slot state plays: dry {dry} wet {wet}");
    // Teardown: removing the provider retires the instance and the render returns to dry.
    f.execute("mix.slot", &json!({"track": f.session().track_by_name("Vox").unwrap().id.0, "slot": "tape", "plugin": ""})).unwrap();
    let back = soundcraft_mix::render_range(f.session(), Range::new(0, 4800), 512)[0][2000];
    assert!((back - dry).abs() < 1e-6);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Appends one capability row to the CSV named by `SML_HOST_MATRIX_CSV` (header once).
fn append_matrix_row(kind: &str, c: &serde_json::Value) {
    let Some(path) = std::env::var_os("SML_HOST_MATRIX_CSV") else { return };
    use std::io::Write;
    let new = !Path::new(&path).exists();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    if new {
        writeln!(f, "kind,id,format,name,version,audio_inputs,audio_outputs,aux_inputs,sidechain,note_input,params,param_units,editor,state,latency_samples,tail_samples,probed_at,host_notes,platform").unwrap();
    }
    let j = |k: &str| c[k].to_string().replace(',', ";").replace('"', "");
    writeln!(
        f,
        "{kind},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{} {}",
        j("id"),
        j("format"),
        j("name"),
        j("version"),
        j("audio_inputs"),
        j("audio_outputs"),
        j("aux_inputs"),
        j("sidechain"),
        j("note_input"),
        j("params"),
        j("param_units"),
        j("editor"),
        j("state"),
        j("latency_samples"),
        j("tail_samples"),
        j("probed_at"),
        j("host_notes"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
    .unwrap();
}

/// R31 capability matrix: the CLAP fixture's record is read from a live instance and matches
/// what the fixture declares (ports, latency, state, GUI, parameters).
#[test]
fn capability_matrix_reads_the_clap_fixture_from_a_live_instance() {
    install_fixture();
    let mut e = Engine::default();
    let v = e.execute("engine.plugin_capabilities", &json!({"id": "clap:org.soundcraft.test.gain"})).unwrap();
    let rows = v["plugins"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{rows:?}");
    let c = &rows[0];
    assert_eq!(c["format"], "CLAP");
    assert_eq!(c["audio_inputs"], json!([2]));
    assert_eq!(c["audio_outputs"], json!([2]));
    assert_eq!(c["aux_inputs"], 0);
    assert_eq!(c["latency_samples"], 3);
    assert_eq!(c["state"], true);
    assert_eq!(c["params"], 2, "hidden param 9 is not visible");
    assert!(c["editor"].as_str().unwrap().starts_with("available"));
    assert_eq!(c["note_input"], false);
    append_matrix_row("deterministic host fixture (CLAP)", c);
    let synth = e.execute("engine.plugin_capabilities", &json!({"id": "clap:org.soundcraft.test.synth"})).unwrap();
    let s = &synth["plugins"][0];
    assert_eq!(s["note_input"], true);
    assert_eq!(s["is_instrument"], true);
    append_matrix_row("deterministic host fixture (CLAP)", s);
    for t in v["external_targets"].as_array().unwrap() {
        append_matrix_row(
            "external target",
            &json!({"id": t["requirement"], "format": "unknown until supplied", "name": t["project"], "version": "", "sidechain": t["host_work"], "editor": t["status"]}),
        );
    }
}

/// A real CLAP plugin with a sidechain port (fixture `sidegate`) hosted on an aux: the insert's
/// sidechain bus reaches the plugin's aux input port, the gate opens exactly at the key; the
/// capability record says so; the route survives save/reopen; signal flow and latency report
/// name it.
#[test]
fn clap_sidechain_port_is_fed_from_the_insert_sidechain_bus() {
    install_fixture();
    let mut e = Engine::default();
    let s = e.session_mut();
    let n = 4800usize;
    let mut kick = vec![0.0f32; n];
    kick[1000] = 1.0;
    kick[2500] = 1.0;
    let k = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Kick"));
    s.pool.insert(SourceId(80), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![kick] })));
    let id = s.new_clip_id();
    s.track_mut(k).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "k", SourceId(80), 0, 0, n as i64));
    let vox = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
    s.pool.insert(SourceId(81), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![vec![0.5; n]] })));
    let id = s.new_clip_id();
    s.track_mut(vox).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "v", SourceId(81), 0, 0, n as i64));
    let sc = s.add_bus("SC", ChannelFormat::Stereo);
    let vb = s.add_bus("VoxBus", ChannelFormat::Stereo);
    let aux = s.add_track(TrackKind::Aux, ChannelFormat::Stereo, Some("Gate"));
    s.track_mut(aux).unwrap().mixer.input = soundcraft_model::Route::Bus(vb);
    s.track_mut(k).unwrap().mixer.output = soundcraft_model::Route::None;
    s.track_mut(k).unwrap().mixer.sends[0] =
        Some(soundcraft_model::SendSlot { level_db: 0.0, ..soundcraft_model::SendSlot::new(soundcraft_model::Route::Bus(sc)) });
    s.track_mut(vox).unwrap().mixer.output = soundcraft_model::Route::Bus(vb);
    e.execute("mix.insert", &json!({"track": aux.0, "plugin": "clap:org.soundcraft.test.sidegate"})).unwrap();
    assert!(e.execute("mix.insert_sidechain", &json!({"track": aux.0, "slot": 0, "bus": "NoSuch"})).is_err());
    let r = e.execute("mix.insert_sidechain", &json!({"track": aux.0, "slot": 0, "bus": "SC"})).unwrap();
    assert_eq!(r["sidechain"], sc.0);
    let out = soundcraft_mix::render_range(e.session(), Range::new(0, n as i64), 512);
    let open: Vec<usize> = out[0].iter().enumerate().filter(|(_, x)| x.abs() > 1e-4).map(|(i, _)| i).collect();
    assert_eq!(open, vec![1000, 2500], "the CLAP sidechain port hears the SC bus exactly");
    let caps = e.execute("engine.plugin_capabilities", &json!({"id": "clap:org.soundcraft.test.sidegate"})).unwrap();
    assert_eq!(caps["plugins"][0]["aux_inputs"], 1);
    assert!(caps["plugins"][0]["sidechain"].as_str().unwrap().contains("routed"));
    let flow = e.execute("mix.signal_flow", &json!({"track": aux.0})).unwrap();
    assert!(flow.to_string().contains("sidechain: SC"), "{flow}");
    let lat = e.execute("mix.latency_report", &json!({})).unwrap();
    assert_eq!(lat["sidechains"][0]["bus"], "SC");
    assert!(lat["sidechains"][0]["detector_lag"].as_str().unwrap().starts_with("none"));
    // Round trip through the document format.
    let text = serde_json::to_string(e.session()).unwrap();
    let back: soundcraft_model::Session = serde_json::from_str(&text).unwrap();
    assert_eq!(back.track(aux).unwrap().mixer.inserts[0].as_ref().unwrap().sidechain, Some(sc));
    e.execute("mix.insert_sidechain", &json!({"track": aux.0, "slot": 0, "bus": null})).unwrap();
    let out = soundcraft_mix::render_range(e.session(), Range::new(0, n as i64), 512);
    assert!(out[0].iter().all(|x| x.abs() < 1e-6), "no sidechain: the gate stays closed (silence on its port)");
}

/// R31: an export never silently yields a substituted master. A hosted plugin that passes the
/// missing-plugin check but fails to load (here a nonexistent deterministic provider id) makes
/// the bounce refuse with the plugin named; the user can acknowledge explicitly. The tail
/// option renders past the range end.
#[test]
fn export_refuses_a_substituted_master_unless_acknowledged() {
    install_fixture();
    let mut e = Engine::default();
    let s = e.session_mut();
    let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
    s.pool.insert(SourceId(90), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![vec![0.25; 4800]] })));
    let id = s.new_clip_id();
    s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "dc", SourceId(90), 0, 0, 4800));
    s.track_mut(t).unwrap().mixer.inserts[0] = Some(soundcraft_model::Insert::new("test:nonexistent:1"));
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("bounce-guard-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mix.wav").to_string_lossy().into_owned();
    let err = e.execute("file.bounce_mix", &json!({"path": path, "start": 0, "end": 4800})).unwrap_err().to_string();
    assert!(err.contains("failed to load") && err.contains("test:nonexistent:1"), "{err}");
    assert!(!Path::new(&path).exists(), "nothing written");
    e.execute("file.bounce_mix", &json!({"path": path, "start": 0, "end": 4800, "acknowledge_missing": true, "tail": 0.5})).unwrap();
    let (_, buf) = soundcraft_audio_io::decode(&std::fs::read(&path).unwrap(), Some("wav")).unwrap();
    assert_eq!(buf.channels[0].len(), 4800 + 24_000, "tail of 0.5 s rendered past the range");
    assert!(e.execute("file.bounce_mix", &json!({"path": path, "tail": "later"})).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// R31: the plugin version at insert time is recorded and compared with what is installed.
#[test]
fn plugin_versions_are_recorded_and_compared() {
    install_fixture();
    let mut e = Engine::default();
    e.execute("track.new", &json!({})).unwrap();
    let t = e.session().tracks[0].id;
    e.execute("mix.insert", &json!({"track": t.0, "plugin": "clap:org.soundcraft.test.gain"})).unwrap();
    assert_eq!(e.session().track(t).unwrap().mixer.inserts[0].as_ref().unwrap().plugin_version.as_deref(), Some("1.0.0"));
    let v = e.execute("session.plugin_versions", &json!({})).unwrap();
    assert_eq!(v[0]["status"], "same");
    let s = e.session_mut();
    s.track_mut(t).unwrap().mixer.inserts[0].as_mut().unwrap().plugin_version = Some("0.9.0".into());
    s.track_mut(t).unwrap().mixer.inserts[1] = Some(soundcraft_model::Insert::new("clap:org.soundcraft.test.synth"));
    s.track_mut(t).unwrap().mixer.inserts[2] = Some(soundcraft_model::Insert::new("clap:com.example.gone"));
    let v = e.execute("session.plugin_versions", &json!({})).unwrap();
    let st: Vec<&str> = v.as_array().unwrap().iter().map(|r| r["status"].as_str().unwrap()).collect();
    assert_eq!(st, vec!["changed", "unrecorded", "missing"]);
    let text = serde_json::to_string(e.session()).unwrap();
    assert!(text.contains("\"plugin_version\":\"0.9.0\""));
}
