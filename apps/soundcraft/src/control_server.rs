//! Loopback JSON-lines control server: one request per line, one reply per line. The transport
//! and its authentication (protocol 2, HF21) live in `soundcraft_automation::server`; this
//! module hands each admitted request to the UI thread, which runs it through the app's command
//! path (so the recording guard and every other refusal apply), and waits for the reply.

use serde_json::{Value, json};
use soundcraft_ui_egui::ControlRequest;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Starts the server on `127.0.0.1:port` with the token in `token_path` (created when missing).
/// Requests arrive on the returned receiver. Without a settings folder there is no token, so the
/// control channel stays off (never unauthenticated).
pub fn start(port: u16, ctx: egui::Context, token_path: Option<std::path::PathBuf>) -> Receiver<ControlRequest> {
    let (tx, rx) = channel::<ControlRequest>();
    let Some(token_path) = token_path else {
        log::error!("control server not started: no settings folder for the control token (SOUNDCRAFT_CONFIG_DIR is empty)");
        return rx;
    };
    let tx = Mutex::new(tx);
    let dispatch: soundcraft_automation::server::Dispatch = Arc::new(move |method: String, params: Value| {
        let (req, rrx) = ControlRequest::new(method, params);
        let sent = tx.lock().map(|t| t.send(req).is_ok()).unwrap_or(false);
        if !sent {
            return json!({"ok": false, "code": "closed", "error": "the app is closing"});
        }
        ctx.request_repaint();
        // A reply that takes longer is reported as a timeout; the request itself is not cancelled.
        rrx.recv_timeout(Duration::from_secs(90)).unwrap_or_else(
            |_| json!({"ok": false, "code": "timeout", "error": "timeout (the request was not cancelled; check the state before retrying)"}),
        )
    });
    match soundcraft_automation::server::start(port, token_path.clone(), soundcraft_automation::server::Limits::default(), dispatch) {
        Ok(actual) => {
            log::info!("control server listening on 127.0.0.1:{actual} (token file {})", token_path.display());
            println!("SOUNDCRAFT_CONTROL_PORT={actual}");
        }
        Err(e) => log::error!("{e}"),
    }
    rx
}
