//! Bend tool commands (EDIT_TOOLBAR_SPEC T08): warp anchors on a clip and the render handshake
//! with the off-thread service in `crate::warp`. The anchor edit is the undoable step; applying a
//! finished render (`clip.warp_poll`) swaps the clip's content to a derived source and leaves
//! no undo record of its own, so undoing the edit restores anchors and content together.
//! The original material is remembered in `edit.values` (`elastic.<clip>.{src,off,len}`, the
//! same keys `clip.remove_warp` restores from) and is never rewritten.

use super::*;
use crate::cmd;
use crate::warp::{Job, Status};
use serde_json::json;
use soundcraft_model::{ClipContent, ClipId, SourceId, WarpAnchor, validate_warp, warped_len};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "clip.warp_set",
            "Set Warp Anchors",
            ["Clip"],
            None,
            "{clip?, anchors: [[src, dst]…] (samples from the clip start; strictly increasing; ≥64 apart; segment ratio 0.25..4)} — replaces the clip's Bend map and queues a pitch-preserving render; the clip keeps its last render until the new one is applied by clip.warp_poll",
            has_selection,
            warp_set
        ),
        cmd!(
            "clip.warp_clear",
            "Clear Warp Anchors",
            ["Clip"],
            None,
            "{clip?} — removes the Bend map and restores the original material",
            has_selection,
            warp_clear
        ),
        cmd!(query "clip.warp_poll", "Apply Finished Bend Renders", [], None, "{} — host-side: applies finished renders whose revision still matches; stale results are dropped", always, warp_poll),
        cmd!(query "clip.warp_status", "Bend Status", [], None, "{clip?} — pending/rendering/error state per clip, and the service counters", always, warp_status),
    ]
}

/// The clip's original material: (source, offset, length), recorded on the first warp.
fn original(s: &soundcraft_model::Session, id: ClipId) -> Option<(SourceId, Samples, Samples)> {
    let key = |k: &str| format!("elastic.{}.{k}", id.0);
    let (src, off, len) =
        (s.edit.values.get(&key("src")).copied()?, s.edit.values.get(&key("off")).copied()?, s.edit.values.get(&key("len")).copied()?);
    Some((SourceId(src as u64), off as Samples, (len as Samples).max(1)))
}

fn target(e: &Engine, cmd: &str, p: &Value) -> Result<ClipId> {
    p.get("clip").and_then(Value::as_u64).map(ClipId).or_else(|| clip_ids_param(e, p).first().copied()).ok_or_else(|| bad(cmd, "`clip` required"))
}

fn warp_set(e: &mut Engine, p: &Value) -> Result<Value> {
    let cmd = "clip.warp_set";
    let id = target(e, cmd, p)?;
    let anchors: Vec<WarpAnchor> = p
        .get("anchors")
        .and_then(Value::as_array)
        .ok_or_else(|| bad(cmd, "`anchors` ([[src, dst]…]) required"))?
        .iter()
        .map(|a| {
            let src = a.get(0).and_then(Value::as_i64).ok_or_else(|| bad(cmd, "anchor `src` must be an integer sample"))?;
            let dst = a.get(1).and_then(Value::as_i64).ok_or_else(|| bad(cmd, "anchor `dst` must be an integer sample"))?;
            Ok(WarpAnchor { src, dst })
        })
        .collect::<Result<Vec<_>>>()?;
    if anchors.is_empty() {
        return warp_clear(e, &json!({"clip": id.0}));
    }
    let (orig, name, sr) = {
        let s = e.session();
        let (_, c) = s.find_clip(id).ok_or_else(|| bad(cmd, format!("clip {} not found", id.0)))?;
        if c.edit_locked || c.time_locked {
            return Err(bad(cmd, format!("clip `{}` is locked", c.name)));
        }
        let ClipContent::Audio { source, offset } = c.content else { return Err(bad(cmd, "Bend applies to audio clips")) };
        let orig = original(s, id).unwrap_or((source, offset, c.length));
        (orig, c.name.clone(), s.sample_rate.as_f64() as f32)
    };
    let (src_id, offset, src_len) = orig;
    validate_warp(&anchors, src_len).map_err(|m| bad(cmd, m))?;
    let source = e.session().pool.get(src_id).cloned().ok_or_else(|| bad(cmd, format!("the original audio of `{name}` is not loaded")))?;
    let rev = e.session().find_clip(id).map_or(0, |(_, c)| c.warp_rev) + 1;
    // Admission before mutation: a full queue changes nothing.
    e.warp.submit(Job { clip: id, rev, source, offset, src_len, anchors: anchors.clone(), sample_rate: sr }).map_err(|m| bad(cmd, m))?;
    let s = e.session_mut();
    let key = |k: &str| format!("elastic.{}.{k}", id.0);
    s.edit.values.entry(key("src")).or_insert(src_id.0 as f64);
    s.edit.values.entry(key("off")).or_insert(offset as f64);
    s.edit.values.entry(key("len")).or_insert(src_len as f64);
    let Some(c) = s.find_clip_mut(id) else { return Err(bad(cmd, "clip vanished")) };
    c.warp = anchors.clone();
    c.warp_rev = rev;
    Ok(json!({"clip": id.0, "rev": rev, "anchors": anchors.len(), "rendered_len": warped_len(&anchors, src_len), "pending": true}))
}

