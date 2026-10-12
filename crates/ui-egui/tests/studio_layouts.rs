//! R18 layouts, R37 monitor and R21 remote through the real host: one application, four
//! layouts that change no persistent identity; monitor controls that touch UI state (the
//! speaker path) and never the document; a phone client pairing over HTTP and driving the
//! transport while the UI thread serves the queue.
use egui::vec2;
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::{Value, json};
use soundcraft_ui_egui::{MainWindow, Services, SoundApp};
use std::io::{Read, Write};
use std::net::TcpStream;

fn harness() -> Harness<'static, SoundApp> {
    let app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
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

#[test]
fn layouts_share_one_session_and_change_no_identity() {
    let mut h = harness();
    let session = h.state().engine.session().clone();
    let ids: Vec<u64> = session.tracks.iter().map(|t| t.id.0).collect();
    for (w, label) in [(MainWindow::Record, "Record"), (MainWindow::Edit, "Edit"), (MainWindow::Mix, "Mix"), (MainWindow::Master, "Master")] {
        h.get_by_label(&format!("{label} layout")).click();
        h.run_steps(4);
        assert_eq!(h.state().ui.window, w, "{label}");
        assert_eq!(*h.state().engine.session(), session, "switching layouts edits nothing");
        assert_eq!(h.state().engine.session().tracks.iter().map(|t| t.id.0).collect::<Vec<_>>(), ids);
    }
    h.state_mut().run("window.record", json!({})).unwrap();
    h.run_steps(3);
    assert!(h.get_all_by_label("Record enable").count() >= 1, "track identity and arming stay visible");
    h.get_by_label("Trim (F6)");
    h.state_mut().run("window.master", json!({})).unwrap();
    h.run_steps(3);
    h.get_by_label("Back to Mix").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.window, MainWindow::Mix);
    // Prefs round-trip keeps the layout; older prefs without it load as Edit.
    let saved = serde_json::to_string(&h.state().ui).unwrap();
    let back: soundcraft_ui_egui::UiState = serde_json::from_str(&saved).unwrap();
    assert_eq!(back.window, MainWindow::Mix);
    let old: soundcraft_ui_egui::UiState = serde_json::from_value(json!({"window": "Edit"})).unwrap();
    assert_eq!(old.window, MainWindow::Edit);
    assert!(h.state().engine.session().format_version == session.format_version);
}

#[test]
fn monitor_controls_live_outside_the_document() {
    let mut h = harness();
    let before = h.state().engine.session_arc();
    let undo = h.state().engine.undo_depth();
    h.state_mut().run("monitor.volume", json!({"db": -12.0})).unwrap();
    h.state_mut().run("monitor.dim", json!({})).unwrap();
    h.state_mut().run("monitor.mono", json!({"value": true})).unwrap();
    let m = h.state().ui.monitor;
    assert_eq!((m.volume_db, m.dim, m.mono), (-12.0, true, true));
    assert!(std::sync::Arc::ptr_eq(&before, &h.state().engine.session_arc()), "the document is untouched");
    assert_eq!(h.state().engine.undo_depth(), undo);
    assert!(h.state_mut().run("monitor.volume", json!({"db": 6.0})).unwrap()["db"].as_f64().unwrap() <= 0.0, "never above unity");
    // Headphone control only for a real cue path.
    let err = h.state_mut().run("monitor.cue", json!({})).unwrap_err();
    assert!(err.contains("unavailable"), "{err}");
    h.state_mut().engine.session_mut().outputs.push(soundcraft_model::OutputPath {
        name: "Cue 1-2".into(),
        first_channel: 2,
        format: soundcraft_model::ChannelFormat::Stereo,
    });
    let r = h.state_mut().run("monitor.cue", json!({})).unwrap();
    assert_eq!(r["cue"], "Cue 1-2");
    assert!(r["control"].as_str().unwrap().contains("unavailable"), "detected but not relabelled: {r}");
}

fn http(addr: std::net::SocketAddr, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> (u16, Value) {
    let mut s = TcpStream::connect(addr).unwrap();
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Length: {}\r\n", body.len());
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    s.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    s.read_to_string(&mut resp).unwrap();
    let (head, body) = resp.split_once("\r\n\r\n").unwrap();
    let status: u16 = head.lines().next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, serde_json::from_str(body).unwrap_or(Value::Null))
}

/// V-REMOTE through the real host: pair, snapshot, transport and monitor commands executed on
/// the UI thread, a dropped client leaving the transport alone, revocation.
#[test]
fn phone_client_drives_the_host_through_the_queue() {
    let mut h = harness();
    assert!(h.state().remote.is_none(), "opt-in: nothing runs until started");
    let r = h.state_mut().run("remote.start", json!({"bind": "127.0.0.1:0"})).unwrap();
    assert_eq!(r["https"], false);
    let addr = h.state().remote.as_ref().unwrap().local_addr();
    let code = h.state_mut().run("remote.pair", json!({})).unwrap()["code"].as_str().unwrap().to_string();
    h.run_steps(2);
    // The client runs on another thread; the UI thread keeps stepping frames to serve it.
    let client = std::thread::spawn(move || {
        let (st, body) = http(addr, "POST", "/pair", &[], &json!({"code": code, "name": "test phone"}).to_string());
        assert_eq!(st, 200, "{body}");
        let token = body["token"].as_str().unwrap().to_string();
        let auth = format!("Bearer {token}");
        let (st, state) = http(addr, "GET", "/state", &[("Authorization", &auth)], "");
        assert_eq!(st, 200);
        let rev = state["revision"].as_u64().unwrap();
        let (st, r) = http(
            addr,
            "POST",
            "/cmd",
            &[("Authorization", &auth)],
            &json!({"request_id": "p1", "command": "transport.play", "expected_revision": rev}).to_string(),
        );
        assert_eq!(st, 200, "{r}");
        assert_eq!(r["accepted"], true, "{r}");
        let (_, r) = http(addr, "POST", "/cmd", &[("Authorization", &auth)], &json!({"request_id": "m1", "command": "monitor.dim"}).to_string());
        assert_eq!(r["accepted"], true, "{r}");
        let (st, r) = http(addr, "POST", "/cmd", &[("Authorization", &auth)], &json!({"request_id": "x1", "command": "session.close"}).to_string());
        assert_eq!(st, 403, "{r}");
        let (_, state) = http(addr, "GET", "/state", &[("Authorization", &auth)], "");
        (token, state)
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !client.is_finished() && std::time::Instant::now() < deadline {
        h.run_steps(2);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let (token, state) = client.join().unwrap();
    assert!(h.state().engine.transport.playing, "Play from the phone started the transport on the host");
    assert!(h.state().ui.monitor.dim, "a UI-layer command ran through the same queue");
    assert_eq!(state["state"]["transport"]["playing"], true);
    assert_eq!(state["state"]["monitor"]["dim"], true);
    assert!(state["state"]["limits"].as_str().unwrap().contains("HTTP"));
    // The phone goes away: nothing stops.
    h.run_steps(10);
    assert!(h.state().engine.transport.playing);
    // Revoke from the desktop, then the old token is dead.
    let id = h.state().remote.as_ref().unwrap().tokens()[0].id.clone();
    h.state_mut().run("remote.revoke", json!({"id": id})).unwrap();
    let auth = format!("Bearer {token}");
    let (st, _) = http(addr, "GET", "/state", &[("Authorization", &auth)], "");
    assert_eq!(st, 401);
    h.state_mut().run("remote.stop", json!({})).unwrap();
    assert!(h.state().remote.is_none());
}
