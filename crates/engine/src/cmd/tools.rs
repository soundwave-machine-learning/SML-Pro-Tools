//! Audio-editing tool commands (EDIT_TOOLBAR_SPEC T02–T07, T10): the toolbar's Split, Eraser,
//! Mute and Paint gestures and the Link/alternate-tool settings. Every tool dispatches one of
//! these shared commands; a sweep collects clip ids on the UI thread and commits them here in
//! one command, so one gesture is one undo record. Locks reject a compound edit atomically.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::{ClipId, Tool};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "edit.split_at",
            "Split at Pointer",
            [],
            None,
            "{at: position, clips?: [id]} — splits the named clips (default: the selected clips, else the clips under `at` on the selected tracks) that contain `at`; zero-length pieces and locked clips reject the whole command",
            has_selection,
            split_at
        ),
        cmd!(
            "edit.erase_clips",
            "Erase Clips",
            [],
            None,
            "{clips: [id]} — removes clip instances (media files are kept); ripples only in Shuffle mode; a locked clip rejects the whole sweep",
            has_selection,
            erase_clips
        ),
        cmd!(
            "clip.gain_draw",
            "Draw Clip Gain",
            [],
            None,
            "{clip, shape: freehand|line|parabola|square|triangle|saw|sine|transform, start?, end?, from?, to?, curve?, baseline?, amplitude?, cycles?, phase?, density_ms?, points?, box_start?, box_end?, time_shift?, time_scale?, value_offset?, value_scale?} — like automation.draw but into the clip-gain envelope (dB, −144..+36) of one clip; the mixer automation lane is never touched",
            has_selection,
            gain_draw
        ),
        cmd!(noundo "edit.tool_link", "Link Arrow/Range", [], None, "{value?: bool} — with Arrow active, the upper half of a tall track selects ranges", always, |e, p| {
            let cur = e.session().edit.tool_link;
            let v = bool_or(p, "value", !cur);
            e.session_mut().edit.tool_link = v;
            Ok(json!({"value": v}))
        }),
        cmd!(noundo "edit.alt_tool", "Alternate Tool", [], None, "{tool?: id | null} — the tool reached by holding the alternate modifier (Ctrl, or Cmd on macOS) while Arrow is active", always, |e, p| {
            let t = match p.get("tool") {
                None | Some(Value::Null) => None,
                Some(v) => Some(v.as_str().and_then(Tool::from_id).filter(|t| *t != Tool::Arrow).ok_or_else(|| bad("edit.alt_tool", "unknown tool (Arrow cannot be its own alternate)"))?),
            };
            e.session_mut().edit.alt_tool = t;
            Ok(json!({"tool": t.map(Tool::id)}))
        }),
        cmd!(noundo "edit.paint_shape", "Paint Shape", [], None, "{shape: freehand|line|parabola|square|triangle|saw|sine|transform, cycles?: 1}", always, |e, p| {
            let shape = str_param(p, "shape").and_then(super::paint::Shape::parse).ok_or_else(|| bad("edit.paint_shape", format!("`shape` must be one of {}", super::paint::Shape::ALL.join("|"))))?;
            let cycles = f64_or(p, "cycles", e.session().edit.paint_cycles);
            if !cycles.is_finite() || cycles <= 0.0 || cycles > 1_000_000.0 {
                return Err(bad("edit.paint_shape", "`cycles` must be positive"));
            }
            let s = e.session_mut();
            s.edit.paint_shape = shape.id().to_string();
            s.edit.paint_cycles = cycles;
            Ok(json!({"shape": shape.id(), "cycles": cycles}))
        }),
    ]
}

/// The clips a tool gesture targets when the command names none: the selected clips, else
/// the clips under `at` on the selected tracks.
fn targets(e: &Engine, p: &Value, at: Option<Samples>) -> Vec<ClipId> {
    if p.get("clips").is_some() || p.get("clip").is_some() {
        return clip_ids_param(e, p);
    }
    let s = e.session();
    if !s.edit.selected_clips.is_empty() {
        return s.edit.selected_clips.clone();
    }
    let Some(at) = at else { return clip_ids_param(e, p) };
    s.tracks
        .iter()
        .filter(|t| s.edit.selected_tracks.contains(&t.id))
        .flat_map(|t| t.clips().iter().filter(move |c| c.start < at && at < c.end()).map(|c| c.id))
        .collect()
}