/// After a reopen (R30): a warped clip whose rendered (derived) source did not load but whose
/// original material did is queued for a fresh render from its anchors. Returns one message per
/// clip; the clip keeps the missing source until the render arrives through `clip.warp_poll`.
pub(crate) fn rerender_missing_warps(e: &mut Engine) -> Vec<String> {
    let sr = e.session().sample_rate.as_f64() as f32;
    let mut jobs = Vec::new();
    {
        let s = e.session();
        for t in &s.tracks {
            for c in t.clips() {
                let ClipContent::Audio { source, .. } = c.content else { continue };
                if c.warp.is_empty() || s.pool.get(source).is_some() {
                    continue;
                }
                let Some((src_id, offset, src_len)) = original(s, c.id) else { continue };
                let Some(orig) = s.pool.get(src_id).cloned() else { continue };
                jobs.push((
                    c.id,
                    c.name.clone(),
                    Job { clip: c.id, rev: c.warp_rev, source: orig, offset, src_len, anchors: c.warp.clone(), sample_rate: sr },
                ));
            }
        }
    }
    let mut out = Vec::new();
    for (id, name, job) in jobs {
        match e.warp.submit(job) {
            Ok(()) => out.push(format!("Bend: re-rendering `{name}` (clip {}) from its original material", id.0)),
            Err(m) => out.push(format!("Bend: `{name}` (clip {}) needs a re-render but the queue refused it: {m}", id.0)),
        }
    }
    out
}

fn warp_clear(e: &mut Engine, p: &Value) -> Result<Value> {
    let cmd = "clip.warp_clear";
    let id = target(e, cmd, p)?;
    {
        let s = e.session();
        let (_, c) = s.find_clip(id).ok_or_else(|| bad(cmd, format!("clip {} not found", id.0)))?;
        if c.edit_locked || c.time_locked {
            return Err(bad(cmd, format!("clip `{}` is locked", c.name)));
        }
    }
    let orig = original(e.session(), id);
    e.warp.forget(id);
    let s = e.session_mut();
    let Some(c) = s.find_clip_mut(id) else { return Err(bad(cmd, "clip vanished")) };
    c.warp.clear();
    c.warp_rev += 1;
    if let Some((src, off, len)) = orig {
        c.content = ClipContent::Audio { source: src, offset: off };
        c.length = len;
        c.fade_out.len = c.fade_out.len.min(c.length);
        c.fade_in.len = c.fade_in.len.min(c.length);
    }
    Ok(json!({"clip": id.0, "restored": orig.is_some()}))
}

fn warp_poll(e: &mut Engine, _p: &Value) -> Result<Value> {
    let done = e.warp.poll();
    let mut applied = Vec::new();
    let mut dropped = 0u64;
    let mut errors = Vec::new();
    for d in done {
        let current = e.session().find_clip(d.clip).map(|(_, c)| (c.warp_rev, c.name.clone()));
        let Some((rev, name)) = current else {
            dropped += 1;
            continue;
        };
        if rev != d.rev {
            dropped += 1;
            continue;
        }
        match d.result {
            Ok(buf) => {
                let len = buf.channels.first().map_or(0, Vec::len) as Samples;
                if len <= 0 {
                    errors.push(json!({"clip": d.clip.0, "error": "empty render"}));
                    continue;
                }
                let s = e.session_mut();
                let src = crate::io::add_source(s, &format!("{name}-Bend"), buf, None, soundcraft_audio_io::FileFormat::Wav);
                if let Some(c) = s.find_clip_mut(d.clip) {
                    c.content = ClipContent::Audio { source: src, offset: 0 };
                    c.length = len;
                    c.fade_out.len = c.fade_out.len.min(len);
                    c.fade_in.len = c.fade_in.len.min(len);
                    applied.push(json!({"clip": d.clip.0, "rev": d.rev, "length": len, "source": src.0}));
                }
            }
            Err(m) => errors.push(json!({"clip": d.clip.0, "rev": d.rev, "error": m})),
        }
    }
    e.warp.dropped_stale += dropped;
    Ok(json!({"applied": applied, "dropped_stale": dropped, "errors": errors, "busy": e.warp.is_busy()}))
}

