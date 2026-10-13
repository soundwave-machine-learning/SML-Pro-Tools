//! Hotfix Pass 03: portable checks of HF15–HF22 behaviour reachable from the app layer.
use serde_json::json;
use soundcraft_ui_egui::{Services, SoundApp};

/// HF18: while a take is being recorded, every app entry that replaces the session or recovers
/// over it is refused — the commands (menus, shortcuts, control channel, assistant all run
/// through `SoundApp::run`), Recover Autosave's "Open as unsaved session" and "Recover to a new
/// track". The refusal changes nothing: the session, its undo history, the dirty flag and the
/// recording state stay as they were. After the take is stopped, recovery works.
#[test]
fn hf18_recording_blocks_every_session_replacement_entry() {
    let dir = std::env::temp_dir().join(format!("sml-hf18-app-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.autosave_dir = Some(dir.join("autosave"));
    let saved = dir.join("demo.scraft");
    app.run("session.save_as", json!({"path": saved.to_string_lossy()})).unwrap();
    app.run("mix.volume", json!({"track": "Kick", "db": -9.0})).unwrap();
    assert_eq!(app.recovery_snapshot_now().unwrap().map(|_| ()), Some(()));
    let point = app.recovery_points().into_iter().next().expect("a recovery point");
    // A capture journal that looks unfinished (as the take being written does).
    let media = dir.join("Take_01.wav");
    std::fs::write(&media, b"RIFF").unwrap();

    app.engine.transport.recording = true;
    let doc = app.engine.session_arc();
    let (rev, depth, dirty) = (app.engine.revision, app.engine.undo_depth(), app.engine.is_dirty());
    for (id, p) in [
        ("session.new", json!({})),
        ("session.open", json!({"path": saved.to_string_lossy()})),
        ("session.close", json!({})),
        ("session.revert", json!({})),
    ] {
        let err = app.run(id, p).unwrap_err();
        assert!(err.contains("refused while recording"), "{id}: {err}");
    }
    let err = app.open_recovery_point(&point).unwrap_err();
    assert!(err.contains("refused while recording"), "{err}");
    let take = soundcraft_playback::capture::UnfinishedTake {
        media: media.clone(),
        journal: serde_json::from_value(json!({"media": media.to_string_lossy(), "sample_rate": 48000, "channels": 1, "durable_frames": 0, "state": "Recording", "generation": null, "first_stream_frame": null, "error": null})).unwrap(),
    };
    let err = app.recover_take(&take).unwrap_err();
    assert!(err.contains("refused while recording"), "{err}");
    assert!(std::sync::Arc::ptr_eq(&doc, &app.engine.session_arc()), "session unchanged");
    assert_eq!((app.engine.revision, app.engine.undo_depth(), app.engine.is_dirty(), app.engine.transport.recording), (rev, depth, dirty, true));
    assert_eq!(std::fs::read(&media).unwrap(), b"RIFF", "the file being written is untouched");

    // Finished: deliberate recovery opens the recovery point as an unsaved session.
    app.engine.transport.recording = false;
    app.open_recovery_point(&point).unwrap();
    assert!(app.engine.path.is_none() && app.engine.is_dirty());
    assert_eq!(app.engine.session().track_by_name("Kick").unwrap().mixer.volume_db, -9.0);
    let _ = std::fs::remove_dir_all(&dir);
}

fn harness(w: f32, h: f32, ppp: f32) -> egui_kittest::Harness<'static, SoundApp> {
    let app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(w, h)).with_pixels_per_point(ppp).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

/// A real pointer click at `p`.
fn click_at(h: &mut egui_kittest::Harness<'static, SoundApp>, p: egui::Pos2) {
    h.event(egui::Event::PointerMoved(p));
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        h.step();
    }
    h.run_steps(2);
}

/// The menu item whose accessible name starts with `prefix` (items carry a shortcut column).
fn item_rect(h: &egui_kittest::Harness<'static, SoundApp>, prefix: &str) -> Option<(egui::Rect, bool)> {
    use egui_kittest::kittest::{NodeT, Queryable};
    // Exact name (a trailing empty shortcut column is trimmed) first, else a name followed by
    // its shortcut ("Command Help F1"); never a different control that only shares the prefix.
    let nodes: Vec<_> = h.get_all_by_role(egui::accesskit::Role::Button).collect();
    nodes
        .iter()
        .find(|n| n.accesskit_node().label().is_some_and(|l| l.trim_end() == prefix))
        .or_else(|| {
            nodes.iter().find(|n| {
                n.accesskit_node().label().is_some_and(|l| {
                    l.strip_prefix(prefix)
                        .is_some_and(|rest| rest.starts_with(' ') && rest.split_whitespace().count() == 1 && rest.trim().len() <= 12)
                })
            })
        })
        .map(|n| (n.rect(), n.accesskit_node().is_disabled()))
}