fn split_at(e: &mut Engine, p: &Value) -> Result<Value> {
    let cmd = "edit.split_at";
    let at = position_param(e, cmd, p, "at")?.ok_or_else(|| bad(cmd, "`at` required"))?;
    let ids = targets(e, p, Some(at));
    // Validate every target before touching any: eligible means strictly containing `at`.
    let mut eligible = Vec::new();
    for id in &ids {
        let s = e.session();
        let (t, c) = s.find_clip(*id).ok_or_else(|| bad(cmd, format!("clip {} not found", id.0)))?;
        if c.edit_locked || c.time_locked {
            return Err(bad(cmd, format!("clip `{}` is locked", c.name)));
        }
        if c.start < at && at < c.end() {
            eligible.push((t, *id));
        }
    }
    if eligible.is_empty() {
        return Err(bad(cmd, "no clip contains the split time (zero-length pieces are not created)"));
    }
    let s = e.session_mut();
    let tempo = s.tempo.clone();
    let ctx = crate::edit::TimeCtx { tempo: &tempo, sr: s.sample_rate };
    let mut made = Vec::new();
    for (t, id) in eligible {
        let new_id = s.new_clip_id();
        let Some(pl) = s.track_mut(t).and_then(|tr| tr.playlist_mut()) else { continue };
        let Some(i) = pl.clips.iter().position(|c| c.id == id) else { continue };
        if let Some((l, r)) = crate::edit::split_clip(&pl.clips[i], at, new_id, ctx) {
            pl.clips[i] = l;
            pl.clips.insert(i + 1, r);
            pl.sort();
            made.push(json!({"left": id.0, "right": new_id.0}));
        }
    }
    Ok(json!({"at": at, "split": made}))
}

fn erase_clips(e: &mut Engine, p: &Value) -> Result<Value> {
    let cmd = "edit.erase_clips";
    let ids = clip_ids_param(e, p);
    if ids.is_empty() {
        return Err(bad(cmd, "`clips` required"));
    }
    let mut found = Vec::new();
    for id in &ids {
        let s = e.session();
        let (t, c) = s.find_clip(*id).ok_or_else(|| bad(cmd, format!("clip {} not found", id.0)))?;
        if c.edit_locked || c.time_locked {
            return Err(bad(cmd, format!("clip `{}` is locked", c.name)));
        }
        found.push((t, *id, c.range()));
    }
    let shuffle = e.session().edit.edit_mode == soundcraft_model::EditMode::Shuffle;
    let s = e.session_mut();
    let mut removed = 0;
    // Later clips first, so a Shuffle ripple cannot move a clip still waiting to be erased.
    found.sort_by_key(|(_, _, r)| std::cmp::Reverse(r.start));
    for (t, id, r) in found {
        if shuffle {
            removed += crate::edit::clear_range(s, t, r, true).len();
        } else if let Some(pl) = s.track_mut(t).and_then(|tr| tr.playlist_mut()) {
            let before = pl.clips.len();
            pl.clips.retain(|c| c.id != id);
            removed += before - pl.clips.len();
        }
    }
    s.edit.selected_clips.retain(|c| !ids.contains(c));
    Ok(json!({"removed": removed, "rippled": shuffle}))
}

