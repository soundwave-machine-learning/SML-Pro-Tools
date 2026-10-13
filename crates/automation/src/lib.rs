//! Agent control for SoundCraft.
//!
//! - [`Backend`]: something that answers control-protocol methods (`engine.execute`,
//!   `session.inspect`, `ui.*`…). [`Headless`] runs an in-process engine; [`Remote`] talks to a
//!   running app's `--control` port.
//! - [`mcp::Server`]: a Model Context Protocol server over stdio (newline-delimited JSON-RPC 2.0)
//!   exposing SoundCraft as tools and resources.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod auth;
pub mod mcp;
pub mod server;

use serde_json::{Value, json};
use soundcraft_engine::Engine;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Answers control-protocol methods.
pub trait Backend {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String>;
    /// True when a UI is attached (screenshots, clicks).
    fn has_ui(&self) -> bool;
    fn describe(&self) -> String;
}

/// An in-process engine (no UI).
pub struct Headless {
    pub engine: Engine,
}

impl Headless {
    pub fn new(engine: Engine) -> Self {
        Headless { engine }
    }
}

impl Backend for Headless {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        match method {
            "engine.execute" | "command" => {
                let id = params.get("command").or_else(|| params.get("id")).and_then(Value::as_str).ok_or("`command` required")?;
                let p = params.get("params").cloned().unwrap_or(json!({}));
                let r = self.engine.execute(id, &p).map_err(|e| e.to_string());
                // Headless has no audio engine: drop transport requests but keep the playhead.
                self.engine.transport_requests.clear();
                r
            }
            "engine.commands" => self.engine.execute("engine.commands", &params).map_err(|e| e.to_string()),
            "engine.parity" => Ok(soundcraft_engine::catalog::parity_json()),
            "session.inspect" | "document.inspect" => {
                Ok(soundcraft_engine::inspect::session(&self.engine, params.get("detail").and_then(Value::as_str) == Some("full")))
            }
            m if m.starts_with("ui.") => {
                Err(format!("`{m}` needs the desktop app: start `soundcraft --control 0` and use `soundcraft-cli mcp --connect PORT`"))
            }
            other => Err(format!("unknown method `{other}`")),
        }
    }
    fn has_ui(&self) -> bool {
        false
    }
    fn describe(&self) -> String {
        format!("headless engine, session `{}`", self.engine.session().name)
    }
}

/// A TCP client for a running app's control channel (protocol 2: authenticated, HF21).
pub struct Remote {
    addr: String,
    token: Option<String>,
    client: String,
    conn: Option<(TcpStream, BufReader<TcpStream>)>,
    next_id: u64,
    /// The server's reply to `auth` (protocol, app version), once connected.
    pub server: Option<Value>,
}

/// Where a request failed: before it reached the app (safe to retry) or after (never retried:
/// it may have run).
enum Failure {
    NotSent(String),
    Sent(String),
}

impl Remote {
    /// `target` is a port (`7979`) or `host:port`. The token comes from `SOUNDCRAFT_CONTROL_TOKEN`
    /// or the token file in the SoundCraft settings folder ([`auth`]).
    pub fn new(target: &str) -> Self {
        let token = std::env::var(auth::TOKEN_ENV)
            .ok()
            .filter(|t| !t.trim().is_empty())
            .map(|t| t.trim().to_string())
            .or_else(|| auth::default_token_path().and_then(|p| auth::load(&p)));
        Remote::with_token(target, token)
    }

    /// Like [`Remote::new`] with an explicit token (or none, to test refusal).
    pub fn with_token(target: &str, token: Option<String>) -> Self {
        let addr = if target.contains(':') { target.to_string() } else { format!("127.0.0.1:{target}") };
        Remote { addr, token, client: "soundcraft-cli".into(), conn: None, next_id: 1, server: None }
    }

    /// Names this client in the app's log (never the token).
    pub fn client_name(mut self, name: &str) -> Self {
        self.client = name.chars().take(64).collect();
        self
    }

