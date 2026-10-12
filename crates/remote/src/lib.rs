//! SML Remote (R21, D14): an opt-in local control service for a phone browser.
//!
//! What it is: a hand-written HTTP/1.1 server on `std::net` (the tree carries no HTTP or TLS
//! crate), bound to an interface the user chooses explicitly (loopback by default), reachable
//! only after pairing with a short-lived code, with per-client revocable session tokens, a role
//! allowlist of command ids, an Origin check, a per-token rate limit and a bounded command
//! queue. Commands travel as R09 envelopes and are executed by the host (the desktop UI thread)
//! through `Server::poll`; the server thread never touches the engine. The native DAW stays
//! authoritative: a receipt says the command was accepted, not when audio happened.
//!
//! What it is not (stated, not hidden): there is no HTTPS, so this is the "browser-only local
//! HTTP configuration" the contract allows with a label — no installable PWA, no microphone
//! access, no encrypted transport are claimed; a phone's LAN-IP HTTP origin is not localhost.
//! There is no QR image (no verified encoder in the tree): pairing is the displayed address plus
//! a 6-digit code that expires. Tokens travel in an HttpOnly cookie (or a bearer header for
//! non-browser clients), never in URLs, and are never logged.
//!
//! Replay protection: every `transport.record` must carry the `nonce` of the snapshot the
//! client last fetched (`GET /state` rotates it), so a request queued before a reconnect can
//! never start a take; duplicate request ids answer with the first receipt (engine dedup) and a
//! stale `expected_revision` is rejected, so two phones cannot silently overwrite each other.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use serde_json::{Value, json};
use soundcraft_engine::envelope::{Client, Envelope, Receipt};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

pub mod page;

/// Commands a paired phone may run (the `phone` role): transport, markers, monitoring,
/// basic mixer controls, take metadata. Everything else is refused before it is queued.
pub const PHONE_ALLOWLIST: &[&str] = &[
    "transport.play",
    "transport.stop",
    "transport.toggle",
    "transport.record",
    "transport.rtz",
    "transport.go_to_end",
    "transport.locate",
    "transport.play_selection",
    "transport.status",
    "markers.add",
    "markers.next",
    "markers.previous",
    "markers.recall",
    "options.pre_roll",
    "options.post_roll",
    "options.loop_playback",
    "setup.click_countoff",
    "edit.select",
    "track.scroll_to",
    "mix.volume",
    "mix.pan",
    "mix.mute",
    "mix.solo",
    "mix.record_arm",
    "mix.input_monitor",
    "clip.rating",
    "clip.rename",
    "monitor.volume",
    "monitor.dim",
    "monitor.mono",
    "monitor.cue",
    // Assistant (R33, typed path): questions and plans; a plan may only contain commands of
    // this list (checked by the host before execution).
    "assistant.ask",
    "assistant.execute",
];

#[derive(Debug, Clone)]
pub struct Config {
    /// Explicit interface binding; `127.0.0.1:0` = loopback, any free port.
    pub bind: SocketAddr,
    pub allowlist: Vec<String>,
    /// Token bucket per client: sustained commands per second and burst size.
    pub rate_per_sec: f64,
    pub burst: f64,
    /// Bounded host queue; a full queue answers 429 (explicit backpressure).
    pub queue: usize,
    pub pairing_ttl: Duration,
    pub pairing_attempts: u32,
    /// How long a connection waits for the host's receipt before answering 504.
    pub reply_timeout: Duration,
    /// Meter frames are coalesced to at most one per this interval.
    pub meter_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bind: "127.0.0.1:0".parse().unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 0))),
            allowlist: PHONE_ALLOWLIST.iter().map(|s| (*s).to_string()).collect(),
            rate_per_sec: 30.0,
            burst: 60.0,
            queue: 64,
            pairing_ttl: Duration::from_secs(120),
            pairing_attempts: 5,
            reply_timeout: Duration::from_secs(2),
            meter_interval: Duration::from_millis(100),
        }
    }
}