/// Opens `menu` in the menu bar and clicks `item`.
fn menu_click(h: &mut egui_kittest::Harness<'static, SoundApp>, menu: &str, item: &str) {
    use egui_kittest::kittest::Queryable;
    // The menu-bar button (an open window may have a control with the same name lower down).
    let m = h.get_all_by_label(menu).map(|n| n.rect()).min_by(|a, b| a.min.y.total_cmp(&b.min.y)).unwrap().center();
    click_at(h, m);
    let (mut r, disabled) = item_rect(h, item).unwrap_or_else(|| panic!("{menu} › {item} is in the menu"));
    assert!(!disabled, "{menu} › {item} enabled");
    // A long menu scrolls: wheel over it until the item is on screen.
    let screen = h.ctx.content_rect();
    let mut n = 0;
    // (The menu's viewport is the window height less 48 points, below the ~35-point menu bar.)
    let bottom = screen.max.y - 13.0;
    while r.max.y > bottom && n < 60 {
        let over = egui::pos2(r.center().x, screen.max.y - 60.0);
        h.event(egui::Event::PointerMoved(over));
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -120.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.run_steps(2);
        r = item_rect(h, item).map(|x| x.0).unwrap_or(r);
        n += 1;
    }
    assert!(r.max.y <= bottom && r.min.y >= 0.0, "{menu} › {item} reachable on screen: {r:?} in {screen:?}");
    if n > 0 {
        // Let the scroll settle (egui animates wheel scrolling), then read the position again.
        h.run_steps(20);
        r = item_rect(h, item).map(|x| x.0).unwrap_or(r);
    }
    click_at(h, r.center());
}

/// HF19: Recover Autosave, Recording, SML Remote, Detach/Dock Mixer, External Control, Command
/// Help and Assistant are real menu items that open the real windows, at a normal size and at
/// 1366×768 / 150 % (911×512 points) and 960×540 at 200 %. Recover Autosave is disabled while
/// recording, with the reason; the HF18 guard still refuses direct calls.
#[test]
fn hf19_core_tools_are_in_the_menus() {
    use egui_kittest::kittest::Queryable;
    for (w, hh, ppp) in [(1600.0, 1000.0, 1.0), (911.0, 512.0, 1.0), (960.0, 540.0, 2.0)] {
        let mut h = harness(w, hh, ppp);
        for (menu, item, open) in [
            ("File", "Recover Autosave...", (|a: &SoundApp| a.ui.show_recover) as fn(&SoundApp) -> bool),
            ("Window", "Recording", |a: &SoundApp| a.ui.show_recording),
            ("Window", "SML Remote", |a: &SoundApp| a.ui.show_remote),
            ("Setup", "External Control...", |a: &SoundApp| a.ui.show_external_control),
            ("Help", "Command Help", |a: &SoundApp| a.ui.show_help),
            ("Help", "Assistant", |a: &SoundApp| a.ui.show_assistant),
            ("Window", "Detach Mixer", |a: &SoundApp| a.ui.mixer_window.detached),
        ] {
            menu_click(&mut h, menu, item);
            assert!(open(h.state()), "{w}x{hh}@{ppp}: {menu} › {item} opened its window");
        }
        menu_click(&mut h, "Window", "Dock Mixer");
        assert!(!h.state().ui.mixer_window.detached, "Dock Mixer docks");
        // Recording: Recover Autosave is disabled with the reason.
        h.state_mut().ui.show_recover = false;
        h.state_mut().engine.transport.recording = true;
        let m = h.get_all_by_label("File").map(|n| n.rect()).min_by(|a, b| a.min.y.total_cmp(&b.min.y)).unwrap().center();
        click_at(&mut h, m);
        let (_, disabled) = item_rect(&h, "Recover Autosave...").expect("listed while recording");
        assert!(disabled, "{w}x{hh}: disabled while recording");
        h.state_mut().engine.transport.recording = false;
        h.key_press(egui::Key::Escape);
        h.run_steps(2);
    }
}
