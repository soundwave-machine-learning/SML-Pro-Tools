//! Dedicated Summing/Tape slots, mix profile, audition bypass, monitoring modes, latency report
//! and the named V13 export paths (MIXER_SLOT_SPEC SL01–SL05, MIXER_MODES_SPEC M01–M05,
//! CLOCK_LATENCY_SPEC CL03, VOCAL_EDITING_SPEC V13).
//!
//! Slots host installed external plugins (or stay Clean/None identity). Native summing/tape sound
//! models are deferred by the owner: no model knobs exist and none are faked here.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::{DedicatedSlot, GRAPH_VERSION, Insert, ProfileKind, SlotKind, StereoPanMode, TapePosition, TrackKind};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "mix.slot",
            "Set Dedicated Slot",
            [],
            None,
            "{track?, slot: tape|summing|master_tape, plugin?: id|\"\" (clean/none), bypass?: bool, trim_db?: -24..24, position?: pre|post}",
            has_selection,
            set_slot
        ),
        cmd!(
            "mix.profile",
            "Mix Profile",
            ["Mix"],
            None,
            "{kind?: standard|logic, law?: 0db|-3db|-3db_compensated, stereo_mode?: balance|stereo_pan, balance_compensated?: bool}",
            always,
            profile
        ),
        cmd!(
            "mix.graph_migrate",
            "Migrate Mixer Graph",
            [],
            None,
            "{} — move a legacy (version 1) session to the current graph version",
            always,
            |e, _| {
                let s = e.session_mut();
                let from = s.mix_profile.graph_version;
                if from >= GRAPH_VERSION {
                    return Err(bad("mix.graph_migrate", "already on the current graph version"));
                }
                s.mix_profile.graph_version = GRAPH_VERSION;
                Ok(
                    json!({"from": from, "to": GRAPH_VERSION, "note": "send/mute/solo policy, follow-send panning and f64 bus sums now apply; save as a new file to keep the original"}),
                )
            }
        ),
        cmd!(noundo "mix.slot_state", "Store Slot Plugin State", [], None, "{track, slot: tape|summing|master_tape, state: base64|null} — not undoable (like mix.insert_state)", has_tracks, |e, p| {
            let cmd = "mix.slot_state";
            let kind = kind_param(cmd, p)?;
            let t = track_param(e, cmd, p, "track")?.ok_or_else(|| bad(cmd, "`track` required"))?;
            let state = match p.get("state") {
                Some(Value::Null) | None => None,
                Some(Value::String(b)) => {
                    let bytes = soundcraft_model::b64::decode(b).ok_or_else(|| bad(cmd, "state is not valid base64"))?;
                    if bytes.len() > 64 << 20 {
                        return Err(bad(cmd, "state larger than 64 MB"));
                    }
                    Some(b.clone())
                }
                Some(_) => return Err(bad(cmd, "state must be base64 text or null")),
            };
            let s = e.session_mut();
            let prov = s
                .track_mut(t)
                .and_then(|tr| tr.mixer.slots.get_mut(kind).as_mut())
                .and_then(|x| x.provider.as_mut())
                .ok_or_else(|| bad(cmd, "that slot has no provider"))?;
            prov.state = state;
            Ok(json!({"stored": prov.state.is_some()}))
        }),
        cmd!(noundo "mix.audition_bypass", "Audition Bypass", [], None, "{summing?, tape?, inserts?, all?: bool} — temporary; never saved or exported", always, audition),
        cmd!(noundo "mix.monitor_mode", "Monitoring Mode", [], None, "{mode: tracking|mixing|hardware}", always, monitor_mode),
        cmd!(query "mix.latency_report", "Latency Report", [], None, "{}", always, |e, _| Ok(latency_report(e))),
        cmd!(query "mix.signal_flow", "Signal Flow", [], None, "{track}", has_tracks, signal_flow),
        cmd!(noundo "file.export_path", "Export Named Path", [], None, "{path, kind: raw|edited_dry|processed|full_mix, track?, start?, end?, acknowledge_missing?}", always, export_path),
    ]
}

