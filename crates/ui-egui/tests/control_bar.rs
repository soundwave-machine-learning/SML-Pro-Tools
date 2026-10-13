//! HF03 + HF05 + HF06: the control bar under the menu bar (tempo, time signature, Loop, Audio
//! Setup), the Edit window's Loop lane and the Audio Setup window — driven through the real app
//! with pointer and keyboard events. Headless: there is no audio device in the test machine, so
//! Audio Setup is checked for honest "not open / unknown" states, never for invented values.
use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::{Harness, kittest::NodeT, kittest::Queryable};
use serde_json::json;
use soundcraft_time::Range;
use soundcraft_ui_egui::{AudioPrefs, Services, SoundApp, UiState};

fn harness_with(w: f32, h: f32, ppp: f32, app: SoundApp) -> Harness<'static, SoundApp> {
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

fn harness(w: f32, h: f32) -> Harness<'static, SoundApp> {
    let mut h = harness_with(w, h, 1.0, SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default()));
    h.state_mut().run("window.edit", json!({})).unwrap();
    h.run_steps(3);
    h
}

fn key(h: &mut Harness<'_, SoundApp>, k: Key, modifiers: Modifiers) {
    h.event(Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers });
    h.event(Event::Key { key: k, physical_key: None, pressed: false, repeat: false, modifiers });
    h.run_steps(2);
}

