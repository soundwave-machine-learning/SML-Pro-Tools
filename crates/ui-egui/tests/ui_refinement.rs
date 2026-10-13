//! Focused UI refinement pass: Visual mixer labels and coincident channels, Comfortable/Compact
//! mixer density, the compact Record toolbar and empty input meters. Driven through the real
//! application; presentation must never change gain, pan or width.
use egui::accesskit::Role;
use egui::{Event, Modifiers, PointerButton, Pos2, Rect, vec2};
use egui_kittest::{Harness, kittest::NodeT, kittest::Queryable};
use serde_json::json;
use soundcraft_ui_egui::{Services, SoundApp, UiState};

fn harness(w: f32, h: f32, ppp: f32) -> Harness<'static, SoundApp> {
    let app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    let mut h = Harness::builder().with_size(vec2(w, h)).with_pixels_per_point(ppp).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

/// Every channel's gain, pan endpoints and width.
fn mix_values(h: &Harness<'_, SoundApp>) -> Vec<(String, f32, Vec<f32>, f32)> {
    h.state().engine.session().tracks.iter().map(|t| (t.name.clone(), t.mixer.volume_db, t.mixer.pan.clone(), t.mixer.width)).collect()
}

fn visual(h: &mut Harness<'_, SoundApp>) {
    h.state_mut().run("window.mix", json!({})).unwrap();
    h.state_mut().run("mix.view", json!({"view": 1})).unwrap();
    h.run_steps(6);
}

/// Label rectangles on the canvas: single-channel labels and cluster chips, with the channel
/// names each one stands for.
fn labels(h: &Harness<'_, SoundApp>) -> Vec<(Rect, Vec<String>)> {
    let mut out = Vec::new();
    for n in h.get_all_by_role(Role::Button) {
        let label = n.accesskit_node().label().map(|l| l.to_string()).unwrap_or_default();
        if let Some(name) = label.strip_suffix(" label").map(str::to_string) {
            out.push((n.rect(), vec![name]));
        }
    }
    for n in h.get_all_by_role(Role::ComboBox) {
        let l = n.accesskit_node().label().map(|l| l.to_string()).unwrap_or_default();
        if let Some((_, names)) = l.split_once(" channels at this position: ") {
            out.push((n.rect(), names.split(", ").map(str::to_string).collect()));
        }
    }
    out
}

fn drag(h: &mut Harness<'_, SoundApp>, from: Pos2, to: Pos2) {
    h.event(Event::PointerMoved(from));
    h.run_steps(1);
    h.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    for k in 1..=6 {
        h.event(Event::PointerMoved(from.lerp(to, k as f32 / 6.0)));
        h.run_steps(1);
    }
    h.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
}

#[test]
fn visual_labels_never_overlap_and_every_channel_is_named() {
    for (w, hgt, ppp) in [(1920.0, 1080.0, 1.0), (1366.0, 768.0, 1.0), (911.0, 512.0, 1.0), (960.0, 540.0, 2.0)] {
        let mut h = harness(w, hgt, ppp);
        let before = mix_values(&h);
        visual(&mut h);
        let ls = labels(&h);
        assert!(!ls.is_empty(), "{w}x{hgt}: labels drawn");
        for (i, (a, an)) in ls.iter().enumerate() {
            assert!(a.min.x >= 0.0 && a.max.x <= w + 0.5 && a.max.y <= hgt + 0.5, "{w}x{hgt}: {an:?} on screen {a:?}");
            for (b, bn) in &ls[i + 1..] {
                assert!(!a.shrink(0.5).intersects(b.shrink(0.5)), "{w}x{hgt}: {an:?} {a:?} overlaps {bn:?} {b:?}");
            }
        }
        // Every channel on the canvas is named by its own label or by a cluster chip.
        let named: Vec<String> = ls.iter().flat_map(|(_, n)| n.clone()).collect();
        for t in ["Kick", "Snare", "Hats", "Bass", "Pad", "Keys", "Lead", "Verb"] {
            assert!(named.iter().any(|n| n == t), "{w}x{hgt}: {t} named on the canvas ({named:?})");
        }
        // Hovering nodes and labels changes nothing in the mix.
        for (r, _) in &ls {
            h.event(Event::PointerMoved(r.center()));
            h.run_steps(2);
        }
        assert_eq!(mix_values(&h), before, "{w}x{hgt}: presentation never changes gain, pan or width");
    }
}