fn kind_param(cmd: &str, p: &Value) -> Result<SlotKind> {
    str_param(p, "slot").and_then(SlotKind::parse).ok_or_else(|| bad(cmd, "`slot` must be tape, summing or master_tape"))
}

fn set_slot(e: &mut Engine, p: &Value) -> Result<Value> {
    let cmd = "mix.slot";
    let kind = kind_param(cmd, p)?;
    let t = tracks_required(e, cmd, p)?.first().copied().ok_or_else(|| bad(cmd, "no track"))?;
    let playing = e.transport.playing;
    let s = e.session();
    let tr = s.track(t).ok_or_else(|| bad(cmd, "no track"))?;
    let is_bus = matches!(tr.kind, TrackKind::Aux | TrackKind::Folder | TrackKind::Master);
    match kind {
        SlotKind::Tape if is_bus => return Err(bad(cmd, "buses and Main have Master Tape, not track Tape")),
        SlotKind::Summing | SlotKind::MasterTape if !is_bus => return Err(bad(cmd, "Summing and Master Tape slots are on buses and Main")),
        _ => {}
    }
    let structural = p.get("plugin").is_some() || p.get("position").is_some();
    if structural && playing {
        return Err(bad(cmd, "stop playback to change a slot's provider or position (graph changes apply stopped)"));
    }
    let provider = match p.get("plugin") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(v) => {
            let id = v.as_str().unwrap_or("").trim();
            if id.is_empty() {
                Some(None)
            } else if soundcraft_dsp::is_removed_builtin(id) {
                return Err(bad(cmd, format!("`{id}` was an upstream bundled processor and is not part of SML Recording")));
            } else if soundcraft_mix::is_missing_plugin(id) {
                return Err(bad(cmd, format!("`{id}` is not an installed VST3/CLAP plugin")));
            } else {
                Some(Some(Insert::new(id)))
            }
        }
    };
    let position = match str_param(p, "position") {
        None => None,
        Some("pre") => Some(TapePosition::Pre),
        Some("post") => Some(TapePosition::Post),
        Some(other) => return Err(bad(cmd, format!("position `{other}` must be pre or post"))),
    };
    if position.is_some() && kind != SlotKind::MasterTape {
        return Err(bad(cmd, "only Master Tape has a Pre/Post position"));
    }
    let is_master = tr.kind == TrackKind::Master;
    let had_slot = tr.mixer.slots.get(kind).is_some();
    let new_id = e.session_mut().alloc();
    let s = e.session_mut();
    let tr = s.track_mut(t).ok_or_else(|| bad(cmd, "no track"))?;
    if !had_slot && kind == SlotKind::MasterTape && position.is_none() {
        // Versioned defaults: Pre on Main (a user limiter can follow), Post on buses.
        tr.mixer.slots.master_tape_position = if is_master { TapePosition::Pre } else { TapePosition::Post };
    }
    let slot = tr.mixer.slots.get_mut(kind).get_or_insert_with(|| DedicatedSlot::new(new_id));
    if let Some(prov) = provider {
        let removed = prov.is_none();
        slot.provider = prov;
        if removed {
            // Removing a provider resets its host trim in the same undo step (SL04).
            slot.trim_db = 0.0;
            slot.bypass = false;
        }
    }
    if let Some(b) = p.get("bypass").and_then(Value::as_bool) {
        slot.bypass = b;
    }
    if let Some(d) = p.get("trim_db").and_then(Value::as_f64).filter(|v| v.is_finite()) {
        slot.trim_db = (d as f32).clamp(-24.0, 24.0);
    }
    if let Some(pos) = position {
        tr.mixer.slots.master_tape_position = pos;
    }
    let out = tr.mixer.slots.get(kind).cloned();
    Ok(json!({"slot": kind.id(), "state": out, "position": format!("{:?}", tr.mixer.slots.master_tape_position)}))
}