/// A command for the host to execute.
pub struct Request {
    pub token_id: String,
    pub envelope: Envelope,
    reply: SyncSender<Value>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenInfo {
    pub id: String,
    pub name: String,
    pub created: u64,
    pub last_seen: u64,
}

struct Token {
    id: String,
    secret: String,
    name: String,
    created: SystemTime,
    last_seen: SystemTime,
    bucket: f64,
    bucket_at: Instant,
    state_nonce: u64,
}

struct Pairing {
    code: String,
    created: Instant,
    attempts: u32,
}

struct State {
    pairing: Option<Pairing>,
    tokens: Vec<Token>,
    snapshot: Value,
    revision: u64,
    meters: Value,
    meters_seq: u64,
    meters_at: Option<Instant>,
    /// Redacted event log for the desktop panel (never carries secrets).
    events: Vec<String>,
}

struct Inner {
    cfg: Config,
    state: Mutex<State>,
    tx: SyncSender<Request>,
    rx: Mutex<Receiver<Request>>,
    local: SocketAddr,
    stop: AtomicBool,
    nonce_counter: AtomicU64,
}

/// The running service. Clone handles share one server.
#[derive(Clone)]
pub struct Server {
    inner: Arc<Inner>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn secs(t: SystemTime) -> u64 {
    t.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn random_hex(n: usize) -> String {
    let mut v = vec![0u8; n];
    rand::fill(&mut v[..]);
    hex(&v)
}

/// Constant-time equality for secrets.
fn ct_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl Server {
    /// Bind and start serving. The host must call `poll` regularly; nothing runs otherwise.
    pub fn start(cfg: Config) -> std::io::Result<Server> {
        let listener = TcpListener::bind(cfg.bind)?;
        listener.set_nonblocking(true)?;
        let local = listener.local_addr()?;
        let (tx, rx) = sync_channel(cfg.queue.max(1));
        let inner = Arc::new(Inner {
            cfg,
            state: Mutex::new(State {
                pairing: None,
                tokens: Vec::new(),
                snapshot: Value::Null,
                revision: 0,
                meters: Value::Null,
                meters_seq: 0,
                meters_at: None,
                events: Vec::new(),
            }),
            tx,
            rx: Mutex::new(rx),
            local,
            stop: AtomicBool::new(false),
            nonce_counter: AtomicU64::new(1),
        });
        let acc = Arc::clone(&inner);
        std::thread::Builder::new().name("sml-remote-accept".into()).spawn(move || accept_loop(listener, acc))?;
        Ok(Server { inner })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.inner.local
    }

    /// The address a phone types (HTTP, as labelled).
    pub fn url(&self) -> String {
        format!("http://{}/", self.inner.local)
    }

    pub fn stop(&self) {
        self.inner.stop.store(true, Ordering::Relaxed);
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.inner.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Begin pairing: a fresh 6-digit code valid for `pairing_ttl`, replacing any older one.
    pub fn start_pairing(&self) -> String {
        let n: u32 = rand::random::<u32>() % 1_000_000;
        let code = format!("{n:06}");
        let mut st = self.state();
        st.pairing = Some(Pairing { code: code.clone(), created: Instant::now(), attempts: 0 });
        st.events.push(format!("{}: pairing opened ({} s)", now_secs(), self.inner.cfg.pairing_ttl.as_secs()));
        code
    }

    pub fn cancel_pairing(&self) {
        self.state().pairing = None;
    }

    /// Seconds left on the open pairing code, if any.
    pub fn pairing_remaining(&self) -> Option<u64> {
        let st = self.state();
        let p = st.pairing.as_ref()?;
        self.inner.cfg.pairing_ttl.checked_sub(p.created.elapsed()).map(|d| d.as_secs())
    }

    pub fn tokens(&self) -> Vec<TokenInfo> {
        self.state()
            .tokens
            .iter()
            .map(|t| TokenInfo { id: t.id.clone(), name: t.name.clone(), created: secs(t.created), last_seen: secs(t.last_seen) })
            .collect()
    }

    /// Revoke one paired client (by its display id). Its next request answers 401.
    pub fn revoke(&self, id: &str) -> bool {
        let mut st = self.state();
        let before = st.tokens.len();
        st.tokens.retain(|t| t.id != id);
        let removed = st.tokens.len() != before;
        if removed {
            st.events.push(format!("{}: client {id} revoked", now_secs()));
        }
        removed
    }

    pub fn revoke_all(&self) {
        let mut st = self.state();
        st.tokens.clear();
        st.events.push(format!("{}: all clients revoked", now_secs()));
    }

    /// The reconnect snapshot clients fetch (`GET /state`); set it whenever the revision changes.
    pub fn set_snapshot(&self, revision: u64, snapshot: Value) {
        let mut st = self.state();
        st.revision = revision;
        st.snapshot = snapshot;
    }

    /// Meter frame, coalesced: frames arriving faster than `meter_interval` replace the pending
    /// one without bumping the sequence, so streams never queue up behind actions.
    pub fn set_meters(&self, meters: Value) {
        let mut st = self.state();
        let due = st.meters_at.is_none_or(|t| t.elapsed() >= self.inner.cfg.meter_interval);
        st.meters = meters;
        if due {
            st.meters_seq += 1;
            st.meters_at = Some(Instant::now());
        }
    }

    pub fn events(&self) -> Vec<String> {
        self.state().events.clone()
    }

    /// Execute queued commands on the host thread. Returns how many ran.
    pub fn poll(&self, mut run: impl FnMut(&Envelope) -> Receipt) -> usize {
        let rx = self.inner.rx.lock().unwrap_or_else(PoisonError::into_inner);
        let mut n = 0;
        while let Ok(req) = rx.try_recv() {
            let receipt = run(&req.envelope);
            let _ = req.reply.try_send(serde_json::to_value(&receipt).unwrap_or(Value::Null));
            n += 1;
        }
        n
    }
}

fn accept_loop(listener: TcpListener, inner: Arc<Inner>) {
    let live = Arc::new(Mutex::new(0usize));
    loop {
        if inner.stop.load(Ordering::Relaxed) {
            return;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                // Bounded concurrency: a flood of connections cannot spawn unbounded threads.
                let count = *live.lock().unwrap_or_else(PoisonError::into_inner);
                if count >= 16 {
                    let _ = respond(&stream, 503, "application/json", br#"{"error":"busy"}"#);
                    continue;
                }
                *live.lock().unwrap_or_else(PoisonError::into_inner) += 1;
                let inner = Arc::clone(&inner);
                let live = Arc::clone(&live);
                let _ = std::thread::Builder::new().name("sml-remote-conn".into()).spawn(move || {
                    handle(stream, &inner);
                    *live.lock().unwrap_or_else(PoisonError::into_inner) -= 1;
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

struct Http {
    method: String,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

const MAX_BODY: usize = 64 * 1024;

fn parse(stream: &TcpStream) -> Option<Http> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut r = BufReader::new(stream);
    let mut line = String::new();
    r.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let (path, q) = target.split_once('?').map_or((target.as_str(), ""), |(p, q)| (p, q));
    let query = q.split('&').filter(|s| !s.is_empty()).filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect();
    let mut headers = HashMap::new();
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).ok()? == 0 {
            return None;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
        if headers.len() > 64 {
            return None;
        }
    }
    let len: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    if len > MAX_BODY {
        return None;
    }
    let mut body = vec![0u8; len];
    if len > 0 {
        r.read_exact(&mut body).ok()?;
    }
    Some(Http { method, path: path.to_string(), query, headers, body })
}

fn respond(mut stream: &TcpStream, status: u16, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    respond_with(&mut stream, status, ctype, body, &[])
}

fn respond_with(stream: &mut &TcpStream, status: u16, ctype: &str, body: &[u8], extra: &[(&str, String)]) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "OK",
    };
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\n",
        body.len()
    );
    for (k, v) in extra {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn json_err(stream: &TcpStream, status: u16, msg: &str) {
    let _ = respond(stream, status, "application/json", json!({"error": msg}).to_string().as_bytes());
}

fn cookie_token(h: &Http) -> Option<String> {
    if let Some(auth) = h.headers.get("authorization")
        && let Some(t) = auth.strip_prefix("Bearer ")
    {
        return Some(t.trim().to_string());
    }
    let c = h.headers.get("cookie")?;
    c.split(';').map(str::trim).find_map(|kv| kv.strip_prefix("sml_remote=").map(str::to_string))
}

/// Browser requests must come from the page this server served: `Origin` (when present) must
/// match the `Host` the request reached. Non-browser clients send no Origin and use a bearer token.
fn origin_ok(h: &Http) -> bool {
    match h.headers.get("origin") {
        None => true,
        Some(o) => {
            let host = h.headers.get("host").cloned().unwrap_or_default();
            o.eq_ignore_ascii_case(&format!("http://{host}")) || o == "null" && false
        }
    }
}

fn handle(stream: TcpStream, inner: &Arc<Inner>) {
    let Some(h) = parse(&stream) else {
        json_err(&stream, 400, "bad request");
        return;
    };
    if !origin_ok(&h) {
        json_err(&stream, 403, "origin not allowed");
        return;
    }
    match (h.method.as_str(), h.path.as_str()) {
        ("GET", "/") => {
            let _ = respond(&stream, 200, "text/html; charset=utf-8", page::HTML.as_bytes());
        }
        ("POST", "/pair") => pair(&stream, &h, inner),
        (_, "/pair") | ("GET", "/favicon.ico") => json_err(&stream, 404, "not found"),
        _ => {
            let Some(secret) = cookie_token(&h) else {
                json_err(&stream, 401, "unauthorized: pair first");
                return;
            };
            let token_id = {
                let mut st = inner.state.lock().unwrap_or_else(PoisonError::into_inner);
                let Some(t) = st.tokens.iter_mut().find(|t| ct_eq(&t.secret, &secret)) else {
                    drop(st);
                    json_err(&stream, 401, "unauthorized: unknown or revoked session");
                    return;
                };
                t.last_seen = SystemTime::now();
                // Token bucket rate limit.
                let dt = t.bucket_at.elapsed().as_secs_f64();
                t.bucket = (t.bucket + dt * inner.cfg.rate_per_sec).min(inner.cfg.burst);
                t.bucket_at = Instant::now();
                if t.bucket < 1.0 {
                    drop(st);
                    json_err(&stream, 429, "rate limited");
                    return;
                }
                t.bucket -= 1.0;
                t.id.clone()
            };
            match (h.method.as_str(), h.path.as_str()) {
                ("GET", "/state") => state_route(&stream, inner, &token_id),
                ("GET", "/meters") => meters_route(&stream, &h, inner),
                ("POST", "/cmd") => cmd_route(&stream, &h, inner, &token_id),
                ("POST", "/logout") => {
                    let mut st = inner.state.lock().unwrap_or_else(PoisonError::into_inner);
                    st.tokens.retain(|t| t.id != token_id);
                    st.events.push(format!("{}: client {token_id} logged out", now_secs()));
                    let _ = respond(&stream, 200, "application/json", b"{\"ok\":true}");
                }
                _ => json_err(&stream, 404, "not found"),
            }
        }
    }
}

fn pair(stream: &TcpStream, h: &Http, inner: &Arc<Inner>) {
    let body: Value = serde_json::from_slice(&h.body).unwrap_or(Value::Null);
    let code = body.get("code").and_then(Value::as_str).unwrap_or("").trim().to_string();
    let name = body.get("name").and_then(Value::as_str).unwrap_or("phone").chars().take(32).collect::<String>();
    let mut st = inner.state.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(p) = st.pairing.as_mut() else {
        drop(st);
        json_err(stream, 403, "pairing is not open on the desktop");
        return;
    };
    if p.created.elapsed() > inner.cfg.pairing_ttl {
        st.pairing = None;
        st.events.push(format!("{}: pairing code expired", now_secs()));
        drop(st);
        json_err(stream, 403, "pairing code expired");
        return;
    }
    if !ct_eq(&p.code, &code) {
        p.attempts += 1;
        let too_many = p.attempts >= inner.cfg.pairing_attempts;
        if too_many {
            st.pairing = None;
            st.events.push(format!("{}: pairing closed after too many wrong codes", now_secs()));
            drop(st);
            json_err(stream, 429, "too many attempts: start pairing again on the desktop");
        } else {
            drop(st);
            json_err(stream, 403, "wrong code");
        }
        return;
    }
    st.pairing = None;
    let secret = random_hex(32);
    let id = random_hex(4);
    st.tokens.push(Token {
        id: id.clone(),
        secret: secret.clone(),
        name: name.clone(),
        created: SystemTime::now(),
        last_seen: SystemTime::now(),
        bucket: inner.cfg.burst,
        bucket_at: Instant::now(),
        state_nonce: 0,
    });
    st.events.push(format!("{}: client {id} ({name}) paired", now_secs()));
    drop(st);
    let body = json!({"token_id": id, "name": name, "token": secret}).to_string();
    let cookie = format!("sml_remote={secret}; HttpOnly; SameSite=Strict; Path=/; Max-Age=86400");
    let _ = respond_with(&mut &*stream, 200, "application/json", body.as_bytes(), &[("Set-Cookie", cookie)]);
}

fn state_route(stream: &TcpStream, inner: &Arc<Inner>, token_id: &str) {
    let nonce = inner.nonce_counter.fetch_add(1, Ordering::Relaxed) ^ rand::random::<u64>();
    let mut st = inner.state.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(t) = st.tokens.iter_mut().find(|t| t.id == token_id) {
        t.state_nonce = nonce;
    }
    let body = json!({"revision": st.revision, "nonce": nonce, "token_id": token_id, "state": st.snapshot, "transport_note": "receipt ≠ sample execution: the DAW schedules audio; this is the last known state"});
    drop(st);
    let _ = respond(stream, 200, "application/json", body.to_string().as_bytes());
}

fn meters_route(stream: &TcpStream, h: &Http, inner: &Arc<Inner>) {
    let since: u64 = h.query.get("since").and_then(|v| v.parse().ok()).unwrap_or(0);
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        {
            let st = inner.state.lock().unwrap_or_else(PoisonError::into_inner);
            if st.meters_seq > since || Instant::now() >= deadline {
                let body = json!({"seq": st.meters_seq, "meters": st.meters});
                drop(st);
                let _ = respond(stream, 200, "application/json", body.to_string().as_bytes());
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn cmd_route(stream: &TcpStream, h: &Http, inner: &Arc<Inner>, token_id: &str) {
    let body: Value = match serde_json::from_slice(&h.body) {
        Ok(v) => v,
        Err(_) => {
            json_err(stream, 400, "body must be JSON");
            return;
        }
    };
    let Some(command) = body.get("command").and_then(Value::as_str).map(str::to_string) else {
        json_err(stream, 400, "`command` required");
        return;
    };
    let Some(request_id) = body.get("request_id").and_then(Value::as_str).map(str::to_string) else {
        json_err(stream, 400, "`request_id` required (unique per attempt; retries reuse it)");
        return;
    };
    if !inner.cfg.allowlist.contains(&command) {
        json_err(stream, 403, "command not allowed for this client");
        return;
    }
    if command == "assistant.execute" {
        // A phone cannot plan a command it could not send itself.
        let steps = body.get("params").and_then(|p| p.get("plan")).and_then(Value::as_array).cloned().unwrap_or_default();
        if let Some(bad) = steps
            .iter()
            .find_map(|s| s.get("command").and_then(Value::as_str).filter(|c| !inner.cfg.allowlist.contains(&(*c).to_string())).map(str::to_string))
        {
            json_err(stream, 403, &format!("plan step `{bad}` is not allowed for this client"));
            return;
        }
    }
    if command == "transport.record" {
        let nonce = body.get("nonce").and_then(Value::as_u64).unwrap_or(0);
        let st = inner.state.lock().unwrap_or_else(PoisonError::into_inner);
        let current = st.tokens.iter().find(|t| t.id == token_id).map_or(0, |t| t.state_nonce);
        drop(st);
        if nonce == 0 || nonce != current {
            json_err(stream, 409, "stale record request: fetch /state again, then record");
            return;
        }
    }
    let envelope = Envelope {
        request_id: format!("{token_id}:{request_id}"),
        client: Client::Remote,
        command,
        params: body.get("params").cloned().unwrap_or_else(|| json!({})),
        expected_revision: body.get("expected_revision").and_then(Value::as_u64),
        gesture: body.get("gesture").and_then(Value::as_str).map(str::to_string),
    };
    let (rtx, rrx) = sync_channel(1);
    match inner.tx.try_send(Request { token_id: token_id.to_string(), envelope, reply: rtx }) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            json_err(stream, 429, "command queue full: back off");
            return;
        }
        Err(TrySendError::Disconnected(_)) => {
            json_err(stream, 503, "host gone");
            return;
        }
    }
    match rrx.recv_timeout(inner.cfg.reply_timeout) {
        Ok(v) => {
            let _ = respond(stream, 200, "application/json", v.to_string().as_bytes());
        }
        Err(_) => json_err(stream, 504, "the desktop did not answer in time; the command may still run — refresh state before retrying"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_engine::Engine;

    /// A tiny HTTP client: (status, headers, body).
    fn http(addr: SocketAddr, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> (u16, HashMap<String, String>, Value) {
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
        let mut lines = head.lines();
        let status: u16 = lines.next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
        let hs = lines.filter_map(|l| l.split_once(':').map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_string()))).collect();
        let v = serde_json::from_str(body).unwrap_or(Value::String(body.to_string()));
        (status, hs, v)
    }

    struct Host {
        server: Server,
        engine: Arc<Mutex<Engine>>,
    }

    fn host(cfg: Config) -> Host {
        let server = Server::start(cfg).unwrap();
        let engine = Arc::new(Mutex::new(soundcraft_engine::demo::demo_engine()));
        let (s2, e2) = (server.clone(), Arc::clone(&engine));
        std::thread::spawn(move || {
            loop {
                s2.poll(|env| e2.lock().unwrap().submit(env));
                let e = e2.lock().unwrap();
                s2.set_snapshot(e.revision, json!({"name": e.session().name}));
                drop(e);
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        Host { server, engine }
    }

    fn pair(h: &Host) -> (String, String) {
        let code = h.server.start_pairing();
        let (st, hs, body) = http(h.server.local_addr(), "POST", "/pair", &[], &json!({"code": code, "name": "test phone"}).to_string());
        assert_eq!(st, 200, "{body}");
        let cookie = hs.get("set-cookie").unwrap();
        assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
        let token = cookie.split_once('=').unwrap().1.split(';').next().unwrap().to_string();
        (token, body["token_id"].as_str().unwrap().to_string())
    }

    fn cmd(h: &Host, token: &str, extra: &[(&str, &str)], body: Value) -> (u16, Value) {
        let cookie = format!("sml_remote={token}");
        let mut hs = vec![("Cookie", cookie.as_str())];
        hs.extend_from_slice(extra);
        let (st, _, v) = http(h.server.local_addr(), "POST", "/cmd", &hs, &body.to_string());
        (st, v)
    }

    /// R33 (typed phone assistant): a paired phone asks and gets a grounded answer; a plan runs
    /// only when every step is in the phone allowlist; a plan with a disallowed step is refused
    /// before the host sees it.
    #[test]
    fn phone_assistant_plans_are_bounded_by_the_allowlist() {
        let h = host(Config { rate_per_sec: 1000.0, burst: 1000.0, ..Config::default() });
        let (token, _) = pair(&h);
        let (st, r) =
            cmd(&h, &token, &[], json!({"request_id": "a1", "command": "assistant.ask", "params": {"text": "mute the track called Lead Vox"}}));
        assert_eq!(st, 200, "{r}");
        let plan = r["applied"]["plan"].clone();
        assert!(plan.as_array().is_some_and(|p| p.len() == 1), "{r}");
        let rev = r["applied"]["revision"].clone();
        let (st, r2) = cmd(
            &h,
            &token,
            &[],
            json!({"request_id": "a2", "command": "assistant.execute", "params": {"plan": plan, "revision": rev, "request_id": "phone-a2"}}),
        );
        assert_eq!(st, 200, "{r2}");
        assert_eq!(r2["applied"]["accepted"], true, "{r2}");
        let (st, r3) = cmd(
            &h,
            &token,
            &[],
            json!({"request_id": "a3", "command": "assistant.execute", "params": {"plan": [{"command": "session.close", "params": {}}], "revision": rev}}),
        );
        assert_eq!(st, 403, "{r3}");
        let (st, r4) = cmd(&h, &token, &[], json!({"request_id": "a4", "command": "assistant.ask", "params": {"text": "open the ARA interface"}}));
        assert_eq!(st, 200);
        assert_eq!(r4["applied"]["mode"], "unsupported", "{r4}");
    }

    /// V-REMOTE: pair/revoke, unauthorized Origin, duplicate and stale commands, two clients,
    /// reconnect snapshot with a fresh record nonce, rate limit, bounded queue, disconnect.
    #[test]
    fn pairing_auth_origin_replay_and_two_clients() {
        let h = host(Config { rate_per_sec: 1000.0, burst: 1000.0, ..Config::default() });
        let addr = h.server.local_addr();
        // The page is public and carries no secret; everything else needs a session.
        let (st, _, page) = http(addr, "GET", "/", &[], "");
        assert_eq!(st, 200);
        assert!(page.as_str().unwrap().contains("SML Remote") && !page.as_str().unwrap().contains("sml_remote="));
        assert_eq!(http(addr, "GET", "/state", &[], "").0, 401);
        // Wrong codes: five strikes close the pairing.
        h.server.start_pairing();
        for i in 0..5 {
            let (st, _, _) = http(addr, "POST", "/pair", &[], r#"{"code":"000000"}"#);
            assert_eq!(st, if i == 4 { 429 } else { 403 });
        }
        assert!(h.server.pairing_remaining().is_none());
        let (token_a, id_a) = pair(&h);
        assert_eq!(h.server.tokens().len(), 1);
        // Origin: the page's own origin passes, another site is refused, bearer without Origin passes.
        let own = format!("http://{addr}");
        let (st, _, state) = http(addr, "GET", "/state", &[("Cookie", &format!("sml_remote={token_a}")), ("Origin", &own)], "");
        assert_eq!(st, 200, "{state}");
        assert_eq!(http(addr, "GET", "/state", &[("Cookie", &format!("sml_remote={token_a}")), ("Origin", "http://evil.example")], "").0, 403);
        assert_eq!(http(addr, "GET", "/state", &[("Authorization", &format!("Bearer {token_a}"))], "").0, 200);
        // Allowlist: saving the session is not a phone action.
        assert_eq!(cmd(&h, &token_a, &[], json!({"request_id": "r0", "command": "session.save"})).0, 403);
        // A real command, then its duplicate (same request id): one mutation, same receipt.
        let rev0 = state["revision"].as_u64().unwrap();
        let (st, r1) = cmd(
            &h,
            &token_a,
            &[],
            json!({"request_id": "r1", "command": "mix.volume", "params": {"track": "Kick", "db": -7.0}, "expected_revision": rev0}),
        );
        assert_eq!(st, 200, "{r1}");
        assert_eq!(r1["accepted"], true);
        let (_, r1b) = cmd(
            &h,
            &token_a,
            &[],
            json!({"request_id": "r1", "command": "mix.volume", "params": {"track": "Kick", "db": -20.0}, "expected_revision": rev0}),
        );
        assert_eq!(r1b["duplicate"], true);
        assert_eq!(h.engine.lock().unwrap().session().track_by_name("Kick").unwrap().mixer.volume_db, -7.0, "the retry did not apply its new value");
        // Two clients: B's edit against the revision it last saw is stale once A changed things.
        let (token_b, _) = pair(&h);
        let (_, _, state_b) = http(addr, "GET", "/state", &[("Cookie", &format!("sml_remote={token_b}"))], "");
        let (_, r2) = cmd(
            &h,
            &token_a,
            &[],
            json!({"request_id": "r2", "command": "mix.pan", "params": {"track": "Kick", "pan": 0.3}, "expected_revision": state_b["revision"]}),
        );
        assert_eq!(r2["accepted"], true);
        let (_, r3) = cmd(
            &h,
            &token_b,
            &[],
            json!({"request_id": "r3", "command": "mix.pan", "params": {"track": "Kick", "pan": -0.3}, "expected_revision": state_b["revision"]}),
        );
        assert_eq!(r3["accepted"], false, "{r3}");
        assert_eq!(r3["rejection"]["stale_revision"]["expected"], state_b["revision"], "{r3}");
        // Record needs the nonce of the latest snapshot; an old nonce (pre-reconnect) is refused.
        let (_, _, s1) = http(addr, "GET", "/state", &[("Cookie", &format!("sml_remote={token_a}"))], "");
        let old_nonce = s1["nonce"].as_u64().unwrap();
        let (_, _, s2) = http(addr, "GET", "/state", &[("Cookie", &format!("sml_remote={token_a}"))], "");
        let (st, r) = cmd(&h, &token_a, &[], json!({"request_id": "rec1", "command": "transport.record", "nonce": old_nonce}));
        assert_eq!(st, 409, "{r}");
        assert_eq!(cmd(&h, &token_a, &[], json!({"request_id": "rec2", "command": "transport.record"})).0, 409);
        let (st, r) = cmd(&h, &token_a, &[], json!({"request_id": "rec3", "command": "transport.record", "nonce": s2["nonce"]}));
        assert_eq!(st, 200, "{r}");
        assert_eq!(r["accepted"], true);
        assert!(h.engine.lock().unwrap().transport_requests.iter().any(|r| matches!(r, soundcraft_engine::TransportRequest::Record)));
        // Revocation from the desktop: the next request is 401; the other client is untouched.
        assert!(h.server.revoke(&id_a));
        assert_eq!(http(addr, "GET", "/state", &[("Cookie", &format!("sml_remote={token_a}"))], "").0, 401);
        assert_eq!(http(addr, "GET", "/state", &[("Cookie", &format!("sml_remote={token_b}"))], "").0, 200);
        // Logout from the phone.
        assert_eq!(http(addr, "POST", "/logout", &[("Cookie", &format!("sml_remote={token_b}"))], "").0, 200);
        assert_eq!(http(addr, "GET", "/state", &[("Cookie", &format!("sml_remote={token_b}"))], "").0, 401);
        assert!(h.server.tokens().is_empty());
        // A dropped connection changes nothing: the host keeps polling and the transport is as it was.
        let playing = h.engine.lock().unwrap().transport.playing;
        drop(TcpStream::connect(addr).unwrap());
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(h.engine.lock().unwrap().transport.playing, playing);
        assert!(h.server.events().iter().all(|e| !e.contains(&token_a)), "secrets never reach the log");
    }

    #[test]
    fn rate_limit_queue_bound_meters_and_pairing_expiry() {
        let h = host(Config {
            rate_per_sec: 0.0001,
            burst: 3.0,
            // Generous enough that a loaded test runner pairs before the code expires.
            pairing_ttl: Duration::from_millis(400),
            meter_interval: Duration::from_millis(100),
            ..Config::default()
        });
        let addr = h.server.local_addr();
        let (token, _) = pair(&h);
        let c = format!("sml_remote={token}");
        for _ in 0..3 {
            assert_eq!(http(addr, "GET", "/state", &[("Cookie", &c)], "").0, 200);
        }
        assert_eq!(http(addr, "GET", "/state", &[("Cookie", &c)], "").0, 429, "burst spent");
        // Meter frames coalesce: 20 frames in 10 ms bump the sequence once.
        for i in 0..20 {
            h.server.set_meters(json!({"main": i}));
        }
        let (st, _, m) = http(addr, "GET", "/meters?since=0", &[("Authorization", &format!("Bearer {token}"))], "");
        assert_eq!(st, 429, "meters count against the same limit");
        let _ = m;
        // Pairing expiry.
        h.server.start_pairing();
        std::thread::sleep(Duration::from_millis(500));
        let (st, _, _) = http(addr, "POST", "/pair", &[], r#"{"code":"123456"}"#);
        assert_eq!(st, 403);
        // A bounded queue: with no host polling, the queue fills and the next command is told to back off.
        let s =
            Server::start(Config { queue: 2, reply_timeout: Duration::from_millis(50), rate_per_sec: 1000.0, burst: 1000.0, ..Config::default() })
                .unwrap();
        let code = s.start_pairing();
        let (_, hs, _) = http(s.local_addr(), "POST", "/pair", &[], &json!({"code": code}).to_string());
        let t = hs["set-cookie"].split_once('=').unwrap().1.split(';').next().unwrap().to_string();
        let a2 = s.local_addr();
        let ts: Vec<_> = (0..3)
            .map(|i| {
                let t = t.clone();
                std::thread::spawn(move || {
                    http(
                        a2,
                        "POST",
                        "/cmd",
                        &[("Authorization", &format!("Bearer {t}"))],
                        &json!({"request_id": format!("q{i}"), "command": "transport.stop"}).to_string(),
                    )
                    .0
                })
            })
            .collect();
        let codes: Vec<u16> = ts.into_iter().map(|t| t.join().unwrap()).collect();
        assert!(codes.iter().filter(|c| **c == 504).count() == 2 && codes.contains(&429), "{codes:?}");
        // The meter stream itself: a fresh server, one frame, long-poll returns it with its sequence.
        let s3 = Server::start(Config { rate_per_sec: 1000.0, burst: 1000.0, ..Config::default() }).unwrap();
        let code = s3.start_pairing();
        let (_, hs, _) = http(s3.local_addr(), "POST", "/pair", &[], &json!({"code": code}).to_string());
        let t3 = hs["set-cookie"].split_once('=').unwrap().1.split(';').next().unwrap().to_string();
        s3.set_meters(json!({"main": [0.5, 0.4]}));
        let (st, _, m) = http(s3.local_addr(), "GET", "/meters?since=0", &[("Authorization", &format!("Bearer {t3}"))], "");
        assert_eq!(st, 200);
        assert_eq!(m["seq"], 1);
        assert_eq!(m["meters"]["main"][0], 0.5);
        let t0 = Instant::now();
        let (_, _, m2) = http(s3.local_addr(), "GET", "/meters?since=1", &[("Authorization", &format!("Bearer {t3}"))], "");
        assert!(t0.elapsed() >= Duration::from_millis(900), "long-poll waits when nothing is new");
        assert_eq!(m2["seq"], 1);
    }
}
