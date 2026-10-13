//! HF10: dragging audio clips between tracks through the real pointer path (press, moves,
//! release on the Edit window), not only the command.
use egui::{Event, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::Harness;
use serde_json::json;
use soundcraft_model::{ClipId, TrackId};
use soundcraft_ui_egui::{Services, SoundApp, edit_window};

fn step(h: &mut Harness<'_, SoundApp>) {
    let mut raw = std::mem::take(h.input_mut());
    h.state_mut().raw_input_hook(&mut raw);
    *h.input_mut() = raw;
    h.step();
}

fn steps(h: &mut Harness<'_, SoundApp>, n: usize) {
    for _ in 0..n {
        step(h);
    }
}

fn harness(w: f32, hgt: f32) -> Harness<'static, SoundApp> {
    let app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    let mut h = Harness::builder().with_size(vec2(w, hgt)).with_pixels_per_point(1.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.state_mut().run("window.edit", json!({})).unwrap();
    steps(&mut h, 6);
    h
}

fn track(h: &Harness<'_, SoundApp>, name: &str) -> TrackId {
    h.state().engine.session().track_by_name(name).unwrap().id
}

fn row(h: &Harness<'_, SoundApp>, t: TrackId) -> [f32; 4] {
    h.state().edit_layout.rows.iter().find(|(id, _)| *id == t.0).map(|(_, r)| *r).expect("row laid out")
}

/// Screen point inside `clip`, at `frac` of its lane height (0.75: the Smart tool's grabber half).
fn clip_point(h: &Harness<'_, SoundApp>, clip: ClipId, frac: f32) -> Pos2 {
    let s = h.state().engine.session();
    let (t, c) = s.find_clip(clip).unwrap();
    let tl = h.state().edit_layout.timeline;
    let tl = egui::Rect::from_min_max(pos2(tl[0], tl[1]), pos2(tl[2], tl[3]));
    let x0 = edit_window::x_of(s, tl, c.start).max(tl.min.x);
    let x1 = edit_window::x_of(s, tl, c.end()).min(tl.max.x);
    let r = row(h, t);
    pos2(x0 + (x1 - x0) * 0.5, r[1] + (r[3] - r[1]) * frac)
}

/// A held modifier, as the platform reports it on every frame of the drag.
fn step_with(h: &mut Harness<'_, SoundApp>, mods: Modifiers) {
    // Straight into this frame's input (the synthetic queue replays one event per frame).
    h.input_mut().events.push(Event::ModifiersChanged(mods));
    step(h);
}

fn drag(h: &mut Harness<'_, SoundApp>, from: Pos2, to: Pos2, mods: Modifiers) {
    let s = &mut h.state_mut().synthetic;
    s.push(Event::PointerMoved(from));
    s.push(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: mods });
    step_with(h, mods);
    step_with(h, mods);
    for k in 1..=8 {
        let p = from + (to - from) * (k as f32 / 8.0);
        h.state_mut().synthetic.push(Event::PointerMoved(p));
        step_with(h, mods);
        if std::env::var("DIAG").is_ok() {
            eprintln!("k{k} {:?} gesture {:?}", p, h.state().gesture);
        }
    }
    h.state_mut().synthetic.push(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: mods });
    step_with(h, mods);
    step_with(h, Modifiers::NONE);
    steps(h, 1);
}

fn first_clip(h: &Harness<'_, SoundApp>, t: TrackId) -> ClipId {
    h.state().engine.session().track(t).unwrap().clips().first().unwrap().id
}

fn clip_on(h: &Harness<'_, SoundApp>, c: ClipId) -> (TrackId, soundcraft_model::Clip) {
    h.state().engine.session().find_clip(c).map(|(t, c)| (t, c.clone())).unwrap()
}

fn row_point(h: &Harness<'_, SoundApp>, t: TrackId, x: f32) -> Pos2 {
    let r = row(h, t);
    pos2(x, r[1] + (r[3] - r[1]) * 0.75)
}

