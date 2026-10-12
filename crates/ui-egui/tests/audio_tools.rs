//! T-T1 / T-T4 / T-T5 (EDIT_TOOLBAR_SPEC): the eight audio tools exist as labelled buttons,
//! dispatch the shared engine commands from real pointer gestures, affect exactly the intended
//! clip ids, give one undo per gesture, and Escape rolls a sweep back. Headless (egui_kittest);
//! native feel and scaling are the owner's gate (T-T6).
use egui::{Event, Key, Modifiers, PointerButton, pos2, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;
use soundcraft_audio_io::AudioBuffer;
use soundcraft_model::{ChannelFormat, Clip, ClipId, Session, SourceAudio, SourceId, Tool, TrackId, TrackKind};
use soundcraft_ui_egui::{Services, SoundApp, edit_window};
use std::sync::Arc;

const SR: u32 = 48_000;

fn session() -> (Session, TrackId, Vec<ClipId>) {
    let mut s = Session::default();
    let tone: Vec<f32> = (0..SR as usize * 2).map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / SR as f32).sin() * 0.5).collect();
    s.pool.insert(SourceId(1), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: SR, channels: vec![tone] })));
    let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Lead"));
    s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Double"));
    let mut ids = Vec::new();
    for (start, len) in [(0, 48_000), (60_000, 48_000)] {
        let id = s.new_clip_id();
        s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "take", SourceId(1), 0, start, len));
        ids.push(id);
    }
    s.edit.selected_tracks = vec![t];
    s.edit.tool = Tool::Arrow;
    // Fit the two clips into the window.
    s.edit.zoom.samples_per_px = 120.0;
    (s, t, ids)
}

fn harness() -> (Harness<'static, SoundApp>, TrackId, Vec<ClipId>) {
    let (s, t, ids) = session();
    let app = SoundApp::new(soundcraft_engine::Engine::new(s), None, Services::default());
    let mut h = Harness::builder().with_size(vec2(1500.0, 900.0)).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(6);
    (h, t, ids)
}

/// Screen position of `at` samples on track `t`, at the given fraction of the lane height.
fn at(h: &Harness<'_, SoundApp>, t: TrackId, at: i64, frac: f32) -> egui::Pos2 {
    let app = h.state();
    let tl = app.edit_layout.timeline;
    let row = app.edit_layout.rows.iter().find(|(id, _)| *id == t.0).map(|(_, r)| *r).expect("track row");
    let x = edit_window::x_of(app.engine.session(), egui::Rect::from_min_max(pos2(tl[0], tl[1]), pos2(tl[2], tl[3])), at);
    pos2(x, row[1] + (row[3] - row[1]) * frac)
}

fn press(h: &mut Harness<'_, SoundApp>, p: egui::Pos2, mods: Modifiers) {
    h.event(Event::PointerMoved(p));
    h.run_steps(2);
    h.event(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: mods });
    h.run_steps(2);
}

fn release(h: &mut Harness<'_, SoundApp>, p: egui::Pos2, mods: Modifiers) {
    h.event(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: mods });
    h.run_steps(4);
}

fn click(h: &mut Harness<'_, SoundApp>, p: egui::Pos2, mods: Modifiers) {
    press(h, p, mods);
    release(h, p, mods);
}

fn drag(h: &mut Harness<'_, SoundApp>, from: egui::Pos2, to: egui::Pos2) {
    press(h, from, Modifiers::NONE);
    for step in 1..=10 {
        h.event(Event::PointerMoved(from.lerp(to, step as f32 / 10.0)));
        h.run_steps(1);
    }
    release(h, to, Modifiers::NONE);
}

fn clips(h: &Harness<'_, SoundApp>, t: TrackId) -> Vec<(i64, i64, bool)> {
    h.state().engine.session().track(t).unwrap().clips().iter().map(|c| (c.start, c.length, c.muted)).collect()
}

fn pick(h: &mut Harness<'_, SoundApp>, tool: Tool) {
    let label = tool.label();
    h.get_by_label(label).click();
    h.run_steps(3);
    assert_eq!(h.state().engine.session().edit.tool, tool, "toolbar button selects {}", tool.label());
}

