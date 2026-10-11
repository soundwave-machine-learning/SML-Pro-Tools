//! Dedicated Nudge (NUDGE_SPEC, R12): explicit targets, typed value/unit separate from the grid,
//! exact fractional accumulation from a logical anchor, tempo-aware musical steps and atomic
//! rejection. One shared engine primitive, distinct command ids per target.
//!
//! Arithmetic contract: for time units the displacement after `k` steps is
//! `round_ties_even(k · value_in_samples)` from the anchor, so ten 1 ms steps at 44.1 kHz move
//! exactly 441 samples and a reversed sequence returns exactly to the anchor. Musical steps map
//! `anchor_tick + k · ticks` through the tempo map. The anchor resets on any direct reposition,
//! target/unit/value, rate or tempo-map change. Invalid compound edits (project start, locked
//! member, missing source handles, zero length, unsupported mapping) reject the whole command
//! with a reason; the engine restores the document so nothing is partially applied.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::{ClipContent, NudgeAnchor, NudgePanel, NudgeTarget, NudgeUnit, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "nudge.set", "Nudge Settings", [], None, "{value?: >0, unit?: samples|ms|s|ticks|frames, target?: clips|contents|trim_start|trim_end|selection|selection_start|selection_end}", always, set),
        cmd!(query "nudge.info", "Nudge Info", [], None, "{}", always, |e, _| Ok(info(e.session()))),
        cmd!("nudge.apply", "Nudge", [], None, "{direction: 1|-1, target?: <target id>, count?: n, clips?}", always, |e, p| apply(e, p, None)),
        cmd!("nudge.clips", "Nudge Clips", ["Edit", "Nudge"], None, "{direction: 1|-1, count?}", always, |e, p| apply(
            e,
            p,
            Some(NudgeTarget::Clips)
        )),
        cmd!("nudge.contents", "Nudge Clip Contents", ["Edit", "Nudge"], None, "{direction: 1|-1, count?}", always, |e, p| apply(
            e,
            p,
            Some(NudgeTarget::ClipContents)
        )),
        cmd!("nudge.trim_start", "Nudge Trim Start", ["Edit", "Nudge"], None, "{direction: 1|-1, count?}", always, |e, p| apply(
            e,
            p,
            Some(NudgeTarget::TrimStart)
        )),
        cmd!("nudge.trim_end", "Nudge Trim End", ["Edit", "Nudge"], None, "{direction: 1|-1, count?}", always, |e, p| apply(
            e,
            p,
            Some(NudgeTarget::TrimEnd)
        )),
        cmd!("nudge.selection", "Nudge Selection Only", ["Edit", "Nudge"], None, "{direction: 1|-1, count?}", always, |e, p| apply(
            e,
            p,
            Some(NudgeTarget::Selection)
        )),
        cmd!("nudge.selection_start", "Nudge Selection Start", ["Edit", "Nudge"], None, "{direction: 1|-1, count?}", always, |e, p| apply(
            e,
            p,
            Some(NudgeTarget::SelectionStart)
        )),
        cmd!("nudge.selection_end", "Nudge Selection End", ["Edit", "Nudge"], None, "{direction: 1|-1, count?}", always, |e, p| apply(
            e,
            p,
            Some(NudgeTarget::SelectionEnd)
        )),
    ]
}

/// Exact displacement in samples (f64, unrounded) for one step of `value` `unit` — time units
/// only; ticks are tempo-dependent and handled separately.
pub fn step_exact(panel: &NudgePanel, s: &Session) -> Option<f64> {
    let sr = s.sample_rate.as_f64();
    Some(match panel.unit {
        NudgeUnit::Samples => panel.value.round(),
        NudgeUnit::Milliseconds => panel.value * sr / 1000.0,
        NudgeUnit::Seconds => panel.value * sr,
        NudgeUnit::Frames => panel.value * s.frame_rate.samples_per_frame(s.sample_rate),
        NudgeUnit::Ticks => return None,
    })
}