fn click_at(h: &mut Harness<'_, SoundApp>, p: Pos2, button: PointerButton) {
    h.event(Event::PointerMoved(p));
    h.run_steps(1);
    h.event(Event::PointerButton { pos: p, button, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    h.event(Event::PointerButton { pos: p, button, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
}

fn drag(h: &mut Harness<'_, SoundApp>, from: Pos2, to: Pos2, escape_before_release: bool) {
    h.event(Event::PointerMoved(from));
    h.run_steps(1);
    h.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    for k in 1..=8 {
        h.event(Event::PointerMoved(from.lerp(to, k as f32 / 8.0)));
        h.run_steps(1);
    }
    if escape_before_release {
        key(h, Key::Escape, Modifiers::NONE);
    }
    h.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
}

fn bpm_at(h: &Harness<'_, SoundApp>, at: i64) -> f64 {
    soundcraft_ui_egui::control_bar::tempo_at(h.state(), at).1
}

/// Focus the BPM field, select its text and type `text` (not committed).
fn type_bpm(h: &mut Harness<'_, SoundApp>, text: &str) {
    h.get_by_label("BPM").focus();
    h.run_steps(1);
    key(h, Key::A, Modifiers::COMMAND);
    h.event(Event::Text(text.into()));
    h.run_steps(2);
}

#[test]
fn bpm_is_visible_and_editable_in_every_layout_and_size() {
    for (w, hgt, ppp) in [(911.0, 512.0, 1.0), (1366.0, 768.0, 1.0), (1920.0, 1080.0, 1.0), (960.0, 540.0, 2.0)] {
        let mut h = harness_with(w, hgt, ppp, SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default()));
        for win in ["window.edit", "window.mix", "window.record", "window.master"] {
            h.state_mut().run(win, json!({})).unwrap();
            h.run_steps(4);
            for label in ["BPM", "⚙ Audio Setup", "⟲ Loop", "Click"] {
                let r = h.get_all_by_label(label).next().unwrap_or_else(|| panic!("{w}x{hgt} {win}: no {label}")).rect();
                assert!(r.min.x >= 0.0 && r.max.x <= w + 0.5 && r.max.y <= hgt * 0.35, "{w}x{hgt} {win}: {label} on screen near the top {r:?}");
            }
            let bpm = h.get_by_label("BPM");
            assert!(!bpm.accesskit_node().is_disabled(), "{w}x{hgt} {win}: BPM editable");
            assert!(bpm.rect().width() >= 50.0, "{w}x{hgt} {win}: BPM legible");
        }
    }
}

#[test]
fn bpm_enter_commits_once_and_escape_or_click_away_cancel() {
    let mut h = harness(1366.0, 768.0);
    let before = h.state().engine.session().clone();
    assert_eq!(bpm_at(&h, 0), 104.0);
    // Typing shortcut letters into the field changes nothing in the session.
    type_bpm(&mut h, "msr 128");
    assert_eq!(*h.state().engine.session(), before, "typing is not a shortcut and commits nothing");
    // Escape cancels.
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert_eq!(*h.state().engine.session(), before, "Escape cancels");
    assert!(h.state().ui.bpm_draft.is_none());
    // Clicking away cancels.
    type_bpm(&mut h, "90");
    let away = pos2(683.0, 760.0); // the status bar
    click_at(&mut h, away, PointerButton::Primary);
    assert_eq!(bpm_at(&h, 0), 104.0, "clicking away cancels");
    // Out of range: refused with a message, nothing changes.
    type_bpm(&mut h, "2000");
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert_eq!(bpm_at(&h, 0), 104.0);
    assert!(h.state().ui.status.contains("5 and 1000"), "{}", h.state().ui.status);
    // Enter commits exactly one undoable change.
    let undo0 = h.state().engine.undo_depth();
    type_bpm(&mut h, "128.5");
    key(&mut h, Key::Enter, Modifiers::NONE);
    h.run_steps(2);
    assert_eq!(bpm_at(&h, 0), 128.5);
    assert_eq!(h.state().engine.undo_depth(), undo0 + 1, "one undo step");
    assert_eq!(h.state().engine.session().tempo.tempos().len(), 1, "a constant tempo stays one event");
    let clips: Vec<_> = before.tracks.iter().flat_map(|t| t.playlist().into_iter().flat_map(|p| p.clips.iter()).map(|c| (c.id, c.start))).collect();
    for (id, start) in clips {
        assert_eq!(h.state().engine.session().find_clip(id).unwrap().1.start, start, "audio keeps its sample position");
    }
    h.state_mut().run("edit.undo", json!({})).unwrap();
    h.run_steps(2);
    assert_eq!(bpm_at(&h, 0), 104.0, "undo restores");
    // Disabled during a take.
    h.state_mut().engine.transport.recording = true;
    h.run_steps(2);
    assert!(h.get_by_label("BPM").accesskit_node().is_disabled(), "no tempo edits while recording");
    h.state_mut().engine.transport.recording = false;
}

#[test]
fn bpm_edits_the_tempo_change_in_effect_and_keeps_the_rest_of_the_map() {
    let mut h = harness(1366.0, 768.0);
    let later = 48_000 * 8;
    h.state_mut().run("event.tempo", json!({"bpm": 90, "at": later})).unwrap();
    h.run_steps(2);
    h.get_by_label("MAP");
    // Playhead at 0: the first change is edited, the second stays.
    h.state_mut().run("edit.select", json!({"start": 0, "end": 0})).unwrap();
    h.run_steps(2);
    type_bpm(&mut h, "120");
    key(&mut h, Key::Enter, Modifiers::NONE);
    let ev_later = soundcraft_ui_egui::control_bar::tempo_at(h.state(), later + 10);
    assert_eq!((bpm_at(&h, 0), ev_later.1, ev_later.2), (120.0, 90.0, 2));
    // Playhead after the second change: only that one changes.
    let at_later = ev_later.0 + 4800;
    h.state_mut().run("edit.select", json!({"start": at_later, "end": at_later})).unwrap();
    h.run_steps(2);
    type_bpm(&mut h, "95");
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert_eq!(bpm_at(&h, 0), 120.0, "the first change is kept");
    assert_eq!(bpm_at(&h, at_later), 95.0);
    assert_eq!(h.state().engine.session().tempo.tempos().len(), 2, "the map is not flattened");
}

#[test]
fn time_signature_popover_applies_and_cancels() {
    let mut h = harness(1366.0, 768.0);
    let undo0 = h.state().engine.undo_depth();
    h.get_by_label("4/4").click();
    h.run_steps(2);
    h.get_by_label("Time signature");
    h.state_mut().ui.meter_draft = Some((7, 8));
    h.run_steps(1);
    h.get_by_role_and_label(Role::Button, "Cancel").click();
    h.run_steps(2);
    let m = |h: &Harness<'_, SoundApp>| {
        let s = h.state().engine.session();
        let m = s.tempo.meter_at_tick(0);
        (m.numerator, m.denominator)
    };
    assert_eq!(m(&h), (4, 4), "Cancel changes nothing");
    assert!(h.state().ui.meter_draft.is_none());
    h.get_by_label("4/4").click();
    h.run_steps(2);
    h.state_mut().ui.meter_draft = Some((6, 8));
    h.run_steps(1);
    h.get_by_role_and_label(Role::Button, "Apply").click();
    h.run_steps(2);
    assert_eq!(m(&h), (6, 8));
    assert_eq!(h.state().engine.undo_depth(), undo0 + 1);
    assert_eq!(h.state().engine.session().tempo.meters().len(), 1, "edits the signature in effect, adds none");
    h.get_by_label("6/8");
}

#[test]
fn loop_toggle_and_settings_popover() {
    let mut h = harness(1366.0, 768.0);
    let loop_on = |h: &Harness<'_, SoundApp>| h.state().engine.session().edit.loop_playback;
    assert!(!loop_on(&h));
    h.get_by_label("⟲ Loop").click();
    h.run_steps(2);
    assert!(loop_on(&h), "one click turns Loop Playback on");
    assert!(!h.state().engine.transport.recording && !h.state().is_playing(), "the toggle never starts the transport");
    h.get_by_label("⟲ Loop").click();
    h.run_steps(2);
    assert!(!loop_on(&h));
    // Popover: set Start/End in the main counter format.
    h.get_by_label_contains("Loop: ").click();
    h.run_steps(2);
    h.get_by_label("Loop settings");
    h.state_mut().ui.loop_draft = Some(["3|1|000".into(), "5|1|000".into()]);
    h.run_steps(1);
    h.get_by_role_and_label(Role::Button, "Apply").click();
    h.run_steps(2);
    let r = h.state().engine.session().edit.loop_range.expect("stored");
    let f = h.state().engine.session().edit.main_counter;
    assert_eq!(f, soundcraft_time::TimeFormat::BarsBeats, "the demo counts in bars");
    let bar = |b: i64| {
        let p = json!({"p": format!("{}:{b}|1|000", f.id())});
        soundcraft_engine::cmd::position_param(&h.state().engine, "test", &p, "p").unwrap().unwrap()
    };
    assert_eq!((r.start, r.end), (bar(3), bar(5)));
    // Invalid input: refused, the range stays, the message says why.
    h.state_mut().ui.loop_draft = Some(["5|1|000".into(), "3|1|000".into()]);
    h.run_steps(1);
    h.get_by_role_and_label(Role::Button, "Apply").click();
    h.run_steps(2);
    assert_eq!(h.state().engine.session().edit.loop_range, Some(r), "invalid range leaves the locators alone");
    assert!(h.state().ui.status.contains("end after"), "{}", h.state().ui.status);
    // Set Loop to Selection; the selection itself is untouched.
    h.state_mut().run("edit.select", json!({"start": 4800, "end": 9600})).unwrap();
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Set Loop to Selection").click();
    h.run_steps(2);
    assert_eq!(h.state().engine.session().edit.loop_range, Some(Range::new(4800, 9600)));
    // Closing the popover discards typed text: it reopens with the stored range.
    h.state_mut().ui.loop_draft = Some(["junk".into(), "junk".into()]);
    key(&mut h, Key::Escape, Modifiers::NONE);
    h.run_steps(2);
    assert!(h.state().ui.loop_draft.is_none(), "no stale draft after closing");
}

#[test]
fn loop_lane_drag_handles_escape_and_menu() {
    let mut h = harness(1920.0, 1080.0);
    h.state_mut().run("loop.set", json!({"start": 96_000, "end": 384_000})).unwrap();
    h.state_mut().run("options.loop_playback", json!({"value": true})).unwrap();
    h.run_steps(3);
    let lane = h.get_by_label("Loop range lane").rect();
    let x_of = |h: &Harness<'_, SoundApp>, at: i64| {
        let s = h.state().engine.session();
        let z = s.edit.zoom;
        lane.min.x + ((at - z.scroll) as f64 / z.samples_per_px) as f32
    };
    let y = lane.center().y;
    // Escape during a drag cancels it.
    let (x0, x1) = (x_of(&h, 96_000), x_of(&h, 384_000));
    let undo0 = h.state().engine.undo_depth();
    drag(&mut h, pos2(x1 - 1.0, y), pos2(x1 + 200.0, y), true);
    assert_eq!(h.state().engine.session().edit.loop_range, Some(Range::new(96_000, 384_000)), "Escape cancels the drag");
    assert_eq!(h.state().engine.undo_depth(), undo0);
    // The end handle moves the end only: one undo step.
    drag(&mut h, pos2(x1 - 1.0, y), pos2(x1 + 150.0, y), false);
    let r = h.state().engine.session().edit.loop_range.unwrap();
    assert_eq!(r.start, 96_000, "start kept");
    assert!(r.end > 384_000, "end moved {r:?}");
    assert_eq!(h.state().engine.undo_depth(), undo0 + 1, "one undo step per drag");
    // Dragging the middle moves the whole range, keeping its length.
    let len = r.len();
    let mid = (x0 + x_of(&h, r.end)) / 2.0;
    drag(&mut h, pos2(mid, y), pos2(mid + 100.0, y), false);
    let m = h.state().engine.session().edit.loop_range.unwrap();
    assert!(m.start > r.start && m.len() == len, "moved {r:?} → {m:?}");
    // Drag on empty lane makes a new range; the edit selection never moves.
    let sel = h.state().engine.session().edit.selection;
    let empty = x_of(&h, m.end) + 120.0;
    drag(&mut h, pos2(empty, y), pos2(empty + 160.0, y), false);
    let n = h.state().engine.session().edit.loop_range.unwrap();
    assert!(n.start > m.end, "new range {n:?}");
    assert_eq!(h.state().engine.session().edit.selection, sel, "the lane never changes the selection");
    // Context menu: Select Loop Range copies it into the selection (for Loop Record); Clear.
    h.get_by_label("Loop range lane").click_secondary();
    h.run_steps(2);
    h.get_by_label("Select Loop Range").click();
    h.run_steps(2);
    assert_eq!(h.state().engine.session().edit.selection, n);
    h.get_by_label("Loop range lane").click_secondary();
    h.run_steps(2);
    h.get_by_label("Clear Loop Range").click();
    h.run_steps(2);
    assert_eq!(h.state().engine.session().edit.loop_range, None);
    // Locked while recording.
    h.state_mut().engine.transport.recording = true;
    h.run_steps(2);
    drag(&mut h, pos2(x0, y), pos2(x0 + 100.0, y), false);
    assert_eq!(h.state().engine.session().edit.loop_range, None, "no locator moves during a take");
    h.state_mut().engine.transport.recording = false;
}

#[test]
fn loop_playback_loops_the_stored_range_and_falls_back_to_the_selection() {
    let mut h = harness(1366.0, 768.0);
    // A 0.1 s loop: the headless transport (800 samples per frame) must wrap inside it.
    h.state_mut().run("loop.set", json!({"start": 48_000, "end": 52_800})).unwrap();
    h.state_mut().run("options.loop_playback", json!({"value": true})).unwrap();
    h.state_mut().run("edit.select", json!({"start": 0, "end": 0})).unwrap();
    h.state_mut().run("transport.play", json!({})).unwrap();
    for _ in 0..30 {
        h.run_steps(1);
        let p = h.state().engine.transport.position;
        assert!((48_000..52_800).contains(&p), "position {p} stays in the stored loop");
    }
    h.state_mut().run("transport.stop", json!({})).unwrap();
    h.run_steps(2);
    // No stored range: the selection loops, as before.
    h.state_mut().run("loop.clear", json!({})).unwrap();
    h.state_mut().run("edit.select", json!({"start": 9_600, "end": 14_400})).unwrap();
    h.state_mut().run("transport.play", json!({})).unwrap();
    for _ in 0..30 {
        h.run_steps(1);
        let p = h.state().engine.transport.position;
        assert!((9_600..14_400).contains(&p), "position {p} stays in the selection loop");
    }
    h.state_mut().run("transport.stop", json!({})).unwrap();
    assert!(!h.state().engine.session().edit.loop_record, "loop playback never turns on loop record");
}

#[test]
fn audio_setup_opens_in_one_click_from_every_layout_and_from_a_strip_input() {
    let mut h = harness_with(1366.0, 768.0, 1.0, SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default()));
    for win in ["window.edit", "window.mix", "window.record", "window.master"] {
        h.state_mut().run(win, json!({})).unwrap();
        h.run_steps(3);
        h.get_by_label("⚙ Audio Setup").click();
        h.run_steps(3);
        assert!(h.state().ui.show_playback_engine, "{win}: one click opens Audio Setup");
        h.get_by_label("Output device");
        h.get_by_label("Input device (microphone / interface)");
        h.get_by_label("⚙ Audio Setup").click();
        h.run_steps(3);
        assert!(!h.state().ui.show_playback_engine, "{win}: the same button closes it");
    }
    // A channel strip's input selector → Audio Setup… opens it focused on that track.
    h.state_mut().run("window.mix", json!({})).unwrap();
    h.state_mut().run("mix.view", json!({"view": 0})).unwrap();
    h.run_steps(4);
    h.get_by_label("Snare input").click();
    h.run_steps(2);
    h.get_by_label("Audio Setup…").click();
    h.run_steps(3);
    let snare = h.state().engine.session().track_by_name("Snare").unwrap().id;
    assert!(h.state().ui.show_playback_engine);
    assert_eq!(h.state().ui.audio_focus_track, Some(snare));
    h.get_by_label("Snare");
    // Search reaches it too.
    h.state_mut().run("window.audio_setup", json!({"value": false})).unwrap();
    h.run_steps(2);
    h.state_mut().run("window.audio_setup", json!({})).unwrap();
    h.run_steps(2);
    assert!(h.state().ui.show_playback_engine);
}