/// HF10 diagnosis, kept as the regression: a downward drag through the real pointer path never
/// left its track, because the rows below the dragged one were not laid out yet when the drag
/// looked them up. Now the clip lands on the track under the pointer, unchanged otherwise, and
/// one undo puts it back.
#[test]
fn smart_tool_drag_moves_a_clip_down_and_up_with_one_undo() {
    let mut h = harness(1600.0, 1000.0);
    let (kick, snare) = (track(&h, "Kick"), track(&h, "Snare"));
    let clip = first_clip(&h, kick);
    let before = clip_on(&h, clip).1;
    let from = clip_point(&h, clip, 0.75);
    let to = row_point(&h, snare, from.x);
    drag(&mut h, from, to, Modifiers::NONE);
    let (t, after) = clip_on(&h, clip);
    assert_eq!(t, snare, "moved to Snare ({:?})", h.state().ui.status);
    assert_eq!(after, before, "same start, offset, length, fades and gain; same media reference");
    // One drag is one undo step; redo moves it again.
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert_eq!(clip_on(&h, clip), (kick, before.clone()));
    h.state_mut().run("edit.redo", json!({})).unwrap();
    assert_eq!(clip_on(&h, clip).0, snare);
    // And back up again.
    steps(&mut h, 2);
    let from = clip_point(&h, clip, 0.75);
    let to = row_point(&h, kick, from.x);
    drag(&mut h, from, to, Modifiers::NONE);
    assert_eq!(clip_on(&h, clip), (kick, before));
}

/// Ctrl (Cmd on macOS) while dragging keeps the exact sample position even when the pointer also
/// moves sideways; without it the clip follows the pointer in time, snapped in Grid mode.
#[test]
fn time_lock_modifier_and_grid_snap() {
    let mut h = harness(1600.0, 1000.0);
    let (kick, snare) = (track(&h, "Kick"), track(&h, "Snare"));
    let clip = first_clip(&h, kick);
    let before = clip_on(&h, clip).1;
    let from = clip_point(&h, clip, 0.75);
    let lock = if cfg!(target_os = "macos") { Modifiers::MAC_CMD | Modifiers::COMMAND } else { Modifiers::CTRL | Modifiers::COMMAND };
    let to = row_point(&h, snare, from.x + 137.0);
    drag(&mut h, from, to, lock);
    let (t, c) = clip_on(&h, clip);
    assert_eq!((t, c.start), (snare, before.start), "time locked: track changed, sample position kept");
    // Grid: the new start is on the grid.
    h.state_mut().run("edit.mode", json!({"mode": "grid"})).unwrap();
    steps(&mut h, 2);
    let from = clip_point(&h, clip, 0.75);
    let to = row_point(&h, kick, from.x + 91.0);
    drag(&mut h, from, to, Modifiers::NONE);
    let (t, c) = clip_on(&h, clip);
    let s = h.state().engine.session();
    assert_eq!(t, kick);
    assert_ne!(c.start, before.start, "moved in time");
    assert_eq!(s.edit.grid.snap(c.start, s.sample_rate, &s.tempo, s.frame_rate), c.start, "snapped to the grid");
}

