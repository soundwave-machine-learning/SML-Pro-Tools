//! A/B/C mix snapshots (VISUAL_MIXER_SPEC VM05) and native Stereo Width (VM03).
//!
//! Snapshot scope: per stable channel id — fader gain, panner coordinates, stereo width, mute,
//! solo and view visibility. Excluded (and labelled excluded): plugin states, slot assignments,
//! routing, trims and automation envelopes. Recall is one atomic undoable batch over the ids
//! that still exist; missing ids are reported, new channels are left unchanged, nothing is
//! recreated or rerouted. Snapshots live in the session, so they survive save/reopen.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::{MixSnapshot, SnapshotChannel};

pub const SCOPE: &str = "gain, pan, width, mute, solo, visibility";
pub const EXCLUDED: &str = "plugins, slots, routing, trims, automation";

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "mix.view", "Mixer View", [], None, "{view: 0 console | 1 visual | 2 split}", always, |e, p| {
            let v = f64_or(p, "view", 0.0).round().clamp(0.0, 2.0);
            e.session_mut().edit.values.insert("mixer.view".into(), v);
            Ok(json!({"view": v}))
        }),
        cmd!("mix.width", "Stereo Width", [], None, "{tracks?, width: 0..2 (1 = 100 %)}", has_selection, width),
        cmd!("snapshot.save", "Save Mix Snapshot", ["Mix"], None, "{slot: A|B|C}", always, save),
        cmd!("snapshot.recall", "Recall Mix Snapshot", ["Mix"], None, "{slot: A|B|C}", always, recall),
        cmd!(query "snapshot.info", "Mix Snapshots", [], None, "{}", always, |e, _| {
            let s = e.session();
            let v: Vec<Value> = s
                .mix_snapshots
                .iter()
                .map(|(k, snap)| json!({"slot": k, "channels": snap.channels.len()}))
                .collect();
            Ok(json!({"snapshots": v, "scope": SCOPE, "excluded": EXCLUDED}))
        }),
    ]
}

fn slot(cmd: &str, p: &Value) -> Result<String> {
    let s = str_param(p, "slot").unwrap_or("").to_ascii_uppercase();
    if matches!(s.as_str(), "A" | "B" | "C") { Ok(s) } else { Err(bad(cmd, "slot must be A, B or C")) }
}

fn width(e: &mut Engine, p: &Value) -> Result<Value> {
    let w = p.get("width").and_then(Value::as_f64).filter(|v| v.is_finite()).ok_or_else(|| bad("mix.width", "`width` (0..2) required"))?;
    let tracks = tracks_required(e, "mix.width", p)?;
    let s = e.session_mut();
    let mut n = 0;
    for t in tracks {
        if let Some(tr) = s.track_mut(t) {
            if tr.channels() < 2 {
                // Width is unavailable for a truly mono signal (no fabricated widening).
                continue;
            }
            tr.mixer.width = (w as f32).clamp(0.0, 2.0);
            n += 1;
        }
    }
    Ok(json!({"tracks": n, "width": w.clamp(0.0, 2.0)}))
}

fn save(e: &mut Engine, p: &Value) -> Result<Value> {
    let k = slot("snapshot.save", p)?;
    let s = e.session_mut();
    let channels = s
        .tracks
        .iter()
        .filter(|t| t.kind != soundcraft_model::TrackKind::Master)
        .map(|t| SnapshotChannel {
            track: t.id,
            volume_db: t.mixer.volume_db,
            pan: t.mixer.pan.clone(),
            width: t.mixer.width,
            mute: t.mixer.mute,
            solo: t.mixer.solo,
            hidden: t.hidden,
        })
        .collect::<Vec<_>>();
    let n = channels.len();
    s.mix_snapshots.insert(k.clone(), MixSnapshot { channels });
    Ok(json!({"slot": k, "channels": n, "scope": SCOPE, "excluded": EXCLUDED}))
}

fn recall(e: &mut Engine, p: &Value) -> Result<Value> {
    let k = slot("snapshot.recall", p)?;
    if e.transport.playing {
        return Err(bad("snapshot.recall", "stop playback to recall a snapshot (it may change panner/format settings)"));
    }
    let snap = e.session().mix_snapshots.get(&k).cloned().ok_or_else(|| bad("snapshot.recall", format!("snapshot {k} is empty")))?;
    let s = e.session_mut();
    let mut missing = Vec::new();
    let mut applied = 0;
    for c in &snap.channels {
        match s.track_mut(c.track) {
            Some(t) => {
                t.mixer.volume_db = c.volume_db;
                if t.mixer.pan.len() == c.pan.len() {
                    t.mixer.pan.clone_from(&c.pan);
                }
                t.mixer.width = c.width;
                t.mixer.mute = c.mute;
                t.mixer.solo = c.solo;
                t.hidden = c.hidden;
                applied += 1;
            }
            None => missing.push(c.track.0),
        }
    }
    Ok(json!({"slot": k, "applied": applied, "missing": missing, "excluded": EXCLUDED}))
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use serde_json::json;
    use soundcraft_model::{ChannelFormat, Insert, TrackKind};

    /// VM-T5: A/B/C round trip, scoped recall (plugins/routing untouched), missing/new ids,
    /// undo of a recall, persistence.
    #[test]
    fn vmt5_snapshots_round_trip_and_scope() {
        let mut e = Engine::default();
        let a = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Stereo, Some("Lead"));
        let b = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Dbl"));
        e.execute("mix.volume", &json!({"tracks": [a.0], "db": -3.0})).unwrap();
        e.execute("mix.width", &json!({"tracks": [a.0], "width": 1.5})).unwrap();
        e.execute("snapshot.save", &json!({"slot": "A"})).unwrap();
        e.execute("mix.volume", &json!({"tracks": [a.0], "db": -10.0})).unwrap();
        e.execute("mix.width", &json!({"tracks": [a.0], "width": 0.2})).unwrap();
        e.session_mut().track_mut(a).unwrap().mixer.inserts[0] = Some(Insert::new("vst3:owner.eq"));
        let c = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, Some("New"));
        e.execute("mix.volume", &json!({"tracks": [c.0], "db": -7.0})).unwrap();
        e.session_mut().tracks.retain(|t| t.id != b);
        let r = e.execute("snapshot.recall", &json!({"slot": "a"})).unwrap();
        assert_eq!(r["missing"], json!([b.0]));
        let t = e.session().track(a).unwrap();
        assert_eq!((t.mixer.volume_db, t.mixer.width), (-3.0, 1.5));
        assert!(t.mixer.inserts[0].is_some(), "plugins are outside the snapshot scope");
        assert_eq!(e.session().track(c).unwrap().mixer.volume_db, -7.0, "new channel unchanged");
        e.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(e.session().track(a).unwrap().mixer.volume_db, -10.0, "recall is one undo step");
        let back: soundcraft_model::Session = serde_json::from_str(&serde_json::to_string(e.session()).unwrap()).unwrap();
        assert_eq!(back.mix_snapshots, e.session().mix_snapshots);
        assert!(e.execute("snapshot.recall", &json!({"slot": "C"})).is_err());
    }

    #[test]
    fn width_is_unavailable_on_mono() {
        let mut e = Engine::default();
        let m = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        let r = e.execute("mix.width", &json!({"tracks": [m.0], "width": 2.0})).unwrap();
        assert_eq!(r["tracks"], 0);
        assert_eq!(e.session().track(m).unwrap().mixer.width, 1.0);
    }
}
