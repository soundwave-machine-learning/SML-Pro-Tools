//! HF02 + HF04: the "+" beside TRACKS opens the existing New Tracks dialog; its Name field has
//! a full-width row; creation is one undoable step that never reroutes other tracks.
use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, vec2};
use egui_kittest::{Harness, kittest::NodeT, kittest::Queryable};
use serde_json::json;
use soundcraft_model::{ChannelFormat, Route, TrackKind};
use soundcraft_ui_egui::dialogs::Dialog;
use soundcraft_ui_egui::{Services, SoundApp};

fn harness(w: f32, h: f32, ppp: f32, demo: bool) -> Harness<'static, SoundApp> {
    let engine = if demo { soundcraft_engine::demo::demo_engine() } else { soundcraft_engine::Engine::default() };
    let app = SoundApp::new(engine, None, Services::default());
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

fn key(h: &mut Harness<'_, SoundApp>, k: Key, modifiers: Modifiers) {
    h.event(Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers });
    h.event(Event::Key { key: k, physical_key: None, pressed: false, repeat: false, modifiers });
    h.run_steps(2);
}

fn open_with_plus(h: &mut Harness<'_, SoundApp>) {
    h.get_by_label("Add Track").click();
    h.run_steps(3);
    assert!(matches!(h.state().dialogs.open, Some(Dialog::NewTracks { .. })), "+ opens the New Tracks dialog");
}

fn name_field(h: &Harness<'_, SoundApp>) -> egui::Rect {
    h.get_by_label("Name:").rect()
}

#[test]
fn plus_is_visible_in_every_layout_and_size() {
    for (w, hgt, ppp) in [(911.0, 512.0, 1.0), (1920.0, 1080.0, 1.0), (960.0, 540.0, 2.0)] {
        for demo in [true, false] {
            let mut h = harness(w, hgt, ppp, demo);
            for win in ["window.edit", "window.mix", "window.record"] {
                h.state_mut().run(win, json!({})).unwrap();
                h.run_steps(4);
                let n = h.get_all_by_label("Add Track").next().map(|n| n.rect());
                let r = n.unwrap_or_else(|| panic!("{w}x{hgt} {win} demo={demo}: no Add Track button"));
                assert!(r.width() >= 18.0 && r.min.x >= 0.0 && r.max.x <= w && r.max.y <= hgt, "{w}x{hgt} {win}: on screen {r:?}");
            }
        }
    }
}

#[test]
fn name_field_is_wide_and_stays_inside_the_dialog() {
    for (w, hgt, ppp) in [(911.0, 512.0, 1.0), (1920.0, 1080.0, 1.0), (960.0, 540.0, 2.0)] {
        let mut h = harness(w, hgt, ppp, true);
        h.state_mut().run("window.edit", json!({})).unwrap();
        h.run_steps(3);
        open_with_plus(&mut h);
        let name = name_field(&h);
        let create = h.get_by_role_and_label(Role::Button, "Create").rect();
        assert!(name.width() >= 250.0, "{w}x{hgt}: Name field is usable ({name:?})");
        assert!(name.max.x <= w && name.min.x >= 0.0, "{w}x{hgt}: Name inside the window");
        // The approved controls stay: count, format, kind, timebase, Create and Cancel.
        h.get_by_role_and_label(Role::Button, "Cancel");
        let combos: Vec<String> = h.get_all_by_role(Role::ComboBox).filter_map(|n| n.value()).collect();
        for v in ["Mono", "Audio Track", "Samples"] {
            assert!(combos.iter().any(|c| c == v), "{w}x{hgt}: {v} choice present in {combos:?}");
        }
        assert!(create.max.y <= hgt, "{w}x{hgt}: buttons reachable");
    }
}

#[test]
fn long_unicode_name_is_created_in_full_once_and_escape_changes_nothing() {
    let mut h = harness(1366.0, 768.0, 1.0, true);
    h.state_mut().run("window.edit", json!({})).unwrap();
    h.run_steps(3);
    let before = h.state().engine.session().clone();
    // Escape: no change at all.
    open_with_plus(&mut h);
    h.get_by_label("Name:").focus();
    h.run_steps(1);
    h.event(Event::Text("Scratch".into()));
    h.run_steps(1);
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert!(h.state().dialogs.open.is_none(), "Escape closes");
    assert_eq!(*h.state().engine.session(), before, "Escape mutates nothing");
    // Typing letters that are shortcuts elsewhere (m, s, r) stays in the field.
    open_with_plus(&mut h);
    h.get_by_label("Name:").focus();
    h.run_steps(1);
    key(&mut h, Key::A, Modifiers::COMMAND);
    key(&mut h, Key::Backspace, Modifiers::NONE);
    let long = "Lead Vocal — Ünïcødé ✓ double-tracked chorus take with a very long descriptive name";
    h.event(Event::Text(long.into()));
    h.run_steps(2);
    assert_eq!(h.state().engine.session().tracks.len(), before.tracks.len(), "typing creates nothing");
    assert_eq!(h.state().engine.session().edit.selected_tracks, before.edit.selected_tracks, "typing edits nothing");
    let undo0 = h.state().engine.undo_depth();
    key(&mut h, Key::Enter, Modifiers::NONE);
    h.run_steps(3);
    let s = h.state().engine.session();
    assert_eq!(s.tracks.len(), before.tracks.len() + 1, "Enter creates exactly one track");
    assert!(s.tracks.iter().any(|t| t.name == long && t.kind == TrackKind::Audio && t.format == ChannelFormat::Mono), "full name stored");
    assert_eq!(h.state().engine.undo_depth(), undo0 + 1);
    assert!(h.state().dialogs.open.is_none());
}