fn info(s: &Session) -> Value {
    let p = &s.edit.nudge_panel;
    let samples = step_exact(p, s);
    json!({
        "value": p.value,
        "unit": p.unit.label(),
        "target": p.target.id(),
        "target_label": p.target.label(),
        "step_samples_exact": samples,
        "step_ms": samples.map(|x| x * 1000.0 / s.sample_rate.as_f64()),
        "sample_rate": s.sample_rate.hz(),
        "accumulated_steps": p.anchor.as_ref().map_or(0, |a| a.steps),
        "applied_samples": p.anchor.as_ref().map_or(0, |a| a.applied),
    })
}

fn set(e: &mut Engine, p: &Value) -> Result<Value> {
    let s = e.session_mut();
    let mut panel = s.edit.nudge_panel.clone();
    if let Some(v) = p.get("value").and_then(Value::as_f64) {
        if !(v.is_finite() && v > 0.0 && v <= 1.0e9) {
            return Err(bad("nudge.set", "value must be a positive number"));
        }
        panel.value = v;
    }
    if let Some(u) = str_param(p, "unit") {
        panel.unit = NudgeUnit::parse(u).ok_or_else(|| bad("nudge.set", format!("unknown unit `{u}`")))?;
    }
    if let Some(t) = str_param(p, "target") {
        panel.target = NudgeTarget::parse(t).ok_or_else(|| bad("nudge.set", format!("unknown target `{t}`")))?;
    }
    if let Some(g) = p.get("gain_step_db").and_then(Value::as_f64) {
        panel.gain_step_db = g.clamp(0.1, 12.0);
    }
    // Changing value/unit/target resets the accumulation (documented rule).
    panel.anchor = None;
    s.edit.nudge_panel = panel;
    Ok(info(s))
}

/// Fingerprint of everything a sample conversion depends on.
fn context(s: &Session) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.sample_rate.hz().hash(&mut h);
    serde_json::to_string(&s.tempo).unwrap_or_default().hash(&mut h);
    serde_json::to_string(&s.frame_rate).unwrap_or_default().hash(&mut h);
    h.finish()
}

/// Ties-to-even rounding to integer samples.
fn round_even(x: f64) -> i64 {
    let r = x.round_ties_even();
    if r.is_finite() && r.abs() < 9.0e15 { r as i64 } else { 0 }
}

/// Displacement (from the anchor) after `k` steps.
fn displacement(panel: &NudgePanel, s: &Session, anchor: i64, k: i64) -> std::result::Result<i64, String> {
    match step_exact(panel, s) {
        Some(step) => Ok(round_even(step * k as f64)),
        None => {
            let ticks = panel.value.round() as i64;
            if ticks <= 0 {
                return Err("musical nudge needs at least one tick".into());
            }
            let t0 = s.tempo.samples_to_ticks_f(anchor, s.sample_rate);
            if !t0.is_finite() {
                return Err("unsupported tempo mapping".into());
            }
            // Exact tick position of the anchor, then whole-tick steps through the tempo map.
            let base = t0.floor() as i64;
            let frac = s.tempo.tick_to_samples(base, s.sample_rate);
            let target = s.tempo.tick_to_samples(base.saturating_add(ticks.saturating_mul(k)), s.sample_rate);
            Ok(target - frac)
        }
    }
}

/// Clip ids a clip-target nudge moves: explicit/selected clips plus every clip sharing a group.
fn target_clips(e: &Engine, p: &Value) -> Vec<ClipId> {
    let mut ids = if p.get("clips").is_some() || p.get("clip").is_some() { clip_ids_param(e, p) } else { e.session().edit.selected_clips.clone() };
    let s = e.session();
    let groups: Vec<u64> = ids.iter().filter_map(|id| s.find_clip(*id).and_then(|(_, c)| c.group)).collect();
    if !groups.is_empty() {
        for t in &s.tracks {
            for c in t.clips() {
                if c.group.is_some_and(|g| groups.contains(&g)) && !ids.contains(&c.id) {
                    ids.push(c.id);
                }
            }
        }
    }
    ids.sort_by_key(|c| c.0);
    ids.dedup();
    ids
}

