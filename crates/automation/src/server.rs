//! The app's local control server (JSON lines over TCP, loopback only), protocol 2 (HF21).
//!
//! Admission: the first message of a connection must be `auth` with the current token
//! ([`crate::auth`]); until then nothing is dispatched. A wrong, missing or revoked token, any
//! other first message (an old protocol-1 client, malformed JSON, an HTTP request) gets one error
//! line and the connection is closed. Bounds: [`Limits`] (message size, time to authenticate,
//! idle time, concurrent connections). The token file is re-read for each new connection, so a
//! rotation or revocation applies to the next connection at once; existing authenticated
//! connections keep working until they close. Commands still pass through the app's normal
//! command path, so the recording guard and every other refusal apply (authentication does not
//! replace them).

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// Server bounds.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Largest accepted message (bytes, one line).
    pub max_message: usize,
    /// Time a new connection has to send its `auth` message.
    pub auth_timeout: Duration,
    /// An authenticated connection that sends nothing for this long is closed.
    pub idle_timeout: Duration,
    /// Connections served at once; more are refused.
    pub max_connections: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_message: 1 << 20, auth_timeout: Duration::from_secs(10), idle_timeout: Duration::from_secs(30 * 60), max_connections: 8 }
    }
}

/// Runs one request on the app (the app's dispatcher) and returns the reply object
/// (`{"ok":…, "result"|"error":…}`).
pub type Dispatch = Arc<dyn Fn(String, Value) -> Value + Send + Sync>;

/// Binds `127.0.0.1:port` (0 = any free port), makes sure a token exists in `token_path`, and
/// serves connections on background threads. Returns the bound port.
pub fn start(port: u16, token_path: PathBuf, limits: Limits, dispatch: Dispatch) -> Result<u16, String> {
    crate::auth::ensure(&token_path)?;
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("control server failed to bind 127.0.0.1:{port}: {e}"))?;
    let actual = listener.local_addr().map(|a| a.port()).unwrap_or(port);
    let active = Arc::new(AtomicUsize::new(0));
    std::thread::Builder::new()
        .name("soundcraft-control".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let (d, p, a) = (Arc::clone(&dispatch), token_path.clone(), Arc::clone(&active));
                if a.fetch_add(1, Ordering::AcqRel) >= limits.max_connections {
                    a.fetch_sub(1, Ordering::AcqRel);
                    let mut s = stream;
                    let _ = writeln!(
                        s,
                        "{}",
                        json!({"ok": false, "code": "busy", "error": format!("too many control connections (limit {})", limits.max_connections)})
                    );
                    continue;
                }
                let spawned = std::thread::Builder::new().name("soundcraft-control-conn".into()).spawn(move || {
                    serve(stream, &p, limits, &d);
                    a.fetch_sub(1, Ordering::AcqRel);
                });
                if spawned.is_err() {
                    active.fetch_sub(1, Ordering::AcqRel);
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(actual)
}

/// Reads one line of at most `max` bytes. `Ok(None)` at end of stream; `Err` for an oversized
/// line or a read error/timeout.
fn read_line_bounded(r: &mut impl BufRead, max: usize) -> Result<Option<String>, &'static str> {
    let mut buf = Vec::new();
    let n = r.by_ref().take(max as u64 + 1).read_until(b'\n', &mut buf).map_err(|_| "read timeout or error")?;
    if n == 0 {
        return Ok(None);
    }
    if buf.len() > max {
        return Err("message too large");
    }
    Ok(Some(String::from_utf8_lossy(&buf).trim_end().to_string()))
}

fn reply(out: &mut TcpStream, mut v: Value, id: Value) -> bool {
    if let Some(o) = v.as_object_mut() {
        o.insert("id".into(), id);
    }
    writeln!(out, "{v}").is_ok()
}

