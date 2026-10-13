//! HF01: the Mix window's Split view — Visual over Console in bounded panes with a draggable,
//! saved, clamped divider. Driven through the real app with pointer events at the hotfix's
//! screen sizes (logical points; 1366×768 at 150 % ≈ 911×512).
use egui::{Event, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;
use soundcraft_ui_egui::mix_window::{SPLIT_DEFAULT, SPLIT_DIVIDER, clamp_split, split_heights};
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
    h.run_steps(3);
    h.state_mut().run("window.mix", json!({})).unwrap();
    h.state_mut().run("mix.view", json!({"view": 2})).unwrap();
    h.run_steps(6);
    h
}

fn divider(h: &Harness<'_, SoundApp>) -> egui::Rect {
    h.get_by_label("Mixer split divider").rect()
}

fn drag(h: &mut Harness<'_, SoundApp>, from: Pos2, to: Pos2) {
    h.event(Event::PointerMoved(from));
    h.run_steps(1);
    h.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    for k in 1..=8 {
        h.event(Event::PointerMoved(from.lerp(to, k as f32 / 8.0)));
        h.run_steps(1);
    }
    h.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
}

#[test]
fn divider_resizes_clamps_and_never_touches_the_session() {
    for (w, hgt, ppp) in [(911.0, 512.0, 1.0), (1366.0, 768.0, 1.0), (1920.0, 1080.0, 1.0), (960.0, 540.0, 2.0)] {
        let mut h = harness(w, hgt, ppp);
        let session = h.state().engine.session().clone();
        let d0 = divider(&h);
        assert!(d0.height() >= SPLIT_DIVIDER - 0.5 && d0.width() > w * 0.5, "{w}x{hgt}: divider is a full-width bar {d0:?}");
        assert!(d0.min.y > 60.0 && d0.max.y < hgt - 60.0, "{w}x{hgt}: divider between the panes {d0:?}");
        // Down: the Visual pane grows, the Console keeps a usable minimum.
        drag(&mut h, d0.center(), pos2(d0.center().x, hgt - 5.0));
        let low = divider(&h);
        assert!(low.min.y > d0.min.y + 20.0, "{w}x{hgt}: moved down {d0:?} → {low:?}");
        assert!(hgt - low.max.y >= 100.0, "{w}x{hgt}: Console keeps its minimum height ({low:?})");
        let saved = h.state().ui.mix_split;
        assert!((0.1..=0.9).contains(&saved) && saved > SPLIT_DEFAULT, "{w}x{hgt}: saved share {saved}");
        // Up: clamped the other way.
        drag(&mut h, low.center(), pos2(low.center().x, 0.0));
        let high = divider(&h);
        assert!(high.min.y < d0.min.y, "{w}x{hgt}: moved up {high:?}");
        assert!(high.min.y > 60.0, "{w}x{hgt}: Visual keeps its minimum height ({high:?})");
        // Double-click resets to the default share.
        let c = high.center();
        h.event(Event::PointerMoved(c));
        h.run_steps(1);
        // 60 frames per second (the harness default of 4 per second is slower than the
        // double-click window).
        for _ in 0..2 {
            h.event(Event::PointerButton { pos: c, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
            h.run_steps(1);
            h.event(Event::PointerButton { pos: c, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
            h.run_steps(1);
        }
        h.run_steps(3);
        assert_eq!(h.state().ui.mix_split, SPLIT_DEFAULT, "{w}x{hgt}: double-click resets");
        // The right-click menu resets too.
        let from = divider(&h).center();
        drag(&mut h, from, pos2(c.x, hgt * 0.75));
        assert_ne!(h.state().ui.mix_split, SPLIT_DEFAULT);
        h.get_by_label("Mixer split divider").click_secondary();
        h.run_steps(2);
        h.get_by_label("Reset split").click();
        h.run_steps(2);
        assert_eq!(h.state().ui.mix_split, SPLIT_DEFAULT, "{w}x{hgt}: menu reset");
        assert_eq!(*h.state().engine.session(), session, "{w}x{hgt}: layout moves change nothing in the session");
    }
}

#[test]
fn console_scrolls_inside_its_pane_to_reach_every_strip_control() {
    let mut h = harness(911.0, 512.0, 1.0);
    let d = divider(&h);
    // The Mute buttons sit low in each strip; with a short Console pane they start out of view.
    let lowest = |h: &Harness<'_, SoundApp>| h.get_all_by_label("M").map(|n| n.rect().max.y).fold(f32::MIN, f32::max);
    let before = lowest(&h);
    let pane = egui::Rect::from_min_max(pos2(0.0, d.max.y), pos2(911.0, 512.0));
    let over = pos2(pane.center().x, pane.center().y);
    h.event(Event::PointerMoved(over));
    for _ in 0..20 {
        h.event(Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -120.0),
            modifiers: Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.run_steps(1);
    }
    h.run_steps(3);
    let after = lowest(&h);
    assert!(after < before, "scrolling the Console pane moves its strips up ({before} → {after})");
    assert!(after <= 512.0, "the lowest strip controls become reachable inside the window ({after})");
    assert_eq!(divider(&h), d, "scrolling the Console never moves the divider or the Visual pane");
}

#[test]
fn window_resize_reflows_and_saved_values_are_clamped() {
    let mut h = harness(1920.0, 1080.0, 1.0);
    h.state_mut().run("ui.mix_split", json!({"fraction": 0.85})).unwrap();
    h.run_steps(3);
    h.set_size(vec2(911.0, 512.0));
    h.run_steps(4);
    let d = divider(&h);
    assert!(d.min.y > 60.0, "after shrinking, the Visual pane keeps its minimum ({d:?})");
    assert!(512.0 - d.max.y >= 100.0, "after shrinking, the Console still has room ({d:?})");
    assert_eq!(h.state().ui.mix_split, 0.85, "the saved preference itself is kept for a larger window");
    // Out-of-range values: refused by the command, made safe when loaded.
    assert!(h.state_mut().run("ui.mix_split", json!({"fraction": 2.0})).is_err());
    assert_eq!(clamp_split(f32::NAN), SPLIT_DEFAULT);
    assert_eq!(clamp_split(5.0), SPLIT_DEFAULT);
    let mut ui: UiState = serde_json::from_str("{}").unwrap();
    assert_eq!(ui.mix_split, SPLIT_DEFAULT, "a preferences file from an older build gets the default");
    ui.mix_split = 0.3;
    let back: UiState = serde_json::from_str(&serde_json::to_string(&ui).unwrap()).unwrap();
    assert_eq!(back.mix_split, 0.3, "the share survives a save/load of the preferences");
    let reset = h.state_mut().run("ui.mix_split", json!({})).unwrap();
    assert_eq!(reset["fraction"], json!(SPLIT_DEFAULT));
}

#[test]
fn pane_heights_keep_minimums_and_share_tiny_windows() {
    let (v, c) = split_heights(1000.0, 0.45);
    assert!((v + c + SPLIT_DIVIDER - 1000.0).abs() < 1e-3);
    let (v, c) = split_heights(1000.0, 0.9);
    assert!(c >= 160.0, "Console minimum {c}");
    let (v2, _) = split_heights(1000.0, 0.1);
    assert!(v2 >= 140.0 && v > v2);
    let (v, c) = split_heights(200.0, 0.45);
    assert!(v > 0.0 && c > v, "tiny window shares the height ({v}, {c})");
}
