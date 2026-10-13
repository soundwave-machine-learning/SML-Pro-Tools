//! HF17 (non-destructive punch) and HF18 (no session replacement while recording), at the engine
//! level that every caller goes through. The app-level wiring (record point, punch range,
//! recovery entry points) is tested in `crates/ui-egui/tests/hotfix03.rs`.

use serde_json::json;
use soundcraft_engine::Engine;
use soundcraft_engine::io::{add_recording, add_recording_kept};
use soundcraft_model::{Clip, TrackId};
use soundcraft_time::{Range, Samples};

const SR: u32 = 48_000;

/// A recognisable signal: each value encodes where in the capture it came from.
fn signal(len: usize, tag: f32) -> Vec<f32> {
    (0..len).map(|i| tag + (i % 997) as f32 * 1e-5).collect()
}

/// One mono audio track "Vox", armed, with three earlier takes: A 0–48k, B 96k–144k, C 192k–240k.
fn session() -> (Engine, TrackId) {
    let mut e = Engine::default();
    e.execute("track.new", &json!({"kind": "audio", "format": "mono", "name": "Vox"})).unwrap();
    let t = e.session().track_by_name("Vox").unwrap().id;
    e.execute("mix.record_arm", &json!({"tracks": [t.0], "value": true})).unwrap();
    for (at, tag) in [(0, 0.1), (96_000, 0.2), (192_000, 0.3)] {
        add_recording(&mut e, at, vec![signal(48_000, tag)], SR).unwrap();
    }
    (e, t)
}

fn clips(e: &Engine, t: TrackId) -> Vec<Clip> {
    e.session().track(t).unwrap().clips().to_vec()
}

fn render(e: &Engine, r: Range) -> Vec<Vec<f32>> {
    soundcraft_mix::render_range(e.session(), r, 256)
}

/// Samples of `a` and `b` outside `inside` are identical (both rendered over `whole`).
fn same_outside(a: &[Vec<f32>], b: &[Vec<f32>], whole: Range, inside: Range) {
    for (ca, cb) in a.iter().zip(b) {
        for (i, (x, y)) in ca.iter().zip(cb).enumerate() {
            let at = whole.start + i as Samples;
            if !inside.contains(at) {
                assert_eq!(x.to_bits(), y.to_bits(), "sample {at} outside the punch changed");
            }
        }
    }
}

fn sources(e: &Engine) -> Vec<(u64, String, u64)> {
    e.session().sources.iter().map(|s| (s.id.0, s.path.clone(), s.frames)).collect()
}

/// The punch matrix: punch inside a clip, at its start and at its end, with pre/post-roll on
/// (capture wider than the punch) and off (capture = punch). Outside the punch range every
/// sample of the mix is bit-identical; the earlier sources are untouched; the material the take
/// replaced is on an alternate playlist; one undo restores the arrangement and redo re-applies it.
#[test]
fn punch_replaces_only_the_punch_range() {
    let whole = Range::new(0, 260_000);
    for (name, punch) in [("middle", Range::new(100_000, 120_000)), ("start", Range::new(96_000, 110_000)), ("end", Range::new(130_000, 144_000))] {
        for roll in [0, 20_000] {
            let (mut e, t) = session();
            let before_clips = clips(&e, t);
            let before_render = render(&e, whole);
            let before_sources = sources(&e);
            let undo_before = e.undo_depth();
            let capture_start = punch.start - roll;
            let capture_len = (punch.len() + 2 * roll) as usize;
            let ids = add_recording_kept(&mut e, capture_start, vec![signal(capture_len, 0.9)], SR, punch).unwrap();
            assert_eq!(ids.len(), 1, "{name}/{roll}");
            let after = clips(&e, t);
            // The new clip covers exactly the punch range and plays the matching capture frames.
            let take = after.iter().find(|c| c.id == ids[0]).unwrap();
            assert_eq!(take.range(), punch, "{name}/{roll}: take trimmed to the punch");
            match take.content {
                soundcraft_model::ClipContent::Audio { offset, .. } => {
                    assert_eq!(offset, roll as Samples, "{name}/{roll}: roll audio kept in the media")
                }
                _ => unreachable!(),
            }
            // Clips that did not touch the punch range are exactly as they were.
            for c in &before_clips {
                if !c.range().overlaps(&punch) {
                    assert!(after.iter().any(|a| a == c), "{name}/{roll}: clip {} outside the punch changed", c.name);
                }
            }
            // Mix outside the punch: bit-identical.
            same_outside(&before_render, &render(&e, whole), whole, punch);
            // Earlier media untouched; one new source for the take.
            let now = sources(&e);
            assert_eq!(&now[..before_sources.len()], &before_sources[..], "{name}/{roll}");
            assert_eq!(now.len(), before_sources.len() + 1);
            // Replaced material reachable: the playlist as it was is kept as an alternate.
            let tr = e.session().track(t).unwrap();
            assert_eq!(tr.playlists.len(), 2, "{name}/{roll}: alternate playlist kept");
            assert!(tr.playlists[1].name.contains("before take"), "{}", tr.playlists[1].name);
            let alt: Vec<_> = tr.playlists[1].clips.iter().map(|c| (c.range(), c.content.clone(), c.fade_in, c.fade_out)).collect();
            let orig: Vec<_> = before_clips.iter().map(|c| (c.range(), c.content.clone(), c.fade_in, c.fade_out)).collect();
            assert_eq!(alt, orig, "{name}/{roll}: the alternate is the arrangement before the take");
            assert_eq!(tr.active_playlist, 0, "the take is on the playlist that was active");
            // One undo step.
            assert_eq!(e.undo_depth(), undo_before + 1);
            e.execute("edit.undo", &json!({})).unwrap();
            assert_eq!(clips(&e, t), before_clips, "{name}/{roll}: undo restores the arrangement");
            assert_eq!(e.session().track(t).unwrap().playlists.len(), 1);
            e.execute("edit.redo", &json!({})).unwrap();
            assert_eq!(clips(&e, t), after);
        }
    }
}

