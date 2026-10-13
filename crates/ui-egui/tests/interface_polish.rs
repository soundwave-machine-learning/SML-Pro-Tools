//! Interface polish pass: presentation rules that must hold at the owner's window sizes —
//! menus never covered by the layout switcher, mixer strips in identity → routing → actions
//! order with text state tags, recording status that reflects only real session state, and the
//! built-in palettes validated.
use egui::{Rect, vec2};
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

fn rect(h: &Harness<'_, SoundApp>, label: &str) -> Rect {
    h.get_all_by_label(label).next().unwrap_or_else(|| panic!("no `{label}`")).rect()
}

#[test]
fn layout_switcher_never_covers_the_menus() {
    // 700 points is narrower than menus + switcher: the switcher moves into the control bar.
    for (w, hgt, ppp) in [(700.0, 480.0, 1.0), (911.0, 512.0, 1.0), (960.0, 540.0, 2.0), (1366.0, 768.0, 1.0), (1920.0, 1080.0, 1.0)] {
        let mut h = harness(w, hgt, ppp);
        for win in ["window.edit", "window.mix", "window.record", "window.master"] {
            h.state_mut().run(win, json!({})).unwrap();
            h.run_steps(4);
            let window_menu = rect(&h, "Window");
            for layout in ["Record layout", "Edit layout", "Mix layout", "Master layout"] {
                let b = rect(&h, layout);
                assert!(!b.intersects(window_menu), "{w}x{hgt} {win}: {layout} {b:?} covers the Window menu {window_menu:?}");
                assert!(b.min.x >= 0.0 && b.max.x <= w + 0.5 && b.max.y <= hgt * 0.4, "{w}x{hgt} {win}: {layout} on screen {b:?}");
            }
        }
        // The switcher still works wherever it is drawn.
        h.get_by_label("Mix layout").click();
        h.run_steps(3);
        assert_eq!(h.state().ui.window, soundcraft_ui_egui::MainWindow::Mix, "{w}x{hgt}: switcher works");
    }
}

#[test]
fn strips_read_identity_then_routing_then_actions_with_text_state_tags() {
    let mut h = harness(1920.0, 1080.0, 1.0);
    h.state_mut().run("window.mix", json!({})).unwrap();
    h.state_mut().run("mix.view", json!({"view": 0})).unwrap();
    h.state_mut().run("mix.record_arm", json!({"tracks": ["Kick"], "value": true})).unwrap();
    h.state_mut().run("mix.mute", json!({"tracks": ["Kick"], "value": true})).unwrap();
    h.state_mut().run("mix.solo", json!({"tracks": ["Bass"], "value": true})).unwrap();
    h.run_steps(6);
    let name = rect(&h, "Kick strip name");
    let input = rect(&h, "Kick input");
    let solo = h.get_all_by_label("S").map(|n| n.rect()).find(|r| r.center().x > name.min.x && r.center().x < name.max.x).expect("Kick solo");
    assert!(name.max.y < input.min.y, "identity above routing ({name:?} / {input:?})");
    assert!(input.max.y < solo.min.y, "routing above channel actions ({input:?} / {solo:?})");
    // States read as text, not colour alone.
    h.get_by_label("Kick state: REC, MUTE");
    h.get_by_label("Bass state: SOLO");
    h.get_by_label("Snare state: none");
    // Clicking the name still selects the track (behaviour unchanged by the move).
    h.get_by_label("Snare strip name").click();
    h.run_steps(3);
    let snare = h.state().engine.session().track_by_name("Snare").unwrap().id;
    assert_eq!(h.state().engine.session().edit.selected_tracks, vec![snare]);
}

#[test]
fn a_missing_insert_plugin_is_tagged_as_a_fault() {
    let mut h = harness(1920.0, 1080.0, 1.0);
    {
        let s = h.state_mut().engine.session_mut();
        let kick = s.tracks.iter_mut().find(|t| t.name == "Kick").unwrap();
        kick.mixer.inserts[0] = Some(soundcraft_model::Insert::new("clap:com.example.not-installed"));
    }
    h.state_mut().run("window.mix", json!({})).unwrap();
    h.run_steps(6);
    h.get_by_label("Kick state: FAULT");
}

#[test]
fn recording_status_reflects_armed_tracks_and_opens_audio_setup_on_them() {
    let mut h = harness(1366.0, 768.0, 1.0);
    h.state_mut().run("window.edit", json!({})).unwrap();
    h.run_steps(3);
    h.get_by_label("Recording status");
    h.state_mut().run("track.input", json!({"tracks": ["Snare"], "input": "In 3"})).unwrap();
    h.state_mut().run("mix.record_arm", json!({"tracks": ["Snare"], "value": true})).unwrap();
    h.run_steps(3);
    h.get_by_label("Recording status").click();
    h.run_steps(3);
    let snare = h.state().engine.session().track_by_name("Snare").unwrap().id;
    assert!(h.state().ui.show_playback_engine, "the status opens Audio Setup");
    assert_eq!(h.state().ui.audio_focus_track, Some(snare), "focused on the armed track");
    assert!(!h.state().engine.transport.recording && !h.state().is_playing(), "the status never starts the transport");
}

#[test]
fn every_built_in_palette_is_selectable_and_valid() {
    use soundcraft_ui_egui::theme::{THEME_FORMAT, ThemeData, ThemeMode, Tokens, validate};
    for (mode, tokens) in [
        (ThemeMode::Dark, Tokens::DARK),
        (ThemeMode::Graphite, Tokens::GRAPHITE),
        (ThemeMode::Light, Tokens::LIGHT),
        (ThemeMode::HighContrast, Tokens::HIGH_CONTRAST),
    ] {
        validate(&ThemeData { format: THEME_FORMAT.into(), name: mode.label().into(), base: mode, tokens, typography_scale: 1.0 })
            .unwrap_or_else(|e| panic!("{mode:?}: {e}"));
        // State colours keep their meaning: record, playback, warning and error differ.
        let states = [tokens.rec, tokens.playback, tokens.warning, tokens.error, tokens.focus];
        for (i, a) in states.iter().enumerate() {
            for b in &states[i + 1..] {
                assert_ne!(a, b, "{mode:?}: two state colours are identical");
            }
        }
    }
    let mut h = harness(1366.0, 768.0, 1.0);
    assert_eq!(h.state_mut().run("ui.theme", json!({"mode": "graphite"})).unwrap(), json!({"mode": "graphite"}));
    h.run_steps(3);
    // (The active palette is process-wide and other tests run in parallel, so check what the
    // chosen mode resolves to rather than the global.)
    assert_eq!(h.state().ui.theme, ThemeMode::Graphite);
    let ctx = egui::Context::default();
    assert_eq!(soundcraft_ui_egui::theme::resolve(&ctx, ThemeMode::Graphite, None), (false, Tokens::GRAPHITE));
}