/// Escape before release cancels: nothing moves and nothing is added to undo.
#[test]
fn escape_cancels_a_drag() {
    let mut h = harness(1600.0, 1000.0);
    let (kick, snare) = (track(&h, "Kick"), track(&h, "Snare"));
    let clip = first_clip(&h, kick);
    let before = clip_on(&h, clip);
    let rev = h.state().engine.revision;
    let from = clip_point(&h, clip, 0.75);
    let to = row_point(&h, snare, from.x + 40.0);
    h.state_mut().synthetic.push(Event::PointerMoved(from));
    h.state_mut().synthetic.push(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    steps(&mut h, 2);
    for k in 1..=6 {
        h.state_mut().synthetic.push(Event::PointerMoved(from + (to - from) * (k as f32 / 6.0)));
        steps(&mut h, 1);
    }
    h.state_mut().synthetic.push(Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    steps(&mut h, 1);
    h.state_mut().synthetic.push(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    steps(&mut h, 3);
    assert_eq!(clip_on(&h, clip), before);
    assert_eq!(h.state().engine.revision, rev, "nothing was applied");
}

/// Invalid targets are refused visibly and the source is untouched: an audio clip onto an Aux
/// track, and a stereo clip onto a mono track (it would lose a channel; the format is kept).
#[test]
fn invalid_targets_are_refused_and_the_source_is_untouched() {
    let mut h = harness(1600.0, 1000.0);
    h.state_mut().run("track.new", json!({"kind": "aux", "format": "stereo", "name": "FX Aux"})).unwrap();
    h.state_mut().run("track.new", json!({"kind": "audio", "format": "mono", "name": "Mono Vox"})).unwrap();
    h.state_mut().run("track.new", json!({"kind": "audio", "format": "stereo", "name": "Stereo Bed"})).unwrap();
    steps(&mut h, 4);
    let kick = track(&h, "Kick");
    let clip = first_clip(&h, kick);
    let before = clip_on(&h, clip);
    // Make the Kick clip's media stereo for this check.
    {
        let s = h.state_mut().engine.session_mut();
        let src = match s.find_clip(clip).unwrap().1.content {
            soundcraft_model::ClipContent::Audio { source, .. } => source,
            _ => unreachable!(),
        };
        s.sources.iter_mut().find(|x| x.id == src).unwrap().channels = 2;
    }
    steps(&mut h, 2);
    let rev = h.state().engine.revision;
    for target in ["FX Aux", "Mono Vox"] {
        let t = track(&h, target);
        let from = clip_point(&h, clip, 0.75);
        let to = row_point(&h, t, from.x);
        drag(&mut h, from, to, Modifiers::NONE);
        assert_eq!(clip_on(&h, clip), before, "{target}: refused, source untouched");
        assert!(h.state().ui.status.starts_with("Clip not moved:"), "{target}: {}", h.state().ui.status);
        assert_eq!(h.state().engine.session().track(t).unwrap().format, h.state().engine.session().track(t).unwrap().format);
    }
    assert_eq!(h.state().engine.revision, rev);
    assert_eq!(h.state().engine.session().track(track(&h, "Mono Vox")).unwrap().channels(), 1, "format unchanged");
    // A stereo track takes it.
    let st = track(&h, "Stereo Bed");
    let from = clip_point(&h, clip, 0.75);
    let to = row_point(&h, st, from.x);
    drag(&mut h, from, to, Modifiers::NONE);
    assert_eq!(clip_on(&h, clip).0, st);
    // The command refuses too (agents and the control channel).
    let err = h.state_mut().run("edit.move_clips", json!({"clips": [clip.0], "track": "Mono Vox"})).unwrap_err();
    assert!(err.contains("lose channels"), "{err}");
}

/// A selection on two tracks moves together, each clip keeping its distance in tracks.
#[test]
fn a_multi_track_selection_moves_together() {
    let mut h = harness(1600.0, 1000.0);
    let names: Vec<String> =
        h.state().engine.session().tracks.iter().filter(|t| t.kind.has_playlist() && !t.clips().is_empty()).map(|t| t.name.clone()).collect();
    assert!(names.len() >= 3, "{names:?}");
    let (a, b, c) = (track(&h, &names[0]), track(&h, &names[1]), track(&h, &names[2]));
    let (ca, cb) = (first_clip(&h, a), first_clip(&h, b));
    h.state_mut().engine.session_mut().edit.selected_clips = vec![ca, cb];
    steps(&mut h, 2);
    let from = clip_point(&h, ca, 0.75);
    let to = row_point(&h, b, from.x);
    drag(&mut h, from, to, Modifiers::NONE);
    assert_eq!((clip_on(&h, ca).0, clip_on(&h, cb).0), (b, c), "both moved down one track ({})", h.state().ui.status);
}

/// Taller tracks, zoomed in and scrolled: the destination follows the laid-out rows; dragging to
/// the bottom edge scrolls so a track below the view is reachable in one drag.
#[test]
fn heights_zoom_scroll_and_a_target_below_the_view() {
    let mut h = harness(1366.0, 560.0);
    let all: Vec<u64> = h.state().engine.session().tracks.iter().map(|t| t.id.0).collect();
    h.state_mut().run("track.height", json!({"tracks": all, "height": "large"})).unwrap();
    h.state_mut().run("view.zoom_in", json!({})).ok();
    steps(&mut h, 4);
    let order: Vec<TrackId> = h.state().engine.session().tracks.iter().filter(|t| t.kind.has_playlist() && !t.hidden).map(|t| t.id).collect();
    let first = *order.iter().find(|t| !h.state().engine.session().track(**t).unwrap().clips().is_empty()).unwrap();
    let clip = first_clip(&h, first);
    let before = clip_on(&h, clip).1;
    // The lowest track that can take this clip.
    let last =
        *order.iter().rev().find(|t| soundcraft_engine::edit::move_target_problem(h.state().engine.session(), &before, **t).is_none()).unwrap();
    let tl = h.state().edit_layout.timeline;
    assert!(row(&h, last)[1] > tl[3], "the last track starts below the view");
    // Press on the clip, drag to the bottom edge and hold there until the last track scrolls in.
    let from = clip_point(&h, clip, 0.75);
    let edge = pos2(from.x, tl[3] - 4.0);
    h.state_mut().synthetic.push(Event::PointerMoved(from));
    h.state_mut().synthetic.push(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    steps(&mut h, 2);
    for k in 1..=6 {
        h.state_mut().synthetic.push(Event::PointerMoved(from + (edge - from) * (k as f32 / 6.0)));
        steps(&mut h, 1);
    }
    let mut n = 0;
    while row(&h, last)[3] > tl[3] - 2.0 && n < 400 {
        h.state_mut().synthetic.push(Event::PointerMoved(edge));
        steps(&mut h, 1);
        n += 1;
    }
    let to = row_point(&h, last, from.x);
    h.state_mut().synthetic.push(Event::PointerMoved(to));
    steps(&mut h, 2);
    h.state_mut().synthetic.push(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    steps(&mut h, 3);
    let (t, c) = clip_on(&h, clip);
    assert_eq!(t, last, "reached the track below the view ({})", h.state().ui.status);
    assert_eq!(c.start, before.start);
    assert!(h.state().edit_layout.scroll_y > 0.0, "the tracks scrolled");
}

/// Save and reopen keeps the moved clip on its new track with the same media reference.
#[test]
fn a_moved_clip_survives_save_and_reopen() {
    let mut h = harness(1600.0, 1000.0);
    let (kick, snare) = (track(&h, "Kick"), track(&h, "Snare"));
    let clip = first_clip(&h, kick);
    let before = clip_on(&h, clip).1;
    let from = clip_point(&h, clip, 0.75);
    let to = row_point(&h, snare, from.x);
    drag(&mut h, from, to, Modifiers::NONE);
    assert_eq!(clip_on(&h, clip).0, snare);
    let dir = std::env::temp_dir().join(format!("sml-hf10-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("moved.scraft");
    h.state_mut().run("session.save_as", json!({"path": path.to_string_lossy()})).unwrap();
    let mut e = soundcraft_engine::Engine::default();
    e.execute("session.open", &json!({"path": path.to_string_lossy()})).unwrap();
    let s = e.session();
    let snare2 = s.track_by_name("Snare").unwrap().id;
    let (t, c) = s.find_clip(clip).map(|(t, c)| (t, c.clone())).unwrap();
    assert_eq!(t, snare2);
    assert_eq!(
        (c.start, c.length, c.source(), c.fade_in, c.fade_out),
        (before.start, before.length, before.source(), before.fade_in, before.fade_out)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