#[test]
fn audio_setup_reports_only_what_is_known_and_never_fakes_a_meter() {
    let mut h = harness(1366.0, 768.0);
    h.state_mut().run("edit.select", json!({"tracks": ["Kick"]})).unwrap();
    h.state_mut().run("window.audio_setup", json!({"value": true})).unwrap();
    h.run_steps(3);
    // No device in the test machine: the window says so instead of inventing values.
    h.get_by_label("no output stream");
    h.get_by_label("open now: none (opened on the first record or arm)");
    h.get_by_label("Arm or input-monitor the track to meter its input (no test signal is ever generated).");
    assert!(h.query_all_by_role(Role::ProgressIndicator).next().is_none(), "no meter for an unarmed track");
    // The unset input reads the way recording uses it.
    assert!(h.get_all_by_role(Role::ComboBox).any(|n| n.value().as_deref() == Some("In 1")), "Kick records In 1");
    // Latency ledger.
    h.get_by_label("Advanced / Latency").click();
    h.run_steps(3);
    h.get_by_label("Unknown (not reported by this backend; not assumed zero)");
    h.get_by_label("Not measured: this build has no loopback calibration");
    // Choosing a track input from Audio Setup is the same undoable track.input.
    let undo0 = h.state().engine.undo_depth();
    let combo = h.get_all_by_role(Role::ComboBox).find(|n| n.value().as_deref() == Some("In 1")).unwrap().rect();
    click_at(&mut h, combo.center(), PointerButton::Primary);
    h.get_by_label("In 3").click();
    h.run_steps(2);
    let kick = h.state().engine.session().track_by_name("Kick").unwrap().clone();
    assert_eq!(kick.mixer.input, soundcraft_model::Route::Hardware("In 3".into()));
    assert_eq!(h.state().engine.undo_depth(), undo0 + 1);
    // Arming shows a real meter (silent here: no input device, so "no signal").
    h.state_mut().run("mix.record_arm", json!({"tracks": ["Kick"], "value": true})).unwrap();
    h.run_steps(3);
    let bar = h.get_all_by_role(Role::ProgressIndicator).next().expect("armed track has an input meter");
    let text = format!("{:?} {:?}", bar.accesskit_node().label(), bar.value());
    assert!(text.contains("input: no signal"), "no simulated activity: {text}");
}