fn apply(e: &mut Engine, p: &Value, forced: Option<NudgeTarget>) -> Result<Value> {
    let cmd = "nudge.apply";
    let dir = i64_or(p, "direction", 1).signum();
    if dir == 0 {
        return Err(bad(cmd, "direction must be 1 or -1"));
    }
    let count = i64_or(p, "count", 1).clamp(1, 10_000);
    let mut panel = e.session().edit.nudge_panel.clone();
    let target = match (forced, str_param(p, "target")) {
        (Some(t), _) => t,
        (None, Some(t)) => NudgeTarget::parse(t).ok_or_else(|| bad(cmd, format!("unknown target `{t}`")))?,
        (None, None) => panel.target,
    };
    let clip_ids = target_clips(e, p);
    // "Selected clips" with nothing selected moves the cursor/selection instead.
    let target = if target == NudgeTarget::Clips && clip_ids.is_empty() { NudgeTarget::Selection } else { target };
    let is_clip_target = matches!(target, NudgeTarget::Clips | NudgeTarget::ClipContents | NudgeTarget::TrimStart | NudgeTarget::TrimEnd);
    if is_clip_target && clip_ids.is_empty() {
        return Err(bad(cmd, "select one or more clips first"));
    }
    let s = e.session();
    let ctx = context(s);
    let ids: Vec<u64> = if is_clip_target { clip_ids.iter().map(|c| c.0).collect() } else { Vec::new() };
    // Current logical position of the target set.
    let current = match target {
        NudgeTarget::Clips | NudgeTarget::TrimStart => clip_ids.iter().filter_map(|id| s.find_clip(*id).map(|(_, c)| c.start)).min().unwrap_or(0),
        NudgeTarget::TrimEnd => clip_ids.iter().filter_map(|id| s.find_clip(*id).map(|(_, c)| c.end())).min().unwrap_or(0),
        NudgeTarget::ClipContents => clip_ids
            .iter()
            .filter_map(|id| s.find_clip(*id).and_then(|(_, c)| if let ClipContent::Audio { offset, .. } = c.content { Some(-offset) } else { None }))
            .min()
            .unwrap_or(0),
        NudgeTarget::Selection | NudgeTarget::SelectionStart => s.edit.selection.start,
        NudgeTarget::SelectionEnd => s.edit.selection.end,
    };
    // Reuse the anchor only if nothing else moved the target since the last nudge.
    let mut anchor = match panel.anchor.take() {
        Some(a)
            if a.target == target
                && a.unit == panel.unit
                && a.value_bits == panel.value.to_bits()
                && a.ids == ids
                && a.context == ctx
                && a.anchor.saturating_add(a.applied) == current =>
        {
            a
        }
        _ => NudgeAnchor {
            target,
            unit: panel.unit,
            value_bits: panel.value.to_bits(),
            ids: ids.clone(),
            anchor: current,
            steps: 0,
            applied: 0,
            context: ctx,
        },
    };
    let steps = anchor.steps.saturating_add(dir * count);
    let disp = displacement(&panel, s, anchor.anchor, steps).map_err(|m| bad(cmd, m))?;
    let delta = disp - anchor.applied;
    let s = e.session_mut();
    if delta != 0 {
        apply_delta(s, target, &clip_ids, delta).map_err(|m| bad(cmd, m))?;
    }
    anchor.steps = steps;
    anchor.applied = disp;
    panel.anchor = Some(anchor);
    s.edit.nudge_panel = panel;
    Ok(json!({
        "target": target.id(),
        "requested_steps": dir * count,
        "applied_samples": delta,
        "changed": if delta == 0 { Vec::new() } else { ids },
        "accumulated_samples": disp,
        "below_one_sample": delta == 0,
    }))
}