    fn connect(&mut self) -> Result<(), String> {
        if self.conn.is_some() {
            return Ok(());
        }
        let sock = self.addr.parse().map_err(|e| format!("{}: {e}", self.addr))?;
        let s =
            TcpStream::connect_timeout(&sock, Duration::from_millis(800)).map_err(|e| format!("cannot reach SoundCraft at {}: {e}", self.addr))?;
        s.set_read_timeout(Some(Duration::from_secs(120))).map_err(|e| e.to_string())?;
        let r = s.try_clone().map_err(|e| e.to_string())?;
        self.conn = Some((s, BufReader::new(r)));
        let token = self.token.clone().ok_or_else(|| {
            self.conn = None;
            format!(
                "no control token: set {} or create one by starting SoundCraft with --control (token file {})",
                auth::TOKEN_ENV,
                auth::default_token_path().map_or("in the settings folder".into(), |p| p.display().to_string())
            )
        })?;
        let hello = json!({"token": token, "client": self.client, "protocol": auth::PROTOCOL});
        match self.exchange("auth", &hello) {
            Ok(v) => {
                self.server = Some(v);
                Ok(())
            }
            Err(Failure::NotSent(e) | Failure::Sent(e)) => {
                self.conn = None;
                Err(e)
            }
        }
    }

    fn exchange(&mut self, method: &str, params: &Value) -> Result<Value, Failure> {
        let id = self.next_id;
        self.next_id += 1;
        let line = json!({"id": id, "method": method, "params": params}).to_string();
        let (w, r) = self.conn.as_mut().ok_or_else(|| Failure::NotSent("not connected".into()))?;
        writeln!(w, "{line}").map_err(|e| Failure::NotSent(e.to_string()))?;
        let mut buf = String::new();
        r.read_line(&mut buf).map_err(|e| Failure::Sent(format!("no reply (the request may have run): {e}")))?;
        if buf.is_empty() {
            return Err(Failure::Sent("connection closed before the reply (the request may have run)".into()));
        }
        let v: Value = serde_json::from_str(&buf).map_err(|e| Failure::Sent(e.to_string()))?;
        if v.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            Ok(v.get("result").cloned().unwrap_or(Value::Null))
        } else {
            let msg = v.get("error").and_then(Value::as_str).unwrap_or("error").to_string();
            let code = v.get("code").and_then(Value::as_str).map(|c| format!("[{c}] ")).unwrap_or_default();
            Err(Failure::Sent(format!("{code}{msg}")))
        }
    }

    fn roundtrip(&mut self, method: &str, params: &Value) -> Result<Value, Failure> {
        self.connect().map_err(Failure::NotSent)?;
        self.exchange(method, params)
    }
}

impl Backend for Remote {
    /// One attempt; a request is retried once on a fresh connection only when it never reached
    /// the app (a stale connection that failed on write). A request that was sent is never
    /// replayed automatically: it may have run (HF22).
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        match self.roundtrip(method, &params) {
            Ok(v) => Ok(v),
            Err(Failure::NotSent(_)) if self.conn.is_some() => {
                self.conn = None;
                self.roundtrip(method, &params).map_err(|(Failure::NotSent(e) | Failure::Sent(e))| e)
            }
            Err(Failure::NotSent(e)) | Err(Failure::Sent(e)) => {
                if self.conn.as_ref().is_some() && e.contains("closed") {
                    self.conn = None;
                }
                Err(e)
            }
        }
    }
    fn has_ui(&self) -> bool {
        true
    }
    fn describe(&self) -> String {
        format!("SoundCraft app at {}", self.addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_executes_and_inspects() {
        let mut h = Headless::new(Engine::default());
        let r = h.call("engine.execute", json!({"command": "track.new", "params": {"count": 2, "format": "stereo"}}));
        assert!(r.is_ok(), "{r:?}");
        let s = h.call("session.inspect", json!({})).unwrap();
        assert_eq!(s["tracks"].as_array().map(Vec::len), Some(2));
        assert!(h.call("ui.click", json!({})).is_err());
        assert!(h.call("nope", json!({})).is_err());
    }

    #[test]
    fn remote_reports_unreachable() {
        let mut r = Remote::new("127.0.0.1:1");
        assert!(r.call("session.inspect", json!({})).is_err());
    }
}