#[test]
fn audio_setup_drafts_apply_and_cancel_and_are_remembered() {
    let engine = soundcraft_engine::demo::demo_engine();
    let player = soundcraft_playback::Player::new(engine.session_arc());
    let mut h = harness_with(1366.0, 768.0, 1.0, SoundApp::new(engine, Some(player), Services::default()));
    h.state_mut().run("window.audio_setup", json!({"value": true})).unwrap();
    h.run_steps(3);
    assert!(h.get_by_role_and_label(Role::Button, "Apply").accesskit_node().is_disabled(), "nothing to apply");
    // A buffer choice is a draft: Cancel returns to what is applied.
    h.state_mut().ui.buffer_choice = 128;
    h.run_steps(2);
    h.get_by_label("unapplied changes");
    h.get_by_role_and_label(Role::Button, "Cancel").click();
    h.run_steps(2);
    assert_eq!(h.state().ui.buffer_choice, 0);
    assert!(!h.state().audio_draft_pending());
    // Closing the window discards a draft too.
    h.state_mut().ui.buffer_choice = 256;
    h.state_mut().run("window.audio_setup", json!({"value": false})).unwrap();
    h.run_steps(2);
    assert_eq!(h.state().ui.buffer_choice, 0, "closing cancels");
    // Apply is refused while the transport runs.
    h.state_mut().run("window.audio_setup", json!({"value": true})).unwrap();
    h.run_steps(2);
    h.state_mut().ui.device_choice = Some(("NoSuchHost".into(), "Missing Interface".into()));
    h.state_mut().run("transport.play", json!({})).unwrap();
    h.run_steps(2);
    assert!(h.state().is_playing());
    assert!(h.state_mut().apply_audio_draft().contains("Stop the transport"));
    h.state_mut().run("transport.stop", json!({})).unwrap();
    h.run_steps(2);
    // A device that cannot be opened: refused, nothing else changes, nothing is remembered.
    let msg = h.state_mut().apply_audio_draft();
    assert!(msg.starts_with("Device not changed"), "{msg}");
    assert_eq!(h.state().ui.audio, AudioPrefs::default(), "a failed Apply is not remembered");
    assert!(h.state().player.is_some(), "the open output stays");
}