fn profile(e: &mut Engine, p: &Value) -> Result<Value> {
    let cmd = "mix.profile";
    let s = e.session_mut();
    let mut pr = s.mix_profile.clone();
    if let Some(k) = str_param(p, "kind") {
        pr.kind = match k {
            "standard" => ProfileKind::Standard,
            "logic" | "logic_style" => ProfileKind::LogicStyle,
            _ => return Err(bad(cmd, "kind must be standard or logic")),
        };
    }
    if let Some(l) = str_param(p, "law") {
        let law = soundcraft_dsp::mixmath::ProfileLaw::parse(l)
            .ok_or_else(|| bad(cmd, "law must be 0db, -3db or -3db_compensated (other laws are not implemented)"))?;
        pr.law = law.id().into();
    }
    if let Some(m) = str_param(p, "stereo_mode") {
        pr.stereo_mode = match m {
            "balance" => StereoPanMode::Balance,
            "stereo_pan" => StereoPanMode::StereoPan,
            _ => return Err(bad(cmd, "stereo_mode must be balance or stereo_pan")),
        };
    }
    if let Some(c) = p.get("balance_compensated").and_then(Value::as_bool) {
        pr.balance_compensated = c;
    }
    s.mix_profile = pr.clone();
    Ok(json!(pr))
}

fn audition(e: &mut Engine, p: &Value) -> Result<Value> {
    let s = e.session_mut();
    let all = p.get("all").and_then(Value::as_bool);
    let get = |k: &str, cur: bool| p.get(k).and_then(Value::as_bool).or(all).unwrap_or(cur);
    s.audition.summing = get("summing", s.audition.summing);
    s.audition.tape = get("tape", s.audition.tape);
    s.audition.inserts = get("inserts", s.audition.inserts);
    Ok(
        json!({"audition": s.audition, "notice": if s.audition.any() { "monitoring differs from the saved mix; exports use the normal saved state" } else { "" }}),
    )
}

fn monitor_mode(e: &mut Engine, p: &Value) -> Result<Value> {
    let mode = match str_param(p, "mode") {
        Some("tracking") => 0.0,
        Some("mixing") => 1.0,
        Some("hardware") => 2.0,
        _ => return Err(bad("mix.monitor_mode", "mode must be tracking, mixing or hardware")),
    };
    if e.transport.playing {
        return Err(bad("mix.monitor_mode", "change the monitoring mode while stopped"));
    }
    e.session_mut().edit.values.insert("monitor.mode".into(), mode);
    let note = match mode as i64 {
        0 => {
            "Tracking: small buffer advised; latency contributors are listed by mix.latency_report. Bypassing a plugin may keep its latency; deactivate it (stopped) to remove it."
        }
        2 => {
            "Hardware Monitor: software monitoring of armed inputs is off; use your interface's direct monitoring (plugins are not on that path). Playback continues."
        }
        _ => "Mixing: full processing and delay compensation.",
    };
    Ok(json!({"mode": str_param(p, "mode"), "note": note, "recording": "always dry, in every mode"}))
}

fn latency_report(e: &Engine) -> Value {
    let s = e.session();
    let rows = soundcraft_mix::strip_latencies(s);
    let worst = rows.iter().map(|(_, l)| *l).max().unwrap_or(0);
    let v: Vec<Value> = rows
        .iter()
        .filter_map(|(t, l)| s.track(*t).map(|tr| json!({"track": tr.name, "latency_samples": l, "ms": *l as f64 * 1000.0 / s.sample_rate.as_f64()})))
        .collect();
    json!({"strips": v, "worst_samples": worst, "provenance": "plugin-reported latency (inserts + dedicated slots); device/driver latency is Unknown until measured"})
}