#[test]
fn coincident_channels_are_reachable_and_the_chosen_one_is_dragged() {
    let mut h = harness(1366.0, 768.0, 1.0);
    visual(&mut h);
    let before = mix_values(&h);
    // The demo has several channels at the same pan and nearby gain: a cluster chip.
    let (chip, members) = labels(&h).into_iter().find(|(_, n)| n.len() > 1).expect("a cluster of coincident channels");
    let target = members.last().unwrap().clone();
    h.event(Event::PointerMoved(chip.center()));
    h.run_steps(1);
    h.event(Event::PointerButton { pos: chip.center(), button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.event(Event::PointerButton { pos: chip.center(), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
    // The list shows every member with its values; choosing one selects only that channel.
    for m in &members {
        h.get_all_by_label_contains(&format!("{m} — ")).next().unwrap_or_else(|| panic!("{m} listed"));
    }
    assert_eq!(mix_values(&h), before, "opening the list changes nothing");
    h.get_all_by_label_contains(&format!("{target} — ")).next().unwrap().click();
    h.run_steps(4);
    let id = h.state().engine.session().track_by_name(&target).unwrap().id;
    assert_eq!(h.state().engine.session().edit.selected_tracks, vec![id], "the chosen channel is selected");
    assert_eq!(mix_values(&h), before, "selecting changes nothing");
    // The selected node is on top: a drag where the cluster sits moves the chosen channel only.
    let node = h.get_by_label(&format!("{target} node")).rect();
    let undo0 = h.state().engine.undo_depth();
    drag(&mut h, node.center(), node.center() + vec2(0.0, 40.0));
    let after = mix_values(&h);
    for (b, a) in before.iter().zip(&after) {
        if a.0 == target {
            assert!(a.1 < b.1, "{target} moved down ({} → {})", b.1, a.1);
        } else {
            assert_eq!(a, b, "{} untouched by a drag on {target}", a.0);
        }
    }
    assert_eq!(h.state().engine.undo_depth(), undo0 + 1, "one undo step");
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert_eq!(mix_values(&h), before);
}

#[test]
fn mixer_density_is_a_persisted_preference_and_compact_stays_readable() {
    let mut h = harness(1366.0, 768.0, 1.0);
    h.state_mut().run("window.mix", json!({})).unwrap();
    h.state_mut().run("mix.view", json!({"view": 0})).unwrap();
    h.run_steps(4);
    let before = mix_values(&h);
    assert_eq!(h.state_mut().run("ui.mix_density", json!({})).unwrap(), json!({"density": "comfortable"}));
    let wide = h.get_by_label("Kick strip name").rect().width();
    h.get_by_label("Compact density").click();
    h.run_steps(4);
    assert!(h.state().ui.narrow_mix, "Compact is the narrow_mix preference");
    let narrow = h.get_by_label("Kick strip name").rect().width();
    assert!(narrow < wide * 0.75, "compact strips are narrower ({wide} → {narrow})");
    // Names stay inside their strips and do not overlap the neighbours'.
    let names: Vec<Rect> =
        ["Kick", "Snare", "Hats", "Bass", "Pad", "Keys", "Lead", "Verb"].iter().map(|n| h.get_by_label(&format!("{n} strip name")).rect()).collect();
    for pair in names.windows(2) {
        assert!(pair[0].max.x <= pair[1].min.x, "names do not overlap {pair:?}");
    }
    // Same preference through View › Narrow Mix and the command; bad values are refused.
    h.state_mut().run("window.narrow_mix", json!({})).unwrap();
    assert_eq!(h.state_mut().run("ui.mix_density", json!({})).unwrap(), json!({"density": "comfortable"}));
    assert!(h.state_mut().run("ui.mix_density", json!({"density": "tiny"})).is_err());
    h.state_mut().run("ui.mix_density", json!({"density": "compact"})).unwrap();
    let saved: UiState = serde_json::from_str(&serde_json::to_string(&h.state().ui).unwrap()).unwrap();
    assert!(saved.narrow_mix, "density survives a save/load of the preferences");
    assert_eq!(mix_values(&h), before, "density never changes the mix");
}

/// Updated by Hotfix Pass 02 (HF08), same intent: in the Record layout the transport and Nudge
/// stay reachable and never overlap the tools. The scrolling/stacked toolbar this test first
/// checked is gone: the tools are in a fixed top row, the transport and Nudge in the fixed
/// bottom bar, and what does not fit is in a labelled overflow menu instead of a scroll strip.
#[test]
fn compact_record_toolbar_keeps_nudge_and_transport_reachable() {
    // 911×512 = 1366×768 at 150 %; 640×400 = the minimum window (960×600) at 150 %.
    for (w, hgt) in [(911.0, 512.0), (640.0, 400.0), (1366.0, 768.0)] {
        let mut h = harness(w, hgt, 1.0);
        h.state_mut().run("window.record", json!({})).unwrap();
        h.run_steps(8);
        let l = h.state().toolbar_layout;
        assert!(
            l.transport.width() > 20.0 && l.transport.min.x >= 0.0 && l.transport.max.x <= w + 0.5,
            "{w}x{hgt}: transport in the bottom bar {l:?}"
        );
        assert!(l.transport.min.y >= l.bottom.min.y - 0.5 && l.transport.max.y <= hgt + 0.5, "{w}x{hgt}: transport at the bottom {l:?}");
        if w >= 900.0 {
            assert!(l.nudge.width() > 20.0 && l.nudge.max.x <= w + 0.5, "{w}x{hgt}: Nudge in the bar {l:?}");
            assert!(!l.transport.intersects(l.nudge.shrink(1.0)), "{w}x{hgt}: no overlap");
            assert!(l.tools.width() > 20.0 && l.tools.max.y <= l.visible.max.y + 0.5, "{w}x{hgt}: tools in the top row {l:?}");
        } else {
            // The minimum window: Nudge and the tools are one click away, in the labelled menus.
            h.get_by_label("More controls");
            h.get_by_label("More tools");
        }
        // Stop / Play / Record are named buttons wherever they are drawn.
        h.get_all_by_label("▶ Play").next().expect("Play");
        h.get_all_by_label("● Record").next().expect("Record");
    }
}

#[test]
fn empty_input_meters_say_no_signal_and_draw_no_level() {
    let mut h = harness(1366.0, 768.0, 1.0);
    h.state_mut().run("window.record", json!({})).unwrap();
    h.state_mut().run("edit.select", json!({"tracks": ["Kick"]})).unwrap();
    h.run_steps(4);
    let bars: Vec<String> = h.get_all_by_role(Role::ProgressIndicator).filter_map(|n| n.accesskit_node().label().map(|l| l.to_string())).collect();
    assert!(bars.iter().any(|b| b == "input: no signal (input device not open)"), "the Record panel says why there is no level: {bars:?}");
    assert!(bars.iter().all(|b| !b.contains("dBFS")), "no level is shown without an input: {bars:?}");
}

/// OPEN FUNCTIONAL DEFECT K1 — reproduction, not a specification. Turning on "A/B: listen to
/// reference" stores `mastering.reference.listening = true`, but nothing feeds the reference to
/// the monitor-only path that exists for it (`soundcraft_playback::AuxMonitor`: no `push`, no
/// `solo`, no gain), so no reference audio can be heard. This test documents that state; when
/// the audio path is repaired it fails, and must then be replaced by a test of the repaired
/// behaviour (docs/refinement_pass_01/REFERENCE_AB_DEFECT.md).
#[test]
fn defect_k1_reference_ab_feeds_no_audio_to_the_monitor_path() {
    let mut engine = soundcraft_engine::demo::demo_engine();
    let src = engine.session().tracks.iter().flat_map(|t| t.playlists.iter().flat_map(|p| &p.clips)).find_map(|c| c.source()).unwrap();
    engine.execute("mastering.reference", &json!({"source": src.0})).unwrap();
    let player = soundcraft_playback::Player::new(engine.session_arc());
    let app = SoundApp::new(engine, Some(player), Services::default());
    let mut h = Harness::builder().with_size(vec2(1366.0, 768.0)).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.state_mut().run("window.master", json!({})).unwrap();
    h.run_steps(4);
    // The user's path: the Master layout's A/B toggle, then Play.
    h.get_by_label("A/B: listen to reference").click();
    h.run_steps(3);
    assert_eq!(h.state().engine.session().mastering.reference.as_ref().map(|r| r.listening), Some(true), "the setting is stored");
    h.state_mut().run("transport.play", json!({})).unwrap();
    h.run_steps(30);
    let aux = h.state().player.as_ref().unwrap().aux();
    assert_eq!(aux.queued_frames(), 0, "DEFECT K1: no reference audio is queued for the monitor output");
    assert!(!aux.solo.load(std::sync::atomic::Ordering::Relaxed), "DEFECT K1: the program is not switched off for the A side");
    // The defect is visible on screen, not presented as working.
    h.get_all_by_label_contains("OPEN DEFECT").next().expect("the Master card says it is an open defect");
    h.state_mut().run("transport.stop", json!({})).unwrap();
}