/// K8 as it was: without a punch selection and with pre-roll, the record point (not the
/// pre-roll start) is where the take begins, so a clip ending inside the pre-roll is untouched.
#[test]
fn pre_roll_and_count_off_never_remove_material() {
    let (mut e, t) = session();
    let before = clips(&e, t);
    // Record point 150 000; capture began 30 000 earlier (pre-roll or count-off) over B's end.
    let point = 150_000;
    let ids = add_recording_kept(&mut e, point - 30_000, vec![signal(60_000, 0.9)], SR, Range::new(point, Samples::MAX)).unwrap();
    let after = clips(&e, t);
    for c in &before {
        assert!(after.iter().any(|a| a == c), "clip {} untouched", c.name);
    }
    let take = after.iter().find(|c| c.id == ids[0]).unwrap();
    assert_eq!(take.range(), Range::new(point, point + 30_000));
    assert_eq!(e.session().track(t).unwrap().playlists.len(), 1, "nothing replaced, no alternate");
}

/// Several clips under one punch, a stereo track fed by a mono capture, a capture that stops early
/// (partial take) and one that never reaches the punch range.
#[test]
fn overlaps_formats_partial_and_empty_captures() {
    // Multiple overlaps: punch from inside A to inside C.
    let (mut e, t) = session();
    let before = clips(&e, t);
    let punch = Range::new(24_000, 216_000);
    add_recording_kept(&mut e, punch.start, vec![signal(punch.len() as usize, 0.9)], SR, punch).unwrap();
    let alt = &e.session().track(t).unwrap().playlists[1].clips;
    assert_eq!(alt.len(), 3, "A, B and C all reachable on the alternate");
    let a = clips(&e, t).into_iter().find(|c| c.range().end == 24_000).expect("A's head kept");
    assert_eq!(a.start, before[0].start);

    // Stereo track, mono interface input: the track stays stereo, both sides recorded.
    let mut e = Engine::default();
    e.execute("track.new", &json!({"kind": "audio", "format": "stereo", "name": "Pair"})).unwrap();
    let st = e.session().track_by_name("Pair").unwrap().id;
    e.execute("mix.record_arm", &json!({"tracks": [st.0], "value": true})).unwrap();
    add_recording_kept(&mut e, 0, vec![signal(10_000, 0.5)], SR, Range::new(0, 10_000)).unwrap();
    assert_eq!(e.session().track(st).unwrap().channels(), 2);
    let src = &e.session().sources[0];
    assert_eq!(src.channels, 2, "dual mono, no lost right channel");

    // Partial capture (device lost after 5 000 frames of a 20 000-frame punch): what was captured
    // is placed; nothing outside it is removed.
    let (mut e, t) = session();
    let before = clips(&e, t);
    let punch = Range::new(100_000, 120_000);
    let ids = add_recording_kept(&mut e, punch.start, vec![signal(5_000, 0.9)], SR, punch).unwrap();
    let after = clips(&e, t);
    assert_eq!(after.iter().find(|c| c.id == ids[0]).unwrap().range(), Range::new(100_000, 105_000));
    let b_after: Vec<_> = after.iter().filter(|c| c.range().overlaps(&Range::new(96_000, 144_000)) && c.id != ids[0]).map(Clip::range).collect();
    assert_eq!(b_after, vec![Range::new(96_000, 100_000), Range::new(105_000, 144_000)], "B outside the captured part kept");
    assert_eq!(
        e.session().track(t).unwrap().playlists[1].clips,
        before
            .iter()
            .map(|c| {
                let mut c = c.clone();
                c.id = e.session().track(t).unwrap().playlists[1].clips.iter().find(|x| x.range() == c.range()).unwrap().id;
                c
            })
            .collect::<Vec<_>>()
    );

    // A capture that ends before the punch starts: nothing changes, no undo step, still clean.
    let (mut e, t) = session();
    e.mark_clean();
    let before = clips(&e, t);
    let depth = e.undo_depth();
    let ids = add_recording_kept(&mut e, 0, vec![signal(1_000, 0.9)], SR, Range::new(100_000, 120_000)).unwrap();
    assert!(ids.is_empty());
    assert_eq!(clips(&e, t), before);
    assert_eq!(e.undo_depth(), depth);
    assert!(!e.is_dirty());
}