fn signal_flow(e: &mut Engine, p: &Value) -> Result<Value> {
    let t = track_param(e, "mix.signal_flow", p, "track")?.ok_or_else(|| bad("mix.signal_flow", "`track` required"))?;
    let s = e.session();
    let tr = s.track(t).ok_or_else(|| bad("mix.signal_flow", "no track"))?;
    let slot = |k: SlotKind| {
        tr.mixer.slots.get(k).map_or_else(|| "empty".to_string(), |x| x.provider.as_ref().map_or("Clean/None".into(), |i| i.plugin.clone()))
    };
    let inserts: Vec<String> =
        tr.mixer.inserts.iter().enumerate().filter_map(|(i, x)| x.as_ref().map(|x| format!("{}:{}", (b'a' + i as u8) as char, x.plugin))).collect();
    let sends: Vec<String> = tr
        .mixer
        .sends
        .iter()
        .flatten()
        .map(|x| {
            format!(
                "{:?} {} ({})",
                x.target,
                if x.pre_fader { "pre-fader" } else { "post-fader" },
                if x.follow_main_pan { "follow pan" } else { "independent pan" }
            )
        })
        .collect();
    let order: Vec<String> = match tr.kind {
        TrackKind::Aux | TrackKind::Folder | TrackKind::Master => {
            let pre = tr.mixer.slots.master_tape_position == TapePosition::Pre;
            let mut v = vec![
                format!("sum (f64, graph v{})", s.mix_profile.graph_version),
                format!("Summing [{}]", slot(SlotKind::Summing)),
                "utility trim/polarity".into(),
            ];
            if pre {
                v.push(format!("Master Tape Pre [{}]", slot(SlotKind::MasterTape)));
            }
            v.extend(["user inserts".to_string(), format!("Width {:.0}%", tr.mixer.width * 100.0), "pre-fader send tap".into(), "fader".into()]);
            if !pre {
                v.push(format!("Master Tape Post [{}]", slot(SlotKind::MasterTape)));
            }
            v.extend([
                "post-fader send tap".to_string(),
                if tr.kind == TrackKind::Master { "Main (stereo identity)".into() } else { "output panner".into() },
                "output route/mute".into(),
            ]);
            v
        }
        _ => vec![
            "source (comp, clip gain/fades, edits)".into(),
            "utility trim/polarity".into(),
            format!("Tape [{}]", slot(SlotKind::Tape)),
            "user inserts".into(),
            format!("Width {:.0}%", tr.mixer.width * 100.0),
            "pre-fader send tap".into(),
            "fader".into(),
            "post-fader send tap".into(),
            "output panner".into(),
            "output route/mute".into(),
        ],
    };
    Ok(
        json!({"track": tr.name, "order": order, "inserts": inserts, "sends": sends, "note": "user inserts are pre-fader in this build (no Post-insert section yet)"}),
    )
}

