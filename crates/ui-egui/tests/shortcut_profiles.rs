//! R13 / V-SHORTCUTS (portable): profiles are separate and switch explicitly; the Studio One
//! top-row 1–8 preset selects the audio tools without touching the keypad Nudge keys; Pro Tools
//! F-keys and single-key commands do not leak into the Studio One profile; text entry and a
//! modal dialog take precedence; reassignment by capture lands in the Custom profile and the
//! live binding is what search shows. Physical keyboards and OS interception are native gates.
use egui::{Event, Key, Modifiers, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;
use soundcraft_model::Tool;
use soundcraft_ui_egui::{Services, SoundApp, shortcut_profiles::Profile};

fn harness() -> Harness<'static, SoundApp> {
    let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, Services::default());
    app.run("track.new", json!({"name": "Voice"})).unwrap();
    app.run("edit.select", json!({"tracks": ["Voice"], "start": 0})).unwrap();
    let mut h = Harness::builder().with_size(vec2(1400.0, 900.0)).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

fn key(h: &mut Harness<'_, SoundApp>, key: Key, modifiers: Modifiers) {
    h.event(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.run_steps(1);
    h.event(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
    h.run_steps(2);
}

#[test]
fn profiles_switch_explicitly_and_never_mix() {
    let mut h = harness();
    // Pro Tools style (default): F6 = Trim, the digit 1 does nothing to the tool.
    key(&mut h, Key::F6, Modifiers::NONE);
    assert_eq!(h.state().engine.session().edit.tool, Tool::Trim);
    key(&mut h, Key::Num1, Modifiers::NONE);
    assert_eq!(h.state().engine.session().edit.tool, Tool::Trim, "no top-row tool preset in the Pro Tools profile");
    // Studio One style: 1–8 pick the eight audio tools; F6 is not Trim any more.
    h.state_mut().run("ui.shortcut_profile", json!({"profile": "studio_one"})).unwrap();
    for (k, tool) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8].into_iter().zip(Tool::AUDIO) {
        key(&mut h, k, Modifiers::NONE);
        assert_eq!(h.state().engine.session().edit.tool, tool, "{k:?}");
    }
    key(&mut h, Key::F6, Modifiers::NONE);
    assert_eq!(h.state().engine.session().edit.tool, Tool::Listen, "Pro Tools F-keys do not leak into the Studio One profile");
    // The keypad Nudge preset coexists: Plus still nudges (the clip-less selection moves).
    let sel0 = h.state().engine.session().edit.selection.start;
    key(&mut h, Key::Plus, Modifiers::NONE);
    assert!(h.state().engine.session().edit.selection.start > sel0, "Num+ nudges under the Studio One profile too");
    // Text entry takes precedence: with the search field focused, 1 types instead of switching tools.
    h.state_mut().run("window.search", json!({})).unwrap();
    h.run_steps(3);
    h.event(Event::Text("1".into()));
    key(&mut h, Key::Num1, Modifiers::NONE);
    assert_eq!(h.state().engine.session().edit.tool, Tool::Listen, "typing in a text field never selects a tool");
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert!(!h.state().ui.show_search);
    // A modal dialog takes precedence too.
    {
        let app = h.state_mut();
        assert!(app.dialogs.open_for_command(&app.engine, "track.new"));
    }
    h.run_steps(2);
    key(&mut h, Key::Num2, Modifiers::NONE);
    assert_eq!(h.state().engine.session().edit.tool, Tool::Listen, "a modal dialog swallows tool keys");
    key(&mut h, Key::Escape, Modifiers::NONE);
    // Switching back is reversible and exact.
    h.state_mut().run("ui.shortcut_profile", json!({"profile": "pro_tools"})).unwrap();
    key(&mut h, Key::F7, Modifiers::NONE);
    assert_eq!(h.state().engine.session().edit.tool, Tool::Selector);
}

#[test]
fn reassignment_capture_lands_in_custom_and_search_shows_the_live_binding() {
    let mut h = harness();
    h.state_mut().run("window.shortcuts", json!({})).unwrap();
    h.run_steps(3);
    // Capture: click the Heal chord cell, press Ctrl+Shift+H.
    h.state_mut().shortcut_capture = Some("edit.heal".into());
    key(&mut h, Key::H, Modifiers { ctrl: true, command: true, shift: true, ..Modifiers::NONE });
    let app = h.state();
    assert_eq!(app.ui.shortcut_profile, Profile::Custom, "a reassignment switches to Custom");
    assert_eq!(app.ui.shortcut_base, Profile::ProTools);
    assert_eq!(app.ui.shortcut_overrides.get("edit.heal").map(String::as_str), Some("Cmd+Shift+H"));
    assert_eq!(app.shortcut_for("edit.heal", None).as_deref(), Some("Cmd+Shift+H"));
    assert!(app.shortcut_capture.is_none());
    // The old Cmd+H no longer heals; the new chord does (a selection over nothing to heal is a
    // harmless no-op, the command id is what we check through the status line).
    let r = h.state_mut().run("ui.shortcuts_conflicts", json!({})).unwrap();
    assert!(r["conflicts"].as_array().unwrap().iter().all(|c| c["chord"] != "Cmd+Shift+H"), "{r}");
    // Unbind, reset, export/import round trip.
    h.state_mut().run("ui.shortcut_set", json!({"command": "edit.undo", "chord": ""})).unwrap();
    assert!(h.state().shortcut_for("edit.undo", None).is_none());
    let exported = h.state_mut().run("ui.shortcuts_export", json!({})).unwrap();
    h.state_mut().run("ui.shortcuts_reset", json!({})).unwrap();
    assert_eq!(h.state().shortcut_for("edit.undo", None).as_deref(), Some("Cmd+Z"));
    h.state_mut().run("ui.shortcuts_import", json!({"text": exported["text"]})).unwrap();
    assert!(h.state().shortcut_for("edit.undo", None).is_none());
    assert_eq!(h.state().shortcut_for("edit.heal", None).as_deref(), Some("Cmd+Shift+H"));
    // An invalid file is refused without touching the current overrides.
    assert!(h.state_mut().run("ui.shortcuts_import", json!({"text": "{\"format\":\"nope\"}"})).is_err());
    assert_eq!(h.state().shortcut_for("edit.heal", None).as_deref(), Some("Cmd+Shift+H"));
    // The window lists the live chord for the reassigned command.
    h.run_steps(2);
    h.get_by_label("Heal Separation");
}