/// Save and reopen keep the take, the split clips and the alternate playlist.
#[test]
fn punch_result_survives_save_and_reopen() {
    let dir = std::env::temp_dir().join(format!("sml-hf17-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (mut e, t) = session();
    let punch = Range::new(100_000, 120_000);
    add_recording_kept(&mut e, punch.start - 10_000, vec![signal(40_000, 0.9)], SR, punch).unwrap();
    let path = dir.join("punch.scraft");
    e.execute("session.save_as", &json!({"path": path.to_string_lossy()})).unwrap();
    let whole = Range::new(0, 260_000);
    let before = render(&e, whole);
    let mut f = Engine::default();
    f.execute("session.open", &json!({"path": path.to_string_lossy()})).unwrap();
    let ft = f.session().track_by_name("Vox").unwrap().id;
    assert_eq!(f.session().track(ft).unwrap().playlists.len(), 2);
    assert_eq!(clips(&f, ft).iter().map(Clip::range).collect::<Vec<_>>(), clips(&e, t).iter().map(Clip::range).collect::<Vec<_>>());
    same_outside(&before, &render(&f, whole), whole, Range::new(0, 0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// HF18: while a take is being recorded (count-off and pre-roll included: the engine's
/// `recording` flag is set when Record starts), New, Open, Close and Revert are refused by the
/// engine for every caller, before anything runs: the session, undo history, dirty flag and
/// recording state are unchanged. After the take is finished they work again.
#[test]
fn session_replacement_is_refused_while_recording() {
    let dir = std::env::temp_dir().join(format!("sml-hf18-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (mut e, _) = session();
    let path = dir.join("other.scraft");
    e.execute("session.save_as", &json!({"path": path.to_string_lossy()})).unwrap();
    add_recording(&mut e, 300_000, vec![signal(1_000, 0.4)], SR).unwrap(); // unsaved change
    e.transport.recording = true;
    let doc = e.session_arc();
    let (rev, depth, dirty) = (e.revision, e.undo_depth(), e.is_dirty());
    for (id, p) in [
        ("session.new", json!({})),
        ("session.open", json!({"path": path.to_string_lossy()})),
        ("session.close", json!({})),
        ("session.revert", json!({})),
    ] {
        let err = e.execute(id, &p).unwrap_err();
        assert!(matches!(err, soundcraft_engine::EngineError::Recording(..)), "{id}: {err}");
        assert!(err.to_string().contains("Stop the transport"), "{err}");
        assert!(std::sync::Arc::ptr_eq(&doc, &e.session_arc()), "{id}: session unchanged");
        assert_eq!((e.revision, e.undo_depth(), e.is_dirty(), e.transport.recording), (rev, depth, dirty, true), "{id}");
    }
    // The library entry points refuse too (CLI scripts, recovery).
    assert!(soundcraft_engine::io::open_session(&mut e, &path.to_string_lossy()).is_err());
    let store = soundcraft_engine::autosave::Store::new(dir.join("store"));
    store
        .snapshot(&soundcraft_engine::autosave::SnapshotRequest { session: e.session_arc(), revision: e.revision, plugin_state_age_s: None })
        .unwrap();
    let r = store.recover().unwrap();
    assert!(soundcraft_engine::autosave::open_recovered(&mut e, r).is_err());
    assert!(std::sync::Arc::ptr_eq(&doc, &e.session_arc()));
    // Finished: deliberate recovery and opening work again.
    e.transport.recording = false;
    let r = store.recover().unwrap();
    soundcraft_engine::autosave::open_recovered(&mut e, r).unwrap();
    assert!(e.path.is_none() && e.is_dirty(), "recovered as unsaved; the named session is not overwritten");
    e.execute("session.open", &json!({"path": path.to_string_lossy()})).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// K8 reproduction kept as evidence: the placement the app used before Hotfix 03 (the whole
/// capture, pre-roll included, placed with `add_recording`, then trimmed to the punch with
/// `edit.trim_to_fill_selection`) removed part of clip B outside the punch range. This test
/// asserts that old behaviour of the old path, so the difference to the new path stays visible.
#[test]
fn k8_reproduction_with_the_old_placement_path() {
    let (mut e, t) = session();
    let punch = Range::new(110_000, 120_000);
    let ids = add_recording(&mut e, 100_000, vec![signal(30_000, 0.9)], SR).unwrap();
    e.execute("edit.trim_to_fill_selection", &json!({"clips": [ids[0].0], "start": punch.start, "end": punch.end})).unwrap();
    let covered: Samples = clips(&e, t).iter().filter(|c| c.range().overlaps(&Range::new(96_000, 144_000))).map(|c| c.length).sum();
    assert!(covered < 48_000, "old path: B lost material outside the punch (covered {covered} of 48000)");
}
