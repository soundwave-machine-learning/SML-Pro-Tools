//! E04 / R19 (portable part): input-to-usable timings for opening a menu and for an indexed
//! command search at the W1 workload (16 tracks, 2000 clips), 100 repetitions each, cold (the
//! first opening after launch) and warm recorded separately. Headless egui_kittest with a
//! software renderer: these are UI-thread model/layout costs, not native frame delivery;
//! native W1/W2 distributions on the target machine stay a target gate. The CSV lands in
//! `docs/evidence/next_cycle/sprint_02/menu-timing.csv` when `SML_MENU_TIMING_CSV` is set.
use egui::{PointerButton, pos2, vec2};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use serde_json::json;
use soundcraft_audio_io::AudioBuffer;
use soundcraft_model::{ChannelFormat, Clip, Session, SourceAudio, SourceId, TrackKind};
use soundcraft_ui_egui::{Services, SoundApp};
use std::sync::Arc;
use std::time::Instant;

fn w1() -> Session {
    let mut s = Session::default();
    s.pool.insert(SourceId(1), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![vec![0.1; 48_000]] })));
    for k in 0..16 {
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some(&format!("Track {k}")));
        for c in 0..125 {
            let id = s.new_clip_id();
            s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "c", SourceId(1), 0, c * 50_000, 40_000));
        }
    }
    s.add_track(TrackKind::Master, ChannelFormat::Stereo, None);
    s
}

/// Whether a button whose accessible name starts with `prefix` exists (menu items carry the
/// catalog wording and a trailing shortcut column, e.g. "Can't Undo ").
fn button_starting_with(h: &Harness<'_, SoundApp>, prefix: &str) -> bool {
    h.get_all_by_role(egui::accesskit::Role::Button).any(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with(prefix)))
}

fn stats(mut v: Vec<f64>) -> (f64, f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let q = |p: f64| v[((v.len() as f64 - 1.0) * p).round() as usize];
    (q(0.5), q(0.95), q(0.99), *v.last().unwrap_or(&0.0))
}

#[test]
fn menu_and_search_timings_at_w1() {
    let app = SoundApp::new(soundcraft_engine::Engine::new(w1()), None, Services::default());
    let mut h = Harness::builder().with_size(vec2(1400.0, 900.0)).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(3);
    assert_eq!(h.state().engine.session().tracks.iter().map(|t| t.clips().len()).sum::<usize>(), 2000);
    let mut rows = Vec::new();
    let mut menu_ms = Vec::new();
    let mut cold_menu = None;
    for i in 0..100 {
        // Click the Edit menu; usable = the first frame where an item ("Undo") exists.
        let p = h.get_by_label("Edit").rect().center();
        h.event(egui::Event::PointerMoved(p));
        h.step();
        h.event(egui::Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        h.step();
        // Input = the button release; usable = the first frame where an item ("Cut") exists.
        let t0 = Instant::now();
        h.event(egui::Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
        let mut frames = 0;
        loop {
            h.step();
            frames += 1;
            if button_starting_with(&h, "Cut") || frames > 10 {
                break;
            }
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        assert!(frames <= 10, "the menu never became usable");
        if i == 0 {
            cold_menu = Some(ms);
        } else {
            menu_ms.push(ms);
        }
        // Close it.
        h.event(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
        h.step();
        h.event(egui::Event::PointerMoved(pos2(700.0, 500.0)));
        h.step();
    }
    let (p50, p95, p99, max) = stats(menu_ms.clone());
    rows.push(format!("menu_open_warm,W1,{},{p50:.2},{p95:.2},{p99:.2},{max:.2}", menu_ms.len()));
    rows.push(format!("menu_open_cold,W1,1,{0:.2},{0:.2},{0:.2},{0:.2}", cold_menu.unwrap_or(0.0)));
    // Indexed search: type a query character, usable = the frame where a result label appears.
    let mut search_ms = Vec::new();
    let mut cold_search = None;
    for i in 0..100 {
        h.state_mut().run("window.search", json!({"value": true})).unwrap();
        h.state_mut().palette.query.clear();
        h.step();
        let t0 = Instant::now();
        h.event(egui::Event::Text("boun".into()));
        let mut frames = 0;
        loop {
            h.step();
            frames += 1;
            if button_starting_with(&h, "Bounce Mix") || frames > 10 {
                break;
            }
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        assert!(frames <= 10, "search never produced a result");
        if i == 0 {
            cold_search = Some(ms);
        } else {
            search_ms.push(ms);
        }
        h.state_mut().run("window.search", json!({"value": false})).unwrap();
        h.step();
    }
    let (p50, p95, p99, max) = stats(search_ms.clone());
    rows.push(format!("search_warm,W1,{},{p50:.2},{p95:.2},{p99:.2},{max:.2}", search_ms.len()));
    rows.push(format!("search_cold,W1,1,{0:.2},{0:.2},{0:.2},{0:.2}", cold_search.unwrap_or(0.0)));
    for r in &rows {
        println!("{r}");
    }
    if let Some(path) = std::env::var_os("SML_MENU_TIMING_CSV") {
        let mut out = String::from("case,workload,repetitions,p50_ms,p95_ms,p99_ms,max_ms,provenance\n");
        for r in &rows {
            out.push_str(&format!("{r},headless egui_kittest software renderer in container P (not native)\n"));
        }
        std::fs::write(path, out).unwrap();
    }
    // The SML goals (p95 menu ≤ 50 ms, search ≤ 100 ms) are native goals; here they bound the
    // model/layout cost that would dominate a native frame.
    let (_, menu_p95, _, _) = stats(menu_ms);
    let (_, search_p95, _, _) = stats(search_ms);
    assert!(menu_p95 <= 50.0, "menu p95 {menu_p95:.1} ms");
    assert!(search_p95 <= 100.0, "search p95 {search_p95:.1} ms");
}