fn warp_status(e: &mut Engine, p: &Value) -> Result<Value> {
    let clips: Vec<ClipId> = match p.get("clip").and_then(Value::as_u64) {
        Some(c) => vec![ClipId(c)],
        None => e.session().tracks.iter().flat_map(|t| t.clips().iter().filter(|c| !c.warp.is_empty()).map(|c| c.id)).collect(),
    };
    let rows: Vec<Value> = clips
        .iter()
        .map(|id| {
            let (anchors, rev) = e.session().find_clip(*id).map_or((0, 0), |(_, c)| (c.warp.len(), c.warp_rev));
            let st = match e.warp.status(*id) {
                None => json!("ready"),
                Some(Status::Queued { rev }) => json!({"queued": rev}),
                Some(Status::Rendering { rev }) => json!({"rendering": rev}),
                Some(Status::Error { rev, message }) => json!({"error": message, "rev": rev}),
            };
            json!({"clip": id.0, "anchors": anchors, "rev": rev, "status": st})
        })
        .collect();
    Ok(json!({"clips": rows, "busy": e.warp.is_busy(), "dropped_stale": e.warp.dropped_stale}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_audio_io::AudioBuffer;
    use soundcraft_model::{ChannelFormat, Clip, Session, SourceAudio, TrackKind};
    use std::sync::Arc;

    fn engine() -> (Engine, ClipId) {
        let mut s = Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
        let tone: Vec<f32> = (0..48_000).map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 48_000.0).sin() * 0.5).collect();
        s.pool.insert(SourceId(1), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![tone] })));
        let id = s.new_clip_id();
        s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "lead", SourceId(1), 0, 10_000, 48_000));
        s.edit.selected_clips = vec![id];
        s.edit.selected_tracks = vec![t];
        (Engine::new(s), id)
    }

    fn wait_applied(e: &mut Engine) -> Value {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let r = e.execute("clip.warp_poll", &json!({})).unwrap();
            if !r["applied"].as_array().is_none_or(Vec::is_empty) || !r["errors"].as_array().is_none_or(Vec::is_empty) {
                return r;
            }
            assert!(std::time::Instant::now() < deadline, "render did not finish");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// R30: the derived (rendered) media of a warped clip is saved with the session; when it is
    /// missing on reopen, the clip is re-rendered from its original material and anchors.
    #[test]
    fn warped_clip_is_rerendered_when_its_derived_media_is_missing_on_reopen() {
        // The original is a registered source (as a recorded or imported file is), so the save
        // writes it and the reopen can rebuild the derived render from it.
        let mut s = Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
        let tone: Vec<f32> = (0..48_000).map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 48_000.0).sin() * 0.5).collect();
        let src = crate::io::add_source(
            &mut s,
            "lead",
            AudioBuffer { sample_rate: 48_000, channels: vec![tone] },
            None,
            soundcraft_audio_io::FileFormat::Wav,
        );
        let id = s.new_clip_id();
        s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "lead", src, 0, 0, 48_000));
        s.edit.selected_clips = vec![id];
        s.edit.selected_tracks = vec![t];
        let mut e = Engine::new(s);
        let anchors = json!([[12_000, 18_000], [36_000, 36_000]]);
        e.execute("clip.warp_set", &json!({"clip": id.0, "anchors": anchors})).unwrap();
        let r = wait_applied(&mut e);
        let len = r["applied"][0]["length"].as_i64().unwrap();
        let dir = std::env::temp_dir().join(format!("sml-bend-reopen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.scraft").to_string_lossy().into_owned();
        crate::io::save_session(&mut e, &path).unwrap();
        let derived =
            std::fs::read_dir(dir.join("Audio Files")).unwrap().flatten().find(|f| f.file_name().to_string_lossy().contains("Bend")).unwrap();
        std::fs::remove_file(derived.path()).unwrap();
        let mut e2 = Engine::default();
        let missing = crate::io::open_session(&mut e2, &path).unwrap();
        assert_eq!(missing.len(), 1, "{missing:?}");
        assert!(e2.messages.iter().any(|m| m.contains("re-rendering")), "{:?}", e2.messages);
        assert!(e2.warp.is_busy());
        let r = wait_applied(&mut e2);
        assert_eq!(r["applied"][0]["length"].as_i64(), Some(len));
        let (_, c) = e2.session().find_clip(id).unwrap();
        let ClipContent::Audio { source, .. } = c.content else { panic!() };
        assert!(e2.session().pool.get(source).is_some(), "the clip plays again");
        assert_eq!(c.warp.len(), 2, "anchors kept");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T-T3: the edit map is honoured exactly, the original stays, undo restores anchors and
    /// content together, a stale result never replaces a newer revision, clear restores.
    #[test]
    fn bend_edit_map_render_undo_and_stale_results() {
        let (mut e, id) = engine();
        let src_before = e.session().pool.get(SourceId(1)).unwrap().buffer.channels.clone();
        let r = e.execute("clip.warp_set", &json!({"anchors": [[12_000, 18_000], [36_000, 36_000]]})).unwrap();
        assert_eq!(r["rendered_len"], 48_000);
        assert_eq!(e.session().find_clip(id).unwrap().1.warp.len(), 2);
        assert!(
            e.session().find_clip(id).unwrap().1.content == ClipContent::Audio { source: SourceId(1), offset: 0 },
            "last valid content kept while pending"
        );
        assert_eq!(e.undo_depth(), 1);
        let st = e.execute("clip.warp_status", &json!({"clip": id.0})).unwrap();
        assert_ne!(st["clips"][0]["status"], json!("ready"));
        let applied = wait_applied(&mut e);
        assert_eq!(applied["applied"][0]["clip"], id.0);
        let c = e.session().find_clip(id).unwrap().1.clone();
        assert_eq!(c.length, 48_000);
        assert_eq!(c.start, 10_000, "the clip stays where it was");
        let ClipContent::Audio { source, offset } = c.content else { panic!() };
        assert_ne!(source, SourceId(1));
        assert_eq!(offset, 0);
        assert_eq!(e.session().pool.get(SourceId(1)).unwrap().buffer.channels, src_before, "original immutable");
        assert_eq!(e.undo_depth(), 1, "applying the render adds no undo step");
        // A second edit that lengthens the clip; then a third before the second lands.
        e.execute("clip.warp_set", &json!({"anchors": [[12_000, 24_000]]})).unwrap();
        e.execute("clip.warp_set", &json!({"anchors": [[12_000, 15_000]]})).unwrap();
        let rev = e.session().find_clip(id).unwrap().1.warp_rev;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let r = e.execute("clip.warp_poll", &json!({})).unwrap();
            if r["applied"].as_array().is_some_and(|a| a.iter().any(|x| x["rev"] == rev)) {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let c = e.session().find_clip(id).unwrap().1.clone();
        assert_eq!(c.length, 48_000 + 3_000, "the newest revision's length, never the stale one");
        // Undo twice: back to the first map and its render length.
        e.execute("edit.undo", &json!({})).unwrap();
        e.execute("edit.undo", &json!({})).unwrap();
        let c = e.session().find_clip(id).unwrap().1.clone();
        assert_eq!((c.warp.len(), c.length), (2, 48_000));
        // Late results for undone revisions are dropped.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while e.warp.is_busy() && std::time::Instant::now() < deadline {
            e.execute("clip.warp_poll", &json!({})).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(e.session().find_clip(id).unwrap().1.length, 48_000);
        // Clear restores the original material; locks reject.
        e.execute("clip.warp_clear", &json!({})).unwrap();
        let c = e.session().find_clip(id).unwrap().1.clone();
        assert!(c.warp.is_empty());
        assert_eq!(c.content, ClipContent::Audio { source: SourceId(1), offset: 0 });
        e.session_mut().find_clip_mut(id).unwrap().edit_locked = true;
        assert!(e.execute("clip.warp_set", &json!({"anchors": [[12_000, 18_000]]})).is_err());
        e.session_mut().find_clip_mut(id).unwrap().edit_locked = false;
        // Crossed or too-steep anchors are rejected without a document change.
        let before = e.session_arc();
        assert!(e.execute("clip.warp_set", &json!({"anchors": [[12_000, 18_000], [11_000, 20_000]]})).is_err());
        assert!(e.execute("clip.warp_set", &json!({"anchors": [[1_000, 10_000]]})).is_err());
        assert!(Arc::ptr_eq(&before, &e.session_arc()));
    }
}