#[test]
fn a_missing_saved_device_is_named_at_launch_and_apply_retries_it() {
    let engine = soundcraft_engine::demo::demo_engine();
    let player = soundcraft_playback::Player::new(engine.session_arc());
    let mut app = SoundApp::new(engine, Some(player), Services::default());
    // Preferences from a machine where an interface was chosen.
    let mut prefs: UiState = serde_json::from_str("{}").unwrap();
    prefs.audio = AudioPrefs { output: Some(("ASIO".into(), "Studio Interface".into())), buffer: 128, input: None };
    let saved = serde_json::to_string(&prefs).unwrap();
    app.ui = serde_json::from_str(&saved).unwrap();
    assert_eq!(app.ui.audio.buffer, 128, "audio choices survive a save/load of the preferences");
    app.restore_audio_prefs();
    assert!(app.ui.status.contains("Studio Interface") && app.ui.status.contains("could not be opened"), "{}", app.ui.status);
    assert!(app.player.is_some(), "the system default output stays in use");
    assert_eq!(app.ui.audio.output, Some(("ASIO".into(), "Studio Interface".into())), "the saved choice is kept for next time");
    assert!(app.audio_draft_pending(), "Apply can retry the saved device");
    let mut h = harness_with(1366.0, 768.0, 1.0, app);
    h.state_mut().run("window.audio_setup", json!({"value": true})).unwrap();
    h.run_steps(3);
    h.get_by_label("the saved output is not open — Apply retries it");
    assert!(!h.get_by_role_and_label(Role::Button, "Apply").accesskit_node().is_disabled());
    // Older preference files have no `audio` section.
    let old: UiState = serde_json::from_str("{}").unwrap();
    assert_eq!(old.audio, AudioPrefs::default());
}