#[test]
fn eight_tools_are_visible_and_dispatch_real_commands() {
    let (mut h, t, ids) = harness();
    // T01: all eight buttons exist by their accessible names, in one row.
    for tool in Tool::AUDIO {
        h.get_by_label(tool.label());
    }
    let undo0 = h.state().engine.undo_depth();
    // Split (T04): one click, exactly one clip split, one undo.
    pick(&mut h, Tool::Split);
    let q = at(&h, t, 24_000, 0.5);
    click(&mut h, q, Modifiers::NONE);
    assert_eq!(clips(&h, t), vec![(0, 24_000, false), (24_000, 24_000, false), (60_000, 48_000, false)]);
    assert_eq!(h.state().engine.undo_depth(), undo0 + 1);
    // Mute (T07): a sweep over both halves mutes both; a second pass over the same clip within
    // the sweep cannot toggle it twice; one undo.
    pick(&mut h, Tool::Mute);
    let (a, b) = (at(&h, t, 2_000, 0.5), at(&h, t, 46_000, 0.5));
    drag(&mut h, a, b);
    assert_eq!(
        clips(&h, t).iter().map(|c| c.2).collect::<Vec<_>>(),
        vec![true, true, false],
        "rows={:?} status={:?}",
        h.state().edit_layout.rows,
        h.state().ui.status
    );
    assert_eq!(h.state().engine.undo_depth(), undo0 + 2);
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert_eq!(clips(&h, t).iter().map(|c| c.2).collect::<Vec<_>>(), vec![false, false, false]);
    // Eraser (T05): Escape during the sweep commits nothing; a completed sweep removes exactly
    // the swept instances and keeps the media; one undo restores the whole gesture.
    pick(&mut h, Tool::Eraser);
    let (from, to) = (at(&h, t, 2_000, 0.5), at(&h, t, 46_000, 0.5));
    press(&mut h, from, Modifiers::NONE);
    for step in 1..=6 {
        h.event(Event::PointerMoved(from.lerp(to, step as f32 / 6.0)));
        h.run_steps(1);
    }
    h.event(Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    release(&mut h, to, Modifiers::NONE);
    assert_eq!(clips(&h, t).len(), 3, "Escape rolled the sweep back");
    let depth = h.state().engine.undo_depth();
    let (a, b) = (at(&h, t, 2_000, 0.5), at(&h, t, 46_000, 0.5));
    drag(&mut h, a, b);
    assert_eq!(clips(&h, t), vec![(60_000, 48_000, false)]);
    assert!(h.state().engine.session().pool.get(SourceId(1)).is_some(), "media kept");
    assert_eq!(h.state().engine.undo_depth(), depth + 1);
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert_eq!(clips(&h, t).len(), 3);
    // Range (T03): a drag selects a time range on the track, nothing else changes.
    pick(&mut h, Tool::Range);
    let before = h.state().engine.session_arc();
    let (a, b) = (at(&h, t, 70_000, 0.5), at(&h, t, 90_000, 0.5));
    drag(&mut h, a, b);
    let sel = h.state().engine.session().edit.selection;
    assert!(sel.start >= 69_000 && sel.start <= 71_000 && sel.end >= 89_000 && sel.end <= 91_000, "{sel:?}");
    assert_eq!(h.state().engine.session().tracks, before.tracks, "selection only");
    // Arrow (T02): a drag on a clip body moves it (object semantics), one undo.
    pick(&mut h, Tool::Arrow);
    h.state_mut().run("edit.tool_link", json!({"value": false})).unwrap();
    let depth = h.state().engine.undo_depth();
    let (a, b) = (at(&h, t, 80_000, 0.7), at(&h, t, 90_000, 0.7));
    drag(&mut h, a, b);
    let moved = clips(&h, t)[2].0;
    assert!(moved > 60_000 && moved < 80_000, "clip moved right by the drag: {moved}");
    assert_eq!(h.state().engine.undo_depth(), depth + 1);
    let _ = ids;
}

#[test]
fn paint_targets_the_shown_lane_or_the_clip_gain_envelope() {
    let (mut h, t, ids) = harness();
    pick(&mut h, Tool::Paint);
    h.state_mut().run("edit.paint_shape", json!({"shape": "line"})).unwrap();
    // Waveform view: Paint draws the clip-gain envelope of the clip under the press.
    let depth = h.state().engine.undo_depth();
    let (a, b) = (at(&h, t, 4_000, 0.8), at(&h, t, 40_000, 0.2));
    drag(&mut h, a, b);
    let c = h.state().engine.session().find_clip(ids[0]).unwrap().1.clone();
    assert!(c.gain_env.len() >= 2, "{:?}", c.gain_env);
    assert!(c.gain_env.first().unwrap().1 < c.gain_env.last().unwrap().1, "rises with the drag");
    assert!(h.state().engine.session().track(t).unwrap().automation.is_empty(), "no mixer lane written from a clip");
    assert_eq!(h.state().engine.undo_depth(), depth + 1);
    // Automation view: Paint writes the lane and leaves the envelope alone.
    h.state_mut().run("track.view", json!({"tracks": [t.0], "view": "volume"})).unwrap();
    h.run_steps(3);
    let (a, b) = (at(&h, t, 70_000, 0.3), at(&h, t, 100_000, 0.6));
    drag(&mut h, a, b);
    let s = h.state().engine.session();
    let lane = s.track(t).unwrap().automation.iter().find(|l| l.param == soundcraft_model::AutoParam::Volume).expect("volume lane");
    assert!(lane.points.len() >= 2);
    assert_eq!(s.find_clip(ids[0]).unwrap().1.gain_env, c.gain_env, "clip envelope untouched");
    let first_at = lane.points.first().unwrap().at;
    // Transform: the drag opens the numeric box; Apply shifts only the boxed points.
    h.state_mut().run("edit.paint_shape", json!({"shape": "transform"})).unwrap();
    let (a, b) = (at(&h, t, 65_000, 0.5), at(&h, t, 105_000, 0.5));
    drag(&mut h, a, b);
    assert!(h.state().transform.is_some(), "the Transform box opened");
    h.state_mut().transform.as_mut().unwrap().time_shift = 480.0;
    h.run_steps(2);
    h.get_by_label("Apply").click();
    h.run_steps(3);
    let s = h.state().engine.session();
    let lane = s.track(t).unwrap().automation.iter().find(|l| l.param == soundcraft_model::AutoParam::Volume).unwrap();
    assert_eq!(lane.points.first().unwrap().at, first_at + 480);
    assert!(h.state().transform.is_none());
}

#[test]
fn bend_anchors_are_added_moved_removed_and_rendered() {
    let (mut h, t, ids) = harness();
    pick(&mut h, Tool::Bend);
    let q = at(&h, t, 24_000, 0.5);
    click(&mut h, q, Modifiers::NONE);
    let c = h.state().engine.session().find_clip(ids[0]).unwrap().1.clone();
    assert_eq!(c.warp.len(), 1, "one anchor at the click: status={:?}", h.state().ui.status);
    assert!((c.warp[0].dst - 24_000).abs() < 400, "{:?}", c.warp);
    assert_eq!(c.warp[0].src, c.warp[0].dst, "a fresh anchor maps source to itself");
    // Drag the anchor right: destination moves, source stays, the render lengthens the clip.
    let x = at(&h, t, c.start + c.warp[0].dst, 0.5);
    let x2 = pos2(x.x + 60.0, x.y);
    drag(&mut h, x, x2);
    let c2 = h.state().engine.session().find_clip(ids[0]).unwrap().1.clone();
    assert_eq!(c2.warp[0].src, c.warp[0].src);
    assert!(c2.warp[0].dst > c.warp[0].dst + 5_000, "{:?}", c2.warp);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while h.state().engine.warp.is_busy() && std::time::Instant::now() < deadline {
        h.run_steps(2);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let c3 = h.state().engine.session().find_clip(ids[0]).unwrap().1.clone();
    assert_eq!(c3.length, 48_000 + (c2.warp[0].dst - c2.warp[0].src), "rendered length follows the map");
    assert_ne!(c3.source(), Some(SourceId(1)), "plays a derived source; the original is untouched");
    assert_eq!(h.state().engine.session().pool.get(SourceId(1)).unwrap().frames(), 96_000);
    // Alt-click removes the anchor and restores the original material.
    let x = at(&h, t, c3.start + c2.warp[0].dst, 0.5);
    h.event(Event::ModifiersChanged(Modifiers::ALT));
    h.run_steps(1);
    click(&mut h, x, Modifiers::ALT);
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(1);
    let c4 = h.state().engine.session().find_clip(ids[0]).unwrap().1.clone();
    assert!(c4.warp.is_empty());
    assert_eq!((c4.length, c4.source()), (48_000, Some(SourceId(1))));
}

#[test]
fn listen_runs_while_held_and_cleans_up_on_release_or_escape() {
    let (mut h, t, _) = harness();
    pick(&mut h, Tool::Listen);
    let cursor = h.state().engine.transport.position;
    let q = at(&h, t, 24_000, 0.5);
    press(&mut h, q, Modifiers::NONE);
    {
        let app = h.state();
        assert!(app.engine.session().listen.active(), "audition started on the press: status={:?} gesture={:?}", app.ui.status, app.gesture);
        assert!(app.engine.transport.playing);
        let flags = app.engine.session().track(t).unwrap();
        assert!(!flags.mixer.solo && !flags.mixer.mute && !flags.mixer.record_arm, "no saved flag changed");
    }
    let q = at(&h, t, 24_000, 0.5);
    release(&mut h, q, Modifiers::NONE);
    let app = h.state();
    assert!(!app.engine.session().listen.active());
    assert!(!app.engine.transport.playing);
    assert_eq!(app.engine.transport.position, cursor, "cursor restored while still owned");
    assert_eq!(app.engine.undo_depth(), 0, "no undo entry");
    // Escape while held.
    let q = at(&h, t, 24_000, 0.5);
    press(&mut h, q, Modifiers::NONE);
    assert!(h.state().engine.session().listen.active());
    h.event(Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
    assert!(!h.state().engine.session().listen.active());
    let q = at(&h, t, 24_000, 0.5);
    release(&mut h, q, Modifiers::NONE);
    assert!(!h.state().engine.transport.playing);
    // A running transport refuses Listen with a visible reason.
    h.state_mut().run("transport.play", json!({})).unwrap();
    h.run_steps(2);
    let q = at(&h, t, 24_000, 0.5);
    press(&mut h, q, Modifiers::NONE);
    assert!(!h.state().engine.session().listen.active());
    assert!(h.state().ui.status.contains("unavailable"), "{}", h.state().ui.status);
    let q = at(&h, t, 24_000, 0.5);
    release(&mut h, q, Modifiers::NONE);
}

/// T-T5: Link/alternate hit zones and the compact-track fallback (pure zone logic).
#[test]
fn link_and_alternate_zones() {
    use edit_window::effective_tool;
    let lane = egui::Rect::from_min_size(pos2(0.0, 100.0), vec2(800.0, 80.0));
    let compact = egui::Rect::from_min_size(pos2(0.0, 100.0), vec2(800.0, 30.0));
    let clip = Some((200.0, 600.0));
    assert_eq!(effective_tool(Tool::Arrow, true, None, false, pos2(400.0, 110.0), lane, clip), Tool::Range, "upper half ranges");
    assert_eq!(effective_tool(Tool::Arrow, true, None, false, pos2(400.0, 170.0), lane, clip), Tool::Grabber, "lower half grabs");
    assert_eq!(effective_tool(Tool::Arrow, false, None, false, pos2(400.0, 110.0), lane, clip), Tool::Grabber, "unlinked: objects everywhere");
    assert_eq!(effective_tool(Tool::Arrow, true, None, false, pos2(203.0, 110.0), lane, clip), Tool::Trim, "edges trim first");
    assert_eq!(effective_tool(Tool::Arrow, true, None, false, pos2(400.0, 105.0), compact, clip), Tool::Grabber, "compact track: no range zone");
    assert_eq!(effective_tool(Tool::Arrow, true, Some(Tool::Eraser), true, pos2(400.0, 110.0), lane, clip), Tool::Eraser, "alternate while held");
    assert_eq!(effective_tool(Tool::Arrow, true, Some(Tool::Eraser), false, pos2(400.0, 110.0), lane, clip), Tool::Range, "released: back to Arrow");
    assert_eq!(
        effective_tool(Tool::Range, true, Some(Tool::Eraser), true, pos2(400.0, 110.0), lane, clip),
        Tool::Range,
        "alternate applies to Arrow only"
    );
    assert_eq!(effective_tool(Tool::Split, true, None, false, pos2(400.0, 170.0), lane, clip), Tool::Split);
}