fn serve(stream: TcpStream, token_path: &std::path::Path, limits: Limits, dispatch: &Dispatch) {
    let Ok(read) = stream.try_clone() else { return };
    let mut out = stream;
    let _ = out.set_read_timeout(Some(limits.auth_timeout));
    let mut reader = BufReader::new(read);
    // 1. Authentication: the first line, within auth_timeout.
    let first = match read_line_bounded(&mut reader, limits.max_message) {
        Ok(Some(l)) => l,
        Ok(None) => return,
        Err(e) => {
            let _ = reply(&mut out, json!({"ok": false, "code": "unauthorized", "error": format!("{e} before authentication")}), Value::Null);
            return;
        }
    };
    let msg: Value = serde_json::from_str(&first).unwrap_or(Value::Null);
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    if msg.get("method").and_then(Value::as_str) != Some("auth") {
        let _ = reply(&mut out, json!({"ok": false, "code": "auth_required", "error": crate::auth::migration_message(Some(token_path))}), id);
        return;
    }
    let p = msg.get("params").cloned().unwrap_or(Value::Null);
    let presented = p.get("token").and_then(Value::as_str).unwrap_or("");
    let ok = crate::auth::load(token_path).is_some_and(|t| crate::auth::verify(&t, presented));
    if !ok {
        log::warn!("control: refused a connection (invalid, missing or revoked token)");
        let _ = reply(&mut out, json!({"ok": false, "code": "unauthorized", "error": "invalid, missing or revoked control token"}), id);
        return;
    }
    let want = p.get("protocol").and_then(Value::as_u64).unwrap_or(crate::auth::PROTOCOL);
    if want > crate::auth::PROTOCOL {
        let _ = reply(
            &mut out,
            json!({"ok": false, "code": "protocol", "error": format!("this SoundCraft speaks control protocol {}; the client asked for {want}", crate::auth::PROTOCOL)}),
            id,
        );
        return;
    }
    let client: String = p.get("client").and_then(Value::as_str).unwrap_or("client").chars().take(64).collect();
    log::info!("control: `{client}` authenticated");
    if !reply(
        &mut out,
        json!({"ok": true, "result": {"protocol": crate::auth::PROTOCOL, "app_version": env!("CARGO_PKG_VERSION"), "authenticated": true}}),
        id,
    ) {
        return;
    }
    // 2. Requests.
    let _ = out.set_read_timeout(Some(limits.idle_timeout));
    loop {
        let line = match read_line_bounded(&mut reader, limits.max_message) {
            Ok(Some(l)) => l,
            Ok(None) => break,
            Err(e) => {
                let _ = reply(&mut out, json!({"ok": false, "code": "bad_request", "error": e}), Value::Null);
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let r = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => {
                let id = msg.get("id").cloned().unwrap_or(Value::Null);
                let method = msg.get("method").and_then(Value::as_str).unwrap_or("").to_string();
                let params = msg.get("params").cloned().unwrap_or(json!({}));
                (dispatch(method, params), id)
            }
            Err(e) => (json!({"ok": false, "code": "bad_request", "error": format!("bad JSON: {e}")}), Value::Null),
        };
        if !reply(&mut out, r.0, r.1) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Backend, Remote};

    fn server(dir: &std::path::Path, limits: Limits) -> (u16, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&calls);
        let d: Dispatch = Arc::new(move |m, p| {
            c.fetch_add(1, Ordering::SeqCst);
            json!({"ok": true, "result": {"method": m, "params": p}})
        });
        let port = start(0, crate::auth::token_path(dir), limits, d).unwrap();
        (port, calls)
    }

    fn raw(port: u16, lines: &[&str]) -> Vec<Value> {
        let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut w = s.try_clone().unwrap();
        let mut r = BufReader::new(s);
        let mut out = Vec::new();
        for l in lines {
            if writeln!(w, "{l}").is_err() {
                break;
            }
            let mut buf = String::new();
            match r.read_line(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => out.push(serde_json::from_str(&buf).unwrap_or(Value::Null)),
            }
        }
        out
    }

    /// No token, a wrong one, an old protocol-1 command, malformed JSON and an HTTP request are
    /// all refused before any dispatch; the right token is admitted; rotation and revocation apply
    /// to the next connection.
    #[test]
    fn admission_rotation_and_revocation() {
        let dir = std::env::temp_dir().join(format!("sml-ctl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (port, calls) = server(&dir, Limits::default());
        let path = crate::auth::token_path(&dir);
        let token = crate::auth::load(&path).expect("the server created a token");
        // Old client: a command first.
        let r = raw(port, &[r#"{"id":1,"method":"engine.execute","params":{"command":"session.close"}}"#, r#"{"id":2,"method":"session.inspect"}"#]);
        assert_eq!(r.len(), 1, "one error, then closed");
        assert_eq!(r[0]["code"], "auth_required");
        assert!(r[0]["error"].as_str().unwrap().contains("control-token"));
        for bad in [r#"{"method":"auth","params":{}}"#, r#"{"method":"auth","params":{"token":"smlc_0000"}}"#, "not json", "GET / HTTP/1.1"] {
            let r = raw(port, &[bad, r#"{"method":"session.inspect"}"#]);
            assert_eq!(r.len(), 1, "{bad}: closed after the refusal");
            assert_eq!(r[0]["ok"], false, "{bad}");
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0, "nothing was dispatched");
        // The right token.
        let mut c = Remote::with_token(&port.to_string(), Some(token.clone()));
        let v = c.call("session.inspect", json!({})).unwrap();
        assert_eq!(v["method"], "session.inspect");
        assert_eq!(c.server.as_ref().unwrap()["protocol"], json!(crate::auth::PROTOCOL));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // Rotate: the open connection keeps working; a new one needs the new token.
        let new = crate::auth::rotate(&path).unwrap();
        assert!(c.call("session.inspect", json!({})).is_ok());
        let err = Remote::with_token(&port.to_string(), Some(token.clone())).call("session.inspect", json!({})).unwrap_err();
        assert!(err.contains("unauthorized"), "{err}");
        assert!(!err.contains(&token[5..]), "the token is never echoed");
        assert!(Remote::with_token(&port.to_string(), Some(new.clone())).call("session.inspect", json!({})).is_ok());
        // Revoke: nobody new gets in.
        crate::auth::revoke(&path).unwrap();
        assert!(Remote::with_token(&port.to_string(), Some(new)).call("session.inspect", json!({})).is_err());
        assert!(Remote::with_token(&port.to_string(), None).call("session.inspect", json!({})).unwrap_err().contains("no control token"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Oversized messages, slow (silent) clients and too many connections are bounded.
    #[test]
    fn bounds() {
        let dir = std::env::temp_dir().join(format!("sml-ctl-b-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let limits = Limits { max_message: 4096, auth_timeout: Duration::from_millis(300), idle_timeout: Duration::from_secs(5), max_connections: 2 };
        let (port, calls) = server(&dir, limits);
        let token = crate::auth::load(&crate::auth::token_path(&dir)).unwrap();
        // Oversized first message.
        let big = format!(r#"{{"method":"auth","params":{{"token":"{}"}}}}"#, "x".repeat(5000));
        let r = raw(port, &[&big]);
        assert_eq!(r[0]["ok"], false);
        // A silent client is dropped after the auth timeout.
        let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let t0 = std::time::Instant::now();
        let mut buf = String::new();
        let _ = BufReader::new(s).read_line(&mut buf);
        assert!(t0.elapsed() < Duration::from_secs(2), "closed after the auth timeout");
        // Connection limit: two held open, the third refused with `busy`.
        let hold: Vec<TcpStream> = (0..2).map(|_| TcpStream::connect(("127.0.0.1", port)).unwrap()).collect();
        std::thread::sleep(Duration::from_millis(100));
        let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let mut buf = String::new();
        let _ = BufReader::new(s).read_line(&mut buf);
        assert!(buf.contains("busy"), "{buf}");
        drop(hold);
        std::thread::sleep(Duration::from_millis(500));
        // An oversized request after authentication closes that connection only.
        let mut c = Remote::with_token(&port.to_string(), Some(token));
        assert!(c.call("x", json!({})).is_ok());
        assert!(c.call("x", json!({"blob": "y".repeat(5000)})).is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
