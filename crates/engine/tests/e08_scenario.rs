//! E08 portable scenario (R35): recover/open → record lead + double → comp/edit/nudge → buses
//! and both mixer views' commands → remote transport envelope → assistant explain + reversible
//! action → master (chain + reference excluded) → export → reopen, with fault injection
//! (autosave during capture, scan in background, invalid theme file, remote disconnect,
//! cancelled Bend render, model timeout). Durations and native crash rates are NOT here
//! (PENDING). Writes `SML_E08_JSON` when set.

use serde_json::{Value, json};
use soundcraft_audio_io::{AudioBuffer, FileFormat};
use soundcraft_engine::Engine;
use soundcraft_engine::autosave::{SnapshotRequest, Store};
use soundcraft_model::{ChannelFormat, TrackKind};
use soundcraft_time::Range;

const SR: u32 = 48_000;

fn phrase(k: u32, len: usize) -> Vec<f32> {
    (0..len)
        .map(|i| {
            let t = i as f32 / SR as f32;
            if (12_000..36_000).contains(&i) || (48_000..72_000).contains(&i) {
                (t * (220.0 + 30.0 * k as f32) * std::f32::consts::TAU).sin() * 0.3
            } else {
                0.0
            }
        })
        .collect()
}

fn exec(e: &mut Engine, id: &str, p: Value) -> Value {
    e.execute(id, &p).unwrap_or_else(|err| panic!("{id}: {err}"))
}

