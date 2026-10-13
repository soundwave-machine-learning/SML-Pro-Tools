//! Hotfix Pass 02: portable checks of HF07–HF13 behaviour reachable from the UI layer.
use egui::vec2;
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;
use soundcraft_ui_egui::{Services, SoundApp};

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

/// HF12: starting a plugin scan from a command (the control channel's path) first writes a
/// recovery point of unsaved edits into the autosave store. The named session file is not
/// written and the session stays modified.
#[test]
fn hf12_a_scan_request_writes_a_recovery_point_first() {
    let dir = std::env::temp_dir().join(format!("sml-hf12-recovery-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.autosave_dir = Some(dir.clone());
    app.run("mix.volume", json!({"track": "Kick", "db": -7.5})).unwrap();
    assert!(app.engine.is_dirty());
    let name = app.engine.session().name.clone();
    let store = soundcraft_engine::autosave::Store::for_session(&dir, &name);
    assert!(store.index().generations.is_empty());
    // No scan service in this test process: the command reports that, after the recovery point.
    let r = app.run("engine.plugin_scan_start", json!({}));
    assert!(r.is_err());
    let idx = store.index();
    assert_eq!(idx.generations.len(), 1, "one recovery point before the scan");
    assert!(app.engine.is_dirty(), "the session is still modified (nothing was saved over it)");
    // Not again for the same revision; a legacy rescan counts as a scan request too.
    let _ = app.run("engine.vst3_plugins", json!({"rescan": true}));
    assert_eq!(store.index().generations.len(), 1);
    assert!(soundcraft_ui_egui::starts_plugin_scan("engine.clap_plugins", &json!({"rescan": true})));
    assert!(!soundcraft_ui_egui::starts_plugin_scan("engine.clap_plugins", &json!({})));
    let _ = std::fs::remove_dir_all(&dir);
}

/// HF07: Audio Setup leads with the driver type and states that a buffer's duration is not
/// round-trip latency; existing Audio Setup tests cover device drafts and Apply/Cancel.
#[test]
fn hf07_audio_setup_shows_driver_type_and_buffer_meaning() {
    let mut h = harness(1366.0, 768.0, 1.0);
    h.state_mut().ui.show_playback_engine = true;
    h.run_steps(4);
    h.get_by_label("Driver type");
    h.get_by_label("Buffer size (requested)");
    h.get_by_label("Buffer duration is one callback's length, not the total or round-trip monitoring latency.");
    let types = soundcraft_playback::device::driver_types();
    if cfg!(windows) {
        assert!(types.iter().any(|d| d.name == "WASAPI Exclusive" && !d.available));
    }
    assert!(types.iter().all(|d| d.available || !d.note.is_empty()), "every unavailable driver says why");
}

fn press_key(h: &mut Harness<'static, SoundApp>, key: egui::Key, modifiers: egui::Modifiers) {
    h.event(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.event(egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
    h.run_steps(2);
}

fn kick_db(h: &Harness<'static, SoundApp>) -> f32 {
    h.state().engine.session().track_by_name("Kick").unwrap().mixer.volume_db
}

/// HF09: the fader's gain law and automation mapping are exactly the model's (unchanged by the
/// new look): representative positions, the saved values, and round trips.
#[test]
fn hf09_fader_law_is_unchanged() {
    use soundcraft_model::{fader_db_to_pos, fader_pos_to_db};
    // Values of the law before this pass (fader_db_to_pos), to 1e-6.
    for (db, pos) in [(12.0f32, 1.0f32), (6.0, 0.875), (0.0, 0.75), (-6.0, 0.625), (-12.0, 0.5), (-24.0, 0.25), (-144.0, 0.0)] {
        assert!((fader_db_to_pos(db) - pos).abs() < 1e-6, "{db} dB → {}", fader_db_to_pos(db));
        assert!((fader_pos_to_db(pos) - db).abs() < 0.01 || db == -144.0, "{pos} → {}", fader_pos_to_db(pos));
    }
    // Opening the mixer and hovering never writes a level.
    let mut h = harness(1920.0, 1080.0, 1.0);
    h.state_mut().run("window.mix", json!({})).unwrap();
    h.state_mut().run("mix.view", json!({"view": 0})).unwrap();
    h.run_steps(4);
    let before: Vec<f32> = h.state().engine.session().tracks.iter().map(|t| t.mixer.volume_db).collect();
    let f = h.get_by_label("Kick volume").rect();
    h.hover_at(f.center());
    h.run_steps(3);
    let after: Vec<f32> = h.state().engine.session().tracks.iter().map(|t| t.mixer.volume_db).collect();
    assert_eq!(before, after);
}

/// HF09: keyboard steps, Home to unity, Alt-click to unity, typed value in the readout; each is
/// one undo step.
#[test]
fn hf09_fader_keyboard_typed_value_and_reset() {
    let mut h = harness(1920.0, 1080.0, 1.0);
    h.state_mut().run("window.mix", json!({})).unwrap();
    h.state_mut().run("mix.view", json!({"view": 0})).unwrap();
    h.run_steps(4);
    let start = kick_db(&h);
    // Focus the fader with a click on its groove away from the cap (a click alone moves nothing).
    h.get_by_label("Kick volume").click();
    h.run_steps(2);
    assert_eq!(kick_db(&h), start, "a click does not move the fader");
    press_key(&mut h, egui::Key::ArrowUp, egui::Modifiers::NONE);
    assert!((kick_db(&h) - (start + 0.5)).abs() < 1e-4, "↑ +0.5 dB: {}", kick_db(&h));
    press_key(&mut h, egui::Key::ArrowDown, egui::Modifiers::SHIFT);
    assert!((kick_db(&h) - (start + 0.4)).abs() < 1e-4, "Shift+↓ −0.1 dB: {}", kick_db(&h));
    press_key(&mut h, egui::Key::Home, egui::Modifiers::NONE);
    assert_eq!(kick_db(&h), 0.0, "Home: unity");
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert!((kick_db(&h) - (start + 0.4)).abs() < 1e-4, "one undo per step: {}", kick_db(&h));
    // Type a value.
    let ro = h.get_by_label("Kick volume readout").rect().center();
    h.event(egui::Event::PointerMoved(ro));
    for _ in 0..2 {
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: ro, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
            h.run_steps(1);
        }
    }
    h.run_steps(2);
    assert!(h.query_by_label("Kick volume readout").is_none(), "double-click opened the value field");
    h.event(egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
    h.run_steps(1);
    h.event(egui::Event::Text("-12.5".into()));
    h.run_steps(1);
    press_key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(kick_db(&h), -12.5, "typed value applied ({})", h.state().ui.status);
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert!((kick_db(&h) - (start + 0.4)).abs() < 1e-4);
    assert_eq!(soundcraft_ui_egui::mix_window::parse_db("-inf"), Some(-144.0));
    assert_eq!(soundcraft_ui_egui::mix_window::parse_db("+20"), Some(12.0));
    assert_eq!(soundcraft_ui_egui::mix_window::parse_db("loud"), None);
}

/// HF09: the same fader in Console and Split (one control, one state).
#[test]
fn hf09_console_and_split_use_the_same_fader() {
    let mut h = harness(1920.0, 1080.0, 1.0);
    h.state_mut().run("window.mix", json!({})).unwrap();
    for view in [0, 2] {
        h.state_mut().run("mix.view", json!({"view": view})).unwrap();
        h.run_steps(4);
        h.get_by_label("Kick volume");
    }
}

/// HF09: dragging the cap moves along the model's fader law; Ctrl/Cmd drags at 1/10 speed; the
/// whole drag is one undo step.
#[test]
fn hf09_fader_drag_fine_modifier_and_one_undo() {
    use soundcraft_model::fader_db_to_pos;
    let mut h = harness(1920.0, 1080.0, 1.0);
    h.state_mut().run("window.mix", json!({})).unwrap();
    h.state_mut().run("mix.view", json!({"view": 0})).unwrap();
    h.run_steps(4);
    let drag = |h: &mut Harness<'static, SoundApp>, dy: f32, mods: egui::Modifiers| {
        let f = h.get_by_label("Kick volume").rect();
        let db = kick_db(h);
        // Grab the cap where the model's law puts it (the groove leaves half a cap at each end).
        let cap_h = (f.height() * 0.11).clamp(20.0, 30.0);
        let (top, bottom) = (f.min.y + cap_h * 0.5, f.max.y - cap_h * 0.5);
        let y = bottom - (bottom - top) * fader_db_to_pos(db);
        let from = egui::pos2(f.center().x + 4.0, y);
        h.event(egui::Event::ModifiersChanged(mods));
        h.event(egui::Event::PointerMoved(from));
        h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: mods });
        h.run_steps(1);
        for k in 1..=5 {
            h.event(egui::Event::PointerMoved(from + egui::vec2(0.0, dy * k as f32 / 5.0)));
            h.run_steps(1);
        }
        h.event(egui::Event::PointerButton {
            pos: from + egui::vec2(0.0, dy),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: mods,
        });
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        h.run_steps(3);
        (bottom - top, db)
    };
    let (travel, start) = drag(&mut h, -40.0, egui::Modifiers::NONE);
    let plain = fader_db_to_pos(kick_db(&h)) - fader_db_to_pos(start);
    assert!((plain - 40.0 / travel).abs() < 0.02, "plain drag follows the pointer: {plain} vs {}", 40.0 / travel);
    let mid = kick_db(&h);
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert!((kick_db(&h) - start).abs() < 1e-4, "one drag, one undo step");
    h.state_mut().run("edit.redo", json!({})).unwrap();
    assert!((kick_db(&h) - mid).abs() < 1e-4);
    let fine_mods = if cfg!(target_os = "macos") {
        egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND
    } else {
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND
    };
    let (_, start2) = drag(&mut h, -40.0, fine_mods);
    let fine = fader_db_to_pos(kick_db(&h)) - fader_db_to_pos(start2);
    assert!((fine - plain * 0.1).abs() < 0.01, "fine drag is 1/10: {fine} vs {plain}");
}