#[test]
fn batch_is_one_undo_step_and_bus_creation_reroutes_nothing() {
    let mut h = harness(1366.0, 768.0, 1.0, true);
    h.state_mut().run("window.edit", json!({})).unwrap();
    h.run_steps(3);
    let before = h.state().engine.session().clone();
    let routes: Vec<Route> = before.tracks.iter().map(|t| t.mixer.output.clone()).collect();
    // A selection exists: an ordinary bus must not take those tracks over.
    h.state_mut().run("edit.select", json!({"tracks": ["Kick", "Snare"]})).unwrap();
    open_with_plus(&mut h);
    // Choose Aux Input through the dialog's own combo: the format becomes Stereo.
    let kind_combo =
        |h: &Harness<'_, SoundApp>| h.get_all_by_role(Role::ComboBox).find(|n| n.value().as_deref() == Some("Audio Track")).map(|n| n.rect());
    let r = kind_combo(&h).expect("kind combo");
    h.event(Event::PointerMoved(r.center()));
    h.event(Event::PointerButton { pos: r.center(), button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.event(Event::PointerButton { pos: r.center(), button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    h.get_by_label("Aux Input").click();
    h.run_steps(2);
    match &h.state().dialogs.open {
        Some(Dialog::NewTracks { kind, format, .. }) => {
            assert_eq!(kind, "aux");
            assert_eq!(format, "Stereo", "buses default to stereo");
        }
        other => panic!("dialog closed: {other:?}"),
    }
    // Master is offered only when none exists (the demo has one).
    let r = h.get_all_by_role(Role::ComboBox).find(|n| n.value().as_deref() == Some("Aux Input")).map(|n| n.rect()).expect("kind combo");
    h.event(Event::PointerButton { pos: r.center(), button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.event(Event::PointerButton { pos: r.center(), button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    let master = h.get_by_label("Master Fader");
    assert!(master.accesskit_node().is_disabled(), "a second Main is not offered");
    key(&mut h, Key::Escape, Modifiers::NONE);
    // Three buses in one Create.
    if let Some(Dialog::NewTracks { count, .. }) = &mut h.state_mut().dialogs.open {
        *count = 3;
    } else {
        h.state_mut().dialogs.open =
            Some(Dialog::NewTracks { count: 3, format: "Stereo".into(), kind: "aux".into(), timebase: "samples".into(), name: "Vocal Bus".into() });
    }
    h.run_steps(2);
    let undo0 = h.state().engine.undo_depth();
    h.get_by_role_and_label(Role::Button, "Create").click();
    h.run_steps(3);
    let s = h.state().engine.session().clone();
    let new: Vec<_> = s.tracks.iter().filter(|t| !before.tracks.iter().any(|b| b.id == t.id)).collect();
    assert_eq!(new.len(), 3, "count honoured");
    assert!(new.iter().all(|t| t.kind == TrackKind::Aux && t.format == ChannelFormat::Stereo));
    let names: Vec<&str> = new.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names.iter().collect::<std::collections::BTreeSet<_>>().len(), 3, "distinct deterministic names {names:?}");
    assert_eq!(h.state().engine.undo_depth(), undo0 + 1, "one undo step for the batch");
    for (b, r) in before.tracks.iter().zip(&routes) {
        assert_eq!(&s.track(b.id).unwrap().mixer.output, r, "{} keeps its output", b.name);
    }
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert_eq!(h.state().engine.session().tracks.len(), before.tracks.len(), "undo removes the whole batch");
    h.state_mut().run("edit.redo", json!({})).unwrap();
    assert_eq!(h.state().engine.session().tracks.len(), before.tracks.len() + 3, "redo recreates it");
    // Save and reopen keep kind, format and names.
    let dir = std::env::temp_dir().join(format!("sml-hf02-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("hf02.scraft");
    h.state_mut().run("session.save_as", json!({"path": path.to_string_lossy()})).unwrap();
    let mut e = soundcraft_engine::Engine::default();
    soundcraft_engine::io::open_session(&mut e, path.to_str().unwrap()).unwrap();
    for t in &new {
        let r = e.session().track_by_name(&t.name).expect("reopened");
        assert_eq!((r.kind, r.format), (TrackKind::Aux, ChannelFormat::Stereo));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shortcut_and_menu_open_the_same_dialog() {
    let mut h = harness(1366.0, 768.0, 1.0, true);
    h.state_mut().run("window.edit", json!({})).unwrap();
    h.run_steps(3);
    key(&mut h, Key::N, Modifiers { command: true, ctrl: !cfg!(target_os = "macos"), shift: true, ..Modifiers::NONE });
    assert!(matches!(h.state().dialogs.open, Some(Dialog::NewTracks { .. })), "Ctrl+Shift+N opens New Tracks");
    key(&mut h, Key::Escape, Modifiers::NONE);
    soundcraft_ui_egui::menus::invoke_menu(h.state_mut(), "track.new", "Track > New...");
    h.run_steps(2);
    assert!(matches!(h.state().dialogs.open, Some(Dialog::NewTracks { .. })), "Track › New... opens the same dialog");
}