fn export_path(e: &mut Engine, p: &Value) -> Result<Value> {
    let cmd = "file.export_path";
    let path = str_param(p, "path").ok_or_else(|| bad(cmd, "`path` required"))?.to_string();
    let kind = str_param(p, "kind").ok_or_else(|| bad(cmd, "`kind` required: raw|edited_dry|processed|full_mix"))?.to_string();
    let r = range_param(e, cmd, p)?;
    let s = e.session();
    let r = if r.is_empty() { soundcraft_time::Range::new(0, s.content_end().max(1)) } else { r };
    let track = track_param(e, cmd, p, "track")?;
    let (channels, label) = match kind.as_str() {
        "raw" => {
            let t = track.ok_or_else(|| bad(cmd, "`track` required"))?;
            let tr = s.track(t).ok_or_else(|| bad(cmd, "no track"))?;
            let clip = tr.clips().iter().find(|c| c.range().overlaps(&r)).ok_or_else(|| bad(cmd, "no clip in range"))?;
            let src = clip.source().ok_or_else(|| bad(cmd, "not an audio clip"))?;
            let audio = s.pool.get(src).ok_or_else(|| bad(cmd, "source audio is not loaded"))?;
            (audio.buffer.channels.clone(), "Raw Source: original media, unedited")
        }
        "edited_dry" => {
            let t = track.ok_or_else(|| bad(cmd, "`track` required"))?;
            (soundcraft_mix::render_clips(s, t, r), "Edited Dry: comp, clip edits/gain/fades only — before trim, Tape, inserts, fader, pan and sends")
        }
        "processed" => {
            missing_plugin_guard(e, cmd, p)?;
            let t = track.ok_or_else(|| bad(cmd, "`track` required"))?;
            let mut iso = e.session().clone();
            for tr in iso.tracks.iter_mut() {
                if tr.id == t {
                    tr.mixer.output = soundcraft_model::Route::Main;
                    tr.mixer.sends.iter_mut().for_each(|x| *x = None);
                } else {
                    tr.inactive = true;
                }
            }
            (
                soundcraft_mix::render_range(&iso, r, 1024),
                "Processed Track: channel final output (slots, inserts, width, fader, pan) before destination-bus processing",
            )
        }
        "full_mix" => {
            missing_plugin_guard(e, cmd, p)?;
            (soundcraft_mix::render_range(s, r, 1024), "Full Mix: Main after all processing, normal saved state, no monitor-only overrides")
        }
        other => return Err(bad(cmd, format!("unknown path `{other}`"))),
    };
    let rate = s.sample_rate.hz();
    let frames = channels.first().map_or(0, Vec::len);
    let buf = soundcraft_audio_io::AudioBuffer { sample_rate: rate, channels };
    let opts = soundcraft_audio_io::EncodeOptions {
        format: soundcraft_audio_io::FileFormat::Wav,
        bit_depth: soundcraft_audio_io::BitDepth::Float32,
        dither: false,
        bwf: None,
    };
    let bytes = soundcraft_audio_io::encode(&buf, &opts).map_err(|err| EngineError::Io(err.to_string()))?;
    std::fs::write(&path, bytes).map_err(|err| EngineError::Io(err.to_string()))?;
    Ok(json!({"path": path, "kind": kind, "frames": frames, "start": r.start, "end": r.end, "label": label}))
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use serde_json::json;
    use soundcraft_audio_io::{AudioBuffer, FileFormat};
    use soundcraft_model::{ChannelFormat, Clip, Insert, TrackKind};

    fn one_track() -> (Engine, soundcraft_model::TrackId) {
        let mut e = Engine::default();
        let s = e.session_mut();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
        let src = crate::io::add_source(s, "vox", AudioBuffer { sample_rate: 48_000, channels: vec![vec![0.5; 4800]] }, None, FileFormat::Wav);
        let id = s.new_clip_id();
        let mut c = Clip::audio(id, "vox", src, 0, 0, 4800);
        c.gain_db = -6.0;
        s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(c);
        s.add_track(TrackKind::Master, ChannelFormat::Stereo, None);
        (e, t)
    }

    /// SL-T4: slot id/provider/bypass/trim/position survive undo and save/reopen; replace/remove
    /// keep the id; removing resets trim; removed built-ins and unknown ids are rejected;
    /// structural changes are refused while playing.
    #[test]
    fn slt4_slot_state_persistence_and_rules() {
        let (mut e, t) = one_track();
        e.execute("mix.slot", &json!({"track": t.0, "slot": "tape", "plugin": "test:gain:-3", "trim_db": -2.0})).unwrap();
        let id = e.session().track(t).unwrap().mixer.slots.tape.as_ref().unwrap().id;
        assert!(
            e.execute("mix.slot", &json!({"track": t.0, "slot": "tape", "plugin": "saturator"}))
                .unwrap_err()
                .to_string()
                .contains("not part of SML Recording")
        );
        assert!(e.execute("mix.slot", &json!({"track": t.0, "slot": "tape", "plugin": "vst3:ffffffffffffffffffffffffffffffff"})).is_err());
        assert!(e.execute("mix.slot", &json!({"track": t.0, "slot": "summing", "plugin": ""})).is_err(), "tracks have no Summing slot");
        e.execute("mix.slot", &json!({"track": t.0, "slot": "tape", "bypass": true})).unwrap();
        let dir = std::env::temp_dir().join(format!("sml-slots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.scraft");
        e.execute("session.save", &json!({"path": path.to_string_lossy()})).unwrap();
        let mut f = Engine::default();
        f.execute("session.open", &json!({"path": path.to_string_lossy()})).unwrap();
        let back = f.session().track_by_name("Vox").unwrap().mixer.slots.tape.clone().unwrap();
        assert_eq!(back, e.session().track(t).unwrap().mixer.slots.tape.clone().unwrap());
        assert_eq!((back.id, back.bypass, back.trim_db), (id, true, -2.0));
        e.execute("mix.slot", &json!({"track": t.0, "slot": "tape", "plugin": null})).unwrap();
        let s = e.session().track(t).unwrap().mixer.slots.tape.clone().unwrap();
        assert_eq!((s.id, s.provider, s.trim_db), (id, None, 0.0), "same slot id; trim reset with the removal");
        e.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(e.session().track(t).unwrap().mixer.slots.tape.as_ref().unwrap().provider, Some(Insert::new("test:gain:-3")));
        e.transport.playing = true;
        assert!(e.execute("mix.slot", &json!({"track": t.0, "slot": "tape", "plugin": ""})).unwrap_err().to_string().contains("stop playback"));
        e.execute("mix.slot", &json!({"track": t.0, "slot": "tape", "trim_db": -1.0})).unwrap();
        e.transport.playing = false;
        // Main Master Tape defaults to Pre; bus Master Tape to Post.
        let master = e.session().master().unwrap().id;
        e.execute("mix.slot", &json!({"track": master.0, "slot": "master_tape", "plugin": ""})).unwrap();
        assert_eq!(e.session().track(master).unwrap().mixer.slots.master_tape_position, soundcraft_model::TapePosition::Pre);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SL05: audition overrides are not saved and never reach exports; latency report includes
    /// slot latency.
    #[test]
    fn audition_is_not_saved_and_latency_is_reported() {
        let (mut e, t) = one_track();
        e.execute("mix.slot", &json!({"track": t.0, "slot": "tape", "plugin": "test:delay:64"})).unwrap();
        let r = e.execute("mix.latency_report", &json!({})).unwrap();
        assert_eq!(r["worst_samples"], 64);
        e.execute("mix.audition_bypass", &json!({"all": true})).unwrap();
        let json = serde_json::to_string(e.session()).unwrap();
        let back: soundcraft_model::Session = serde_json::from_str(&json).unwrap();
        assert!(!back.audition.any(), "reopen resets audition overrides");
        let flow = e.execute("mix.signal_flow", &json!({"track": t.0})).unwrap();
        assert!(flow["order"].to_string().contains("Tape [test:delay:64]"));
    }

    /// V13: the four named paths differ exactly as declared (Edited Dry excludes the fader and
    /// Tape; Processed includes them; Full Mix includes Main; Raw is the untouched media).
    #[test]
    fn v13_named_export_paths() {
        let (mut e, t) = one_track();
        e.execute("mix.volume", &json!({"tracks": [t.0], "db": -6.0})).unwrap();
        e.execute("mix.slot", &json!({"track": t.0, "slot": "tape", "plugin": "test:gain:-6"})).unwrap();
        let master = e.session().master().unwrap().id;
        e.execute("mix.volume", &json!({"tracks": [master.0], "db": -6.0})).unwrap();
        let dir = std::env::temp_dir().join(format!("sml-v13-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut peak = |kind: &str| -> f32 {
            let p = dir.join(format!("{kind}.wav"));
            e.execute("file.export_path", &json!({"path": p.to_string_lossy(), "kind": kind, "track": t.0, "start": 0, "end": 4800})).unwrap();
            let (_, d) = soundcraft_audio_io::decode(&std::fs::read(&p).unwrap(), Some("wav")).unwrap();
            d.channels[0][100].abs()
        };
        let g6 = 10f32.powf(-6.0 / 20.0);
        let raw = peak("raw");
        let dry = peak("edited_dry");
        let processed = peak("processed");
        let full = peak("full_mix");
        let pan = soundcraft_dsp::pan::gains(0.0, soundcraft_dsp::pan::PanLaw::Minus3).0;
        assert!((raw - 0.5).abs() < 1e-6, "raw = media");
        assert!((dry - 0.5 * g6).abs() < 1e-5, "edited dry = clip gain only: {dry}");
        assert!((processed - 0.5 * g6 * g6 * g6 * pan).abs() < 1e-5, "processed = clip gain, tape, fader, pan: {processed}");
        assert!((full - processed * g6).abs() < 1e-5, "full mix adds the master fader: {full}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