/// Applies an integer delta to the target, validating everything first (no partial edits).
fn apply_delta(s: &mut Session, target: NudgeTarget, ids: &[ClipId], d: i64) -> std::result::Result<(), String> {
    match target {
        NudgeTarget::Selection => {
            let sel = s.edit.selection.shifted(d);
            if sel.start < 0 {
                return Err("the selection would move before the session start".into());
            }
            s.edit.selection = sel;
            return Ok(());
        }
        NudgeTarget::SelectionStart => {
            let (a, b) = (s.edit.selection.start + d, s.edit.selection.end);
            if a < 0 || a >= b {
                return Err("selection start must stay before its end and after the session start".into());
            }
            s.edit.selection.start = a;
            return Ok(());
        }
        NudgeTarget::SelectionEnd => {
            let (a, b) = (s.edit.selection.start, s.edit.selection.end + d);
            if b <= a {
                return Err("selection end must stay after its start".into());
            }
            s.edit.selection.end = b;
            return Ok(());
        }
        _ => {}
    }
    // Validate every member before touching any.
    for id in ids {
        let (_, c) = s.find_clip(*id).ok_or_else(|| format!("clip {} not found", id.0))?;
        if c.edit_locked || c.time_locked {
            return Err(format!("clip `{}` is locked", c.name));
        }
        let src_frames = match &c.content {
            ClipContent::Audio { source, .. } => s.source(*source).map(|x| i64::try_from(x.frames).unwrap_or(i64::MAX)),
            _ => None,
        };
        let offset = match &c.content {
            ClipContent::Audio { offset, .. } | ClipContent::Video { offset, .. } => Some(*offset),
            ClipContent::Midi { .. } => None,
        };
        match target {
            NudgeTarget::Clips => {
                if c.start + d < 0 {
                    return Err(format!("clip `{}` would move before the session start", c.name));
                }
            }
            NudgeTarget::ClipContents => {
                let (Some(off), Some(total)) = (offset, src_frames) else { return Err(format!("clip `{}` has no audio source to slide", c.name)) };
                if (c.stretch - 1.0).abs() > 1e-12 {
                    return Err(format!("clip `{}` is time-stretched; content nudge has no defined mapping for it", c.name));
                }
                let n = off - d;
                if n < 0 || n + c.length > total {
                    return Err(format!("clip `{}` has no source handle left for that move", c.name));
                }
            }
            NudgeTarget::TrimStart => {
                let len = c.length - d;
                if len <= 0 || c.start + d < 0 {
                    return Err(format!("clip `{}` would have no length", c.name));
                }
                if let Some(off) = offset
                    && (off + d < 0 || src_frames.is_some_and(|t| off + d + len > t))
                {
                    return Err(format!("clip `{}` has no source handle left for that trim", c.name));
                }
                if (c.stretch - 1.0).abs() > 1e-12 {
                    return Err(format!("clip `{}` is time-stretched; trim nudge has no defined mapping for it", c.name));
                }
            }
            NudgeTarget::TrimEnd => {
                let len = c.length + d;
                if len <= 0 {
                    return Err(format!("clip `{}` would have no length", c.name));
                }
                if let (Some(off), Some(t)) = (offset, src_frames)
                    && off + len > t
                {
                    return Err(format!("clip `{}` has no source handle left for that trim", c.name));
                }
            }
            _ => {}
        }
    }
    for id in ids {
        let Some(c) = s.find_clip_mut(*id) else { continue };
        match target {
            NudgeTarget::Clips => c.start += d,
            NudgeTarget::ClipContents => {
                if let ClipContent::Audio { offset, .. } = &mut c.content {
                    *offset -= d;
                }
            }
            NudgeTarget::TrimStart => {
                c.start += d;
                c.length -= d;
                match &mut c.content {
                    ClipContent::Audio { offset, .. } | ClipContent::Video { offset, .. } => *offset += d,
                    ClipContent::Midi { .. } => {}
                }
                // Keep the envelope's absolute time reference; nodes outside the window stay.
                for (o, _) in c.gain_env.iter_mut() {
                    *o -= d;
                }
            }
            NudgeTarget::TrimEnd => c.length += d,
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use serde_json::json;
    use soundcraft_audio_io::AudioBuffer;
    use soundcraft_model::{ChannelFormat, Clip, ClipContent, ClipId, TrackKind};
    use std::sync::Arc;

    /// A session at `rate` with one track and clips over a 10 s source (impulse at frame 1000).
    fn engine(rate: u32, clips: &[(i64, i64, i64)]) -> (Engine, Vec<ClipId>) {
        let mut e = Engine::default();
        let s = e.session_mut();
        s.sample_rate = soundcraft_time::SampleRate::new(rate).unwrap();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        let mut data = vec![0.0f32; rate as usize * 10];
        data[1000] = 1.0;
        let src =
            crate::io::add_source(s, "fixture", AudioBuffer { sample_rate: rate, channels: vec![data] }, None, soundcraft_audio_io::FileFormat::Wav);
        let mut ids = Vec::new();
        for (start, offset, len) in clips {
            let id = s.new_clip_id();
            s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "c", src, *offset, *start, *len));
            ids.push(id);
        }
        s.edit.selected_clips = ids.clone();
        (e, ids)
    }

    fn clip(e: &Engine, id: ClipId) -> Clip {
        e.session().find_clip(id).unwrap().1.clone()
    }

    fn set(e: &mut Engine, v: f64, unit: &str, target: &str) {
        e.execute("nudge.set", &json!({"value": v, "unit": unit, "target": target})).unwrap();
    }

    fn nudge(e: &mut Engine, dir: i64) -> serde_json::Value {
        e.execute("nudge.apply", &json!({"direction": dir})).unwrap()
    }

    /// N01: +1 then -1 sample returns the exact edit state; source untouched.
    #[test]
    fn n01_one_sample_round_trip() {
        for rate in [44_100, 48_000, 96_000] {
            let (mut e, ids) = engine(rate, &[(48_000, 100, 1000)]);
            let before = clip(&e, ids[0]);
            let src_id = before.source().unwrap();
            let src_before = Arc::clone(e.session().pool.get(src_id).unwrap());
            set(&mut e, 1.0, "samples", "clips");
            nudge(&mut e, 1);
            assert_eq!(clip(&e, ids[0]).start, 48_001);
            nudge(&mut e, -1);
            assert_eq!(clip(&e, ids[0]), before, "bit-identical edit state");
            assert!(Arc::ptr_eq(e.session().pool.get(src_id).unwrap(), &src_before), "source untouched");
        }
    }

    /// N02: 10 ms = 480 samples at 48 kHz; 1000 × +1 ms then 1000 × -1 ms at 44.1 kHz: forward
    /// endpoint exactly +44 100 and zero drift on return.
    #[test]
    fn n02_fractional_ms_accumulate_without_drift() {
        let (mut e, ids) = engine(48_000, &[(48_000, 0, 1000)]);
        set(&mut e, 10.0, "ms", "clips");
        assert_eq!(nudge(&mut e, 1)["applied_samples"], 480);
        let (mut e, ids2) = engine(44_100, &[(100_000, 0, 1000)]);
        set(&mut e, 1.0, "ms", "clips");
        for _ in 0..10 {
            nudge(&mut e, 1);
        }
        assert_eq!(clip(&e, ids2[0]).start, 100_441, "ten 1 ms steps = 441 samples, not 440");
        for _ in 0..990 {
            nudge(&mut e, 1);
        }
        assert_eq!(clip(&e, ids2[0]).start, 144_100);
        for _ in 0..1000 {
            nudge(&mut e, -1);
        }
        assert_eq!(clip(&e, ids2[0]).start, 100_000, "zero drift");
        let _ = ids;
        // Below one sample: requested displacement shows, no fabricated movement.
        let (mut e, ids3) = engine(48_000, &[(1000, 0, 100)]);
        set(&mut e, 0.01, "ms", "clips");
        let r = nudge(&mut e, 1);
        assert_eq!(r["applied_samples"], 0);
        assert_eq!(r["below_one_sample"], true);
        nudge(&mut e, 1);
        nudge(&mut e, 1); // 0.03 ms = 1.44 samples -> 1
        assert_eq!(clip(&e, ids3[0]).start, 1001);
    }

    /// N03: content slip moves audible content by D with fixed clip bounds; no handle = reject.
    #[test]
    fn n03_content_slip_sign_and_handles() {
        let (mut e, ids) = engine(48_000, &[(10_000, 900, 500)]);
        // Impulse is at source frame 1000 -> timeline 10_000 + (1000 - 900) = 10_100.
        set(&mut e, 10.0, "samples", "contents");
        nudge(&mut e, 1);
        let c = clip(&e, ids[0]);
        assert_eq!((c.start, c.length), (10_000, 500), "bounds fixed");
        let ClipContent::Audio { offset, .. } = c.content else { panic!() };
        assert_eq!(10_000 + (1000 - offset), 10_110, "impulse audible 10 samples later");
        // No handle on the left: offset 890 cannot slide right by 1000.
        set(&mut e, 1000.0, "samples", "contents");
        let before = e.session().clone();
        assert!(e.execute("nudge.apply", &json!({"direction": 1})).is_err());
        assert_eq!(e.session().tracks, before.tracks, "rejected without mutation");
    }

    /// N04: trims and selection boundaries change only their value; zero-length rejected.
    #[test]
    fn n04_trims_and_selection_boundaries() {
        let (mut e, ids) = engine(48_000, &[(10_000, 500, 1000)]);
        set(&mut e, 100.0, "samples", "trim_start");
        nudge(&mut e, 1);
        let c = clip(&e, ids[0]);
        assert_eq!((c.start, c.end(), c.length), (10_100, 11_000, 900));
        let ClipContent::Audio { offset, .. } = c.content else { panic!() };
        assert_eq!(offset, 600, "content stays at the same timeline position");
        set(&mut e, 100.0, "samples", "trim_end");
        nudge(&mut e, -1);
        let c = clip(&e, ids[0]);
        assert_eq!((c.start, c.end()), (10_100, 10_900));
        set(&mut e, 800.0, "samples", "trim_end");
        let before = clip(&e, ids[0]);
        assert!(e.execute("nudge.apply", &json!({"direction": -1})).is_err(), "zero length rejected");
        assert_eq!(clip(&e, ids[0]), before);
        e.session_mut().edit.selection = soundcraft_time::Range::new(1000, 2000);
        set(&mut e, 10.0, "samples", "selection_start");
        nudge(&mut e, 1);
        assert_eq!(e.session().edit.selection, soundcraft_time::Range::new(1010, 2000));
        set(&mut e, 10.0, "samples", "selection_end");
        nudge(&mut e, -1);
        assert_eq!(e.session().edit.selection, soundcraft_time::Range::new(1010, 1990));
        set(&mut e, 2000.0, "samples", "selection_start");
        assert!(e.execute("nudge.apply", &json!({"direction": 1})).is_err(), "inversion rejected");
        set(&mut e, 10.0, "samples", "selection");
        nudge(&mut e, 1);
        assert_eq!(e.session().edit.selection, soundcraft_time::Range::new(1020, 2000));
        assert_eq!(clip(&e, ids[0]).start, 10_100, "selection-only never moves audio");
    }

    /// N05: rigid multi-clip/group move keeps offsets; one locked member rejects the whole move.
    #[test]
    fn n05_rigid_group_and_atomic_rejection() {
        let (mut e, ids) = engine(48_000, &[(1000, 0, 100), (1500, 200, 300), (5000, 10, 50)]);
        e.session_mut().find_clip_mut(ids[2]).unwrap().group = Some(7);
        e.session_mut().find_clip_mut(ids[1]).unwrap().group = Some(7);
        e.session_mut().edit.selected_clips = vec![ids[0], ids[1]];
        set(&mut e, 37.0, "samples", "clips");
        nudge(&mut e, 1);
        let starts: Vec<i64> = ids.iter().map(|i| clip(&e, *i).start).collect();
        assert_eq!(starts, vec![1037, 1537, 5037], "group member followed, offsets kept");
        e.session_mut().find_clip_mut(ids[2]).unwrap().time_locked = true;
        let before = e.session().tracks.clone();
        let err = e.execute("nudge.apply", &json!({"direction": 1})).unwrap_err().to_string();
        assert!(err.contains("locked"), "{err}");
        assert_eq!(e.session().tracks, before);
        e.session_mut().find_clip_mut(ids[2]).unwrap().time_locked = false;
        set(&mut e, 2000.0, "samples", "clips");
        assert!(e.execute("nudge.apply", &json!({"direction": -1})).is_err(), "project start rejects whole set");
        assert_eq!(clip(&e, ids[0]).start, 1037);
    }

    /// N06: edit mode / grid / zoom do not change the nudge delta; musical steps follow the tempo
    /// map across a 120 -> 90 BPM change and round-trip.
    #[test]
    fn n06_modes_do_not_change_delta_and_tempo_changes_are_followed() {
        for mode in ["slip", "grid", "relative_grid", "shuffle"] {
            let (mut e, ids) = engine(48_000, &[(48_000, 0, 1000)]);
            let _ = e.execute("edit.mode", &json!({"mode": mode}));
            e.session_mut().edit.zoom.samples_per_px = 3.7;
            set(&mut e, 10.0, "ms", "clips");
            nudge(&mut e, 1);
            assert_eq!(clip(&e, ids[0]).start, 48_480, "mode {mode}");
        }
        let (mut e, ids) = engine(48_000, &[(0, 0, 1000)]);
        // 120 BPM until bar 2 (tick 4*960), then 90 BPM.
        let tick_change = 4 * 960;
        e.session_mut().tempo.set_tempo(tick_change, 90.0).unwrap();
        let sr = e.session().sample_rate;
        let tempo = e.session().tempo.clone();
        set(&mut e, 960.0, "ticks", "clips");
        for k in 1..=8 {
            nudge(&mut e, 1);
            assert_eq!(clip(&e, ids[0]).start, tempo.tick_to_samples(960 * k, sr), "step {k}");
        }
        // Independent expectation: 4 beats @120 (2 s) + 4 beats @90 (2.666.. s).
        assert_eq!(clip(&e, ids[0]).start, 96_000 + 128_000);
        for _ in 0..8 {
            nudge(&mut e, -1);
        }
        assert_eq!(clip(&e, ids[0]).start, 0);
    }

    /// N08: undo/redo restore positions *and* the accumulation state; save/reopen preserves both.
    #[test]
    fn n08_undo_redo_and_persistence() {
        let (mut e, ids) = engine(44_100, &[(10_000, 0, 1000)]);
        set(&mut e, 1.0, "ms", "clips");
        nudge(&mut e, 1);
        nudge(&mut e, 1);
        nudge(&mut e, 1); // 132.3 -> 132
        assert_eq!(clip(&e, ids[0]).start, 10_132);
        e.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(clip(&e, ids[0]).start, 10_088);
        assert_eq!(e.session().edit.nudge_panel.anchor.as_ref().unwrap().steps, 2, "logical state restored too");
        e.execute("edit.redo", &json!({})).unwrap();
        assert_eq!(clip(&e, ids[0]).start, 10_132);
        let json = serde_json::to_string(e.session()).unwrap();
        let back: soundcraft_model::Session = serde_json::from_str(&json).unwrap();
        assert_eq!(back.edit.nudge_panel, e.session().edit.nudge_panel);
        // A direct reposition resets the anchor (documented rule).
        e.session_mut().find_clip_mut(ids[0]).unwrap().start = 20_000;
        nudge(&mut e, 1);
        assert_eq!(clip(&e, ids[0]).start, 20_044);
    }

    #[test]
    fn rejected_commands_do_not_advance_accumulation() {
        let (mut e, ids) = engine(48_000, &[(10, 0, 1000)]);
        set(&mut e, 1.0, "ms", "clips");
        assert!(e.execute("nudge.apply", &json!({"direction": -1})).is_err());
        assert!(e.session().edit.nudge_panel.anchor.is_none());
        nudge(&mut e, 1);
        assert_eq!(clip(&e, ids[0]).start, 58);
    }
}
