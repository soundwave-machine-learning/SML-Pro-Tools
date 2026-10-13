//! Hotfix Pass 02: portable checks of HF07–HF13 behaviour reachable from the UI layer.
use egui::vec2;
#[allow(unused_imports)]
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;
use soundcraft_ui_egui::{Services, SoundApp};

#[allow(dead_code)]
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