/// Paint into one clip's gain envelope. Times are timeline samples (default: the clip's range);
/// the envelope stores offsets from the clip start.
fn gain_draw(e: &mut Engine, p: &Value) -> Result<Value> {
    let cmd = "clip.gain_draw";
    let id = p
        .get("clip")
        .and_then(Value::as_u64)
        .map(ClipId)
        .or_else(|| e.session().edit.selected_clips.first().copied())
        .ok_or_else(|| bad(cmd, "`clip` required"))?;
    let shape = str_param(p, "shape")
        .and_then(super::paint::Shape::parse)
        .ok_or_else(|| bad(cmd, format!("`shape` must be one of {}", super::paint::Shape::ALL.join("|"))))?;
    let (clip_start, clip_end, name) = {
        let s = e.session();
        let (_, c) = s.find_clip(id).ok_or_else(|| bad(cmd, format!("clip {} not found", id.0)))?;
        if !c.is_audio() {
            return Err(bad(cmd, "clip gain applies to audio clips only"));
        }
        if c.edit_locked {
            return Err(bad(cmd, format!("clip `{}` is locked", c.name)));
        }
        (c.start, c.end(), c.name.clone())
    };
    let start = position_param(e, cmd, p, "start")?.unwrap_or(clip_start).clamp(clip_start, clip_end);
    let end = position_param(e, cmd, p, "end")?.unwrap_or(clip_end).clamp(clip_start, clip_end);
    let finite = |key: &str, d: f64| -> Result<f64> {
        match p.get(key) {
            None | Some(Value::Null) => Ok(d),
            Some(v) => v.as_f64().filter(|x| x.is_finite()).ok_or_else(|| bad(cmd, format!("`{key}` must be a finite number"))),
        }
    };
    let sp = super::paint::ShapeParams {
        from: finite("from", 0.0)?,
        to: finite("to", 0.0)?,
        curve: finite("curve", 1.0)?,
        baseline: finite("baseline", 0.0)?,
        amplitude: finite("amplitude", 6.0)?,
        cycles: finite("cycles", 1.0)?,
        phase: finite("phase", 0.0)?,
        density_ms: finite("density_ms", super::paint::DEFAULT_DENSITY_MS)?,
    };
    if sp.cycles <= 0.0 || sp.cycles > 1_000_000.0 {
        return Err(bad(cmd, "`cycles` must be positive"));
    }
    let clamp = |v: f64| (v as f32).clamp(-144.0, 36.0);
    let sr = e.session().sample_rate.as_f64();
    let rel = |at: Samples| at - clip_start;
    let written = match shape {
        super::paint::Shape::Transform => {
            let box_start = rel(position_param(e, cmd, p, "box_start")?.unwrap_or(start));
            let box_end = rel(position_param(e, cmd, p, "box_end")?.unwrap_or(end));
            if box_end <= box_start {
                return Err(bad(cmd, "`box_end` must be after `box_start`"));
            }
            let time_shift = finite("time_shift", 0.0)?;
            let time_scale = finite("time_scale", 1.0)?;
            let value_offset = finite("value_offset", 0.0)?;
            let value_scale = finite("value_scale", 1.0)?;
            if time_scale <= 0.0 || time_scale > 1000.0 {
                return Err(bad(cmd, "`time_scale` must be in (0, 1000]"));
            }
            let s = e.session_mut();
            let Some(c) = s.find_clip_mut(id) else { return Err(bad(cmd, "clip vanished")) };
            let inside: Vec<(Samples, f32)> = c.gain_env.iter().filter(|(o, _)| *o >= box_start && *o <= box_end).copied().collect();
            c.gain_env.retain(|(o, _)| *o < box_start || *o > box_end);
            let mut n = 0;
            for (o, v) in inside {
                let no = box_start + (((o - box_start) as f64) * time_scale + time_shift).round() as Samples;
                if no < box_start || no > box_end {
                    continue;
                }
                c.gain_env.push((no, clamp(f64::from(v) * value_scale + value_offset)));
                n += 1;
            }
            c.gain_env.sort_by_key(|(o, _)| *o);
            c.gain_env.dedup_by_key(|(o, _)| *o);
            n
        }
        super::paint::Shape::Freehand => {
            if start >= end {
                return Err(bad(cmd, "a non-empty range (start < end) is required"));
            }
            let pts: Vec<(Samples, f64)> = p
                .get("points")
                .and_then(Value::as_array)
                .ok_or_else(|| bad(cmd, "`points` ([[at, value]…]) required for freehand"))?
                .iter()
                .map(|q| {
                    let at = q.get(0).and_then(Value::as_i64).ok_or_else(|| bad(cmd, "point `at` must be an integer sample"))?;
                    let v = q.get(1).and_then(Value::as_f64).filter(|x| x.is_finite()).ok_or_else(|| bad(cmd, "point value must be finite"))?;
                    Ok((at, v))
                })
                .collect::<Result<Vec<_>>>()?;
            if pts.len() > super::paint::MAX_POINTS {
                return Err(bad(cmd, format!("at most {} points", super::paint::MAX_POINTS)));
            }
            let s = e.session_mut();
            let Some(c) = s.find_clip_mut(id) else { return Err(bad(cmd, "clip vanished")) };
            let (a, b) = (rel(start), rel(end));
            c.gain_env.retain(|(o, _)| *o < a || *o > b);
            let mut n = 0;
            for (at, v) in pts.iter().filter(|(at, _)| *at >= start && *at <= end) {
                c.gain_env.push((rel(*at), clamp(*v)));
                n += 1;
            }
            c.gain_env.sort_by_key(|(o, _)| *o);
            c.gain_env.dedup_by_key(|(o, _)| *o);
            n
        }
        _ => {
            if start >= end {
                return Err(bad(cmd, "a non-empty range (start < end) is required"));
            }
            let pts = super::paint::shape_points(shape, start, end, sr, &sp);
            let s = e.session_mut();
            let Some(c) = s.find_clip_mut(id) else { return Err(bad(cmd, "clip vanished")) };
            let (a, b) = (rel(start), rel(end));
            c.gain_env.retain(|(o, _)| *o < a || *o > b);
            for (at, v) in &pts {
                c.gain_env.push((rel(*at), clamp(*v)));
            }
            c.gain_env.sort_by_key(|(o, _)| *o);
            c.gain_env.dedup_by_key(|(o, _)| *o);
            pts.len()
        }
    };
    Ok(json!({"clip": id.0, "name": name, "shape": shape.id(), "points": written}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_audio_io::AudioBuffer;
    use soundcraft_model::{ChannelFormat, Clip, Session, SourceAudio, SourceId, TrackId, TrackKind};
    use std::sync::Arc;

    fn engine() -> (Engine, TrackId, Vec<ClipId>) {
        let mut s = Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
        s.pool.insert(SourceId(1), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![vec![0.5; 96_000]] })));
        let mut ids = Vec::new();
        for (start, len) in [(0, 10_000), (20_000, 10_000), (40_000, 10_000)] {
            let id = s.new_clip_id();
            s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "c", SourceId(1), 0, start, len));
            ids.push(id);
        }
        s.edit.selected_tracks = vec![t];
        (Engine::new(s), t, ids)
    }

    fn clips(e: &Engine, t: TrackId) -> Vec<(i64, i64, bool)> {
        e.session().track(t).unwrap().clips().iter().map(|c| (c.start, c.length, c.muted)).collect()
    }

    /// T04: split only the eligible clip under the pointer; locks and zero-length pieces reject.
    #[test]
    fn split_at_pointer_is_exact_and_atomic() {
        let (mut e, t, ids) = engine();
        e.execute("edit.split_at", &json!({"at": 25_000})).unwrap();
        assert_eq!(clips(&e, t), vec![(0, 10_000, false), (20_000, 5_000, false), (25_000, 5_000, false), (40_000, 10_000, false)]);
        assert!(e.execute("edit.split_at", &json!({"at": 20_000})).is_err(), "a clip boundary makes a zero-length piece");
        assert!(e.execute("edit.split_at", &json!({"at": 15_000})).is_err(), "nothing under the pointer");
        let before = e.session_arc();
        e.session_mut().find_clip_mut(ids[0]).unwrap().edit_locked = true;
        let before2 = e.session_arc();
        assert!(e.execute("edit.split_at", &json!({"at": 5_000, "clips": [ids[0].0, ids[2].0]})).is_err());
        assert!(Arc::ptr_eq(&before2, &e.session_arc()), "locked member: nothing changed");
        drop(before);
        e.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(clips(&e, t).len(), 3);
    }

    /// T05: a sweep removes exactly the swept instances, media stays, one undo restores all.
    #[test]
    fn eraser_sweep_is_one_undo_and_keeps_media() {
        let (mut e, t, ids) = engine();
        let frames = e.session().pool.get(SourceId(1)).unwrap().frames();
        e.execute("edit.erase_clips", &json!({"clips": [ids[0].0, ids[2].0]})).unwrap();
        assert_eq!(clips(&e, t), vec![(20_000, 10_000, false)]);
        assert_eq!(e.session().pool.get(SourceId(1)).map(|a| a.frames()), Some(frames), "media kept");
        e.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(clips(&e, t).len(), 3);
        // Shuffle ripples; Slip does not.
        e.execute("edit.mode", &json!({"mode": "shuffle"})).unwrap();
        e.execute("edit.erase_clips", &json!({"clips": [ids[0].0]})).unwrap();
        assert_eq!(clips(&e, t)[0].0, 10_000, "shuffle moved the next clip left by the erased length");
        e.execute("edit.undo", &json!({})).unwrap();
        e.session_mut().find_clip_mut(ids[1]).unwrap().time_locked = true;
        assert!(e.execute("edit.erase_clips", &json!({"clips": [ids[0].0, ids[1].0]})).is_err());
        assert_eq!(clips(&e, t).len(), 3, "locked member: whole sweep rejected");
    }

    /// T07: explicit target state (a sweep never toggles twice) and undo.
    #[test]
    fn mute_sweep_uses_an_explicit_state() {
        let (mut e, t, ids) = engine();
        e.execute("edit.mute_clips", &json!({"clips": [ids[0].0, ids[1].0], "muted": true})).unwrap();
        e.execute("edit.mute_clips", &json!({"clips": [ids[0].0, ids[1].0], "muted": true})).unwrap();
        assert_eq!(clips(&e, t).iter().map(|c| c.2).collect::<Vec<_>>(), vec![true, true, false]);
        e.execute("edit.mute_clips", &json!({"clips": [ids[0].0], "muted": false})).unwrap();
        assert_eq!(clips(&e, t).iter().map(|c| c.2).collect::<Vec<_>>(), vec![false, true, false]);
        e.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(clips(&e, t).iter().map(|c| c.2).collect::<Vec<_>>(), vec![true, true, false]);
    }

    /// T06: Paint into the clip-gain envelope never touches mixer automation, and vice versa.
    #[test]
    fn gain_draw_stays_in_its_domain() {
        let (mut e, t, ids) = engine();
        e.execute("clip.gain_draw", &json!({"clip": ids[1].0, "shape": "line", "from": -12.0, "to": 0.0})).unwrap();
        let c = e.session().find_clip(ids[1]).unwrap().1.clone();
        assert_eq!(c.gain_env.first().map(|p| p.0), Some(0));
        assert_eq!(c.gain_env.last().map(|p| p.0), Some(10_000));
        assert!((c.gain_env.first().unwrap().1 + 12.0).abs() < 1e-6);
        assert!(e.session().track(t).unwrap().automation.is_empty(), "no mixer lane written");
        // A partial range keeps the outside points.
        e.execute("clip.gain_draw", &json!({"clip": ids[1].0, "shape": "line", "start": 22_000, "end": 24_000, "from": 6.0, "to": 6.0})).unwrap();
        let c = e.session().find_clip(ids[1]).unwrap().1.clone();
        assert!(c.gain_env.iter().any(|(o, _)| *o == 0) && c.gain_env.iter().any(|(o, _)| *o == 10_000));
        assert!(c.gain_env.iter().filter(|(o, _)| (2_000..=4_000).contains(o)).all(|(_, v)| (*v - 6.0).abs() < 1e-6));
        // Transform shifts only the box; out-of-box points survive; clamping and rejection.
        e.execute("clip.gain_draw", &json!({"clip": ids[1].0, "shape": "transform", "box_start": 22_000, "box_end": 24_000, "value_offset": 100.0}))
            .unwrap();
        let c = e.session().find_clip(ids[1]).unwrap().1.clone();
        assert!(c.gain_env.iter().filter(|(o, _)| (2_000..=4_000).contains(o)).all(|(_, v)| *v == 36.0), "clamped at +36 dB");
        assert!(e.execute("clip.gain_draw", &json!({"clip": ids[1].0, "shape": "line", "from": "nan"})).is_err());
        // automation.draw on the same track leaves the clip envelope alone.
        e.execute(
            "automation.draw",
            &json!({"tracks": [t.0], "param": "volume", "shape": "line", "start": 0, "end": 50_000, "from": -3.0, "to": -3.0}),
        )
        .unwrap();
        assert_eq!(e.session().find_clip(ids[1]).unwrap().1.gain_env, c.gain_env);
    }

    #[test]
    fn tool_settings_round_trip() {
        let (mut e, _, _) = engine();
        for t in Tool::AUDIO {
            e.execute("edit.tool", &json!({"tool": t.id()})).unwrap();
            assert_eq!(e.session().edit.tool, t);
        }
        e.execute("edit.tool_link", &json!({})).unwrap();
        assert!(!e.session().edit.tool_link);
        e.execute("edit.alt_tool", &json!({"tool": "range"})).unwrap();
        assert_eq!(e.session().edit.alt_tool, Some(Tool::Range));
        assert!(e.execute("edit.alt_tool", &json!({"tool": "arrow"})).is_err());
        e.execute("edit.alt_tool", &json!({"tool": null})).unwrap();
        assert_eq!(e.session().edit.alt_tool, None);
        e.execute("edit.paint_shape", &json!({"shape": "sine", "cycles": 3})).unwrap();
        assert_eq!(e.session().edit.paint_shape, "sine");
        assert!(e.execute("edit.paint_shape", &json!({"shape": "blob"})).is_err());
        let json = serde_json::to_string(e.session()).unwrap();
        let back: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(back.edit.paint_shape, "sine");
        assert_eq!(back.edit.paint_cycles, 3.0);
    }
}