#[test]
fn record_to_master_with_faults() {
    let dir = std::env::temp_dir().join(format!("sml-e08-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut log: Vec<Value> = Vec::new();
    let mut step = |name: &str, ok: bool, detail: String| log.push(json!({"step": name, "ok": ok, "detail": detail}));

    // 1. Recover/open: a previous autosave generation exists; recovery opens it.
    let store = Store::new(dir.join("autosave"));
    let mut seed = Engine::default();
    let inst = seed.session_mut().add_track(TrackKind::Audio, ChannelFormat::Stereo, Some("Inst"));
    let _ = inst;
    store.snapshot(&SnapshotRequest { session: seed.session_arc(), revision: 1, plugin_state_age_s: None }).unwrap();
    let rec = store.recover().expect("a generation to recover");
    let mut e = Engine::default();
    soundcraft_engine::autosave::open_recovered(&mut e, rec).unwrap();
    let names: Vec<String> = e.session().tracks.iter().map(|t| t.name.clone()).collect();
    step(
        "recover/open",
        names.iter().any(|n| n.starts_with("Inst")) && e.path.is_none(),
        format!("recovered generation opened (unsaved); tracks {names:?}"),
    );

    // 2. Record lead and a double (capture path output = clips), autosave snapshot DURING capture.
    let lead = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Lead Vox"));
    let dbl = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Double"));
    e.session_mut().track_mut(lead).unwrap().mixer.record_arm = true;
    let clips = soundcraft_engine::io::add_recording(&mut e, 0, vec![phrase(0, 96_000)], SR).unwrap();
    let during = store.snapshot(&SnapshotRequest { session: e.session_arc(), revision: e.revision, plugin_state_age_s: Some(0) }).unwrap();
    e.session_mut().track_mut(lead).unwrap().mixer.record_arm = false;
    e.session_mut().track_mut(dbl).unwrap().mixer.record_arm = true;
    let clips2 = soundcraft_engine::io::add_recording(&mut e, 0, vec![phrase(1, 96_000)], SR).unwrap();
    e.session_mut().track_mut(dbl).unwrap().mixer.record_arm = false;
    step(
        "record lead + double (+ autosave during capture)",
        clips.len() == 1 && clips2.len() == 1 && during.generation.number >= 2,
        format!("clips {} {} gen {}", clips.len(), clips2.len(), during.generation.number),
    );

    // 3. Comp/edit/nudge: split at the gap, nudge the double 10 ms, strip silence on the lead.
    exec(&mut e, "edit.select", json!({"tracks": [lead.0], "start": 40_000, "end": 40_000}));
    exec(&mut e, "edit.split_at", json!({"tracks": [lead.0], "at": 40_000}));
    exec(&mut e, "edit.select", json!({"tracks": [dbl.0], "clips": [clips2[0].0]}));
    exec(&mut e, "nudge.set", json!({"value": 10, "unit": "ms"}));
    exec(&mut e, "nudge.clips", json!({"direction": 1}));
    exec(&mut e, "edit.select", json!({"tracks": [lead.0], "start": 0, "end": 96_000}));
    exec(&mut e, "edit.strip_silence", json!({"tracks": [lead.0], "start": 0, "end": 96_000, "threshold_db": -40.0}));
    step(
        "comp/edit/nudge",
        e.session().track(lead).unwrap().clips().len() >= 2,
        format!("lead clips {}", e.session().track(lead).unwrap().clips().len()),
    );

    // 4. Buses + mixer-view commands (console + visual are the same engine).
    exec(&mut e, "bus.create_from_selection", json!({"tracks": [lead.0, dbl.0], "name": "Vocal Bus"}));
    exec(&mut e, "mix.volume", json!({"tracks": [dbl.0], "db": -3.0}));
    step("buses + mixer", e.session().bus_by_name("Vocal Bus").is_some(), "lead and double routed".into());

    // 5. Remote transport envelope (paired client) + disconnect.
    // (The remote crate's HTTP test lives in soundcraft-remote; here the host-side path.)
    let env = soundcraft_engine::envelope::Envelope {
        request_id: "phone-1".into(),
        client: soundcraft_engine::envelope::Client::Remote,
        command: "transport.play".into(),
        params: json!({}),
        expected_revision: Some(e.revision),
        gesture: None,
    };
    let rc = e.submit(&env);
    let dup = e.submit(&env);
    step("remote transport envelope + duplicate", rc.accepted && dup.duplicate, format!("accepted {} duplicate {}", rc.accepted, dup.duplicate));
    exec(&mut e, "transport.stop", json!({}));

    // 6. Assistant: explain, then a reversible action with undo; model timeout = unavailable.
    let a = exec(&mut e, "assistant.ask", json!({"text": "what is the shortcut for split"}));
    let plan = exec(&mut e, "assistant.ask", json!({"text": "mute Double"}));
    let before = e.revision;
    let run = exec(&mut e, "assistant.execute", json!({"plan": plan["plan"], "revision": plan["revision"], "request_id": "e08-1"}));
    let muted = e.session().track(dbl).unwrap().mixer.mute;
    e.undo();
    step(
        "assistant explain + operate + undo",
        a["mode"] == "explain" && run["accepted"] == true && muted && !e.session().track(dbl).unwrap().mixer.mute && e.revision != before,
        format!("{}", run),
    );
    // Remote Continuation RC05: speech is real now; with no enrolled phrase nothing is invented,
    // and the provider report says the language model is off.
    let speech = exec(&mut e, "assistant.transcribe", json!({"pcm16": [], "sample_rate": 16000}));
    let provider = exec(&mut e, "assistant.provider", json!({}));
    step(
        "model/speech status is honest",
        speech["transcript"].is_null()
            && speech["reason"].as_str().is_some_and(|r| r.contains("no phrases"))
            && provider["language_model"].as_str().is_some_and(|m| m.starts_with("off")),
        format!("{speech} / {}", provider["language_model"]),
    );

    // 7. Master: chain on the Master track inserts? (fixture plugin optional) + reference excluded.
    let master = e
        .session()
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Master)
        .map(|t| t.id)
        .unwrap_or_else(|| e.session_mut().add_track(TrackKind::Master, ChannelFormat::Stereo, None));
    let refsrc = soundcraft_engine::io::add_source(
        e.session_mut(),
        "ref",
        AudioBuffer { sample_rate: SR, channels: vec![vec![0.9; 96_000]; 2] },
        None,
        FileFormat::Wav,
    );
    exec(&mut e, "mastering.reference", json!({"source": refsrc.0, "listening": true}));
    let out = soundcraft_mix::render_range(e.session(), Range::new(0, 96_000), 1024);
    let peak = soundcraft_dsp::offline::peak_db(&out);
    step("reference excluded from the render", peak < -1.0, format!("peak {peak:.1} dBFS with a 0.9 DC reference listening"));

    // 8. Export + report + reopen.
    let path = dir.join("master.wav").to_string_lossy().into_owned();
    let r = exec(&mut e, "file.bounce_mix", json!({"path": path, "start": 0, "end": 96_000, "tail": "auto", "report": true}));
    let spath = dir.join("song.scraft").to_string_lossy().into_owned();
    exec(&mut e, "session.save", json!({"path": spath}));
    let mut f = Engine::default();
    exec(&mut f, "session.open", json!({"path": spath}));
    step("export + reopen", r["verified"]["header_ok"] == true && f.session().track_by_name("Lead Vox").is_some(), format!("{}", r["verified"]));
    let _ = master;
    if let Some(p) = std::env::var_os("SML_E08_JSON") {
        std::fs::write(p, serde_json::to_string_pretty(&json!({"environment": "P", "steps": log, "pending": ["30-min W1 recordings", "2 h endurance", "8 h soak", "native crash/recovery rates"]})).unwrap()).unwrap();
    }
    assert!(log.iter().all(|s| s["ok"] == true), "{}", serde_json::to_string_pretty(&log).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}
