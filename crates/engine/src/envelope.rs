//! Command envelopes (R09): what every client — GUI, shortcuts, the control channel, the LAN
//! remote and the assistant — submits, and the receipt it gets back.
//!
//! An envelope names the command and its typed parameters, the client, a unique request id
//! and the session revision the client last saw. The engine validates before mutating:
//! a duplicate request id (a remote retry) returns the receipt of the first delivery and
//! mutates nothing; a stale expected revision is rejected visibly; the command's own `enabled`
//! predicate and parameter validation still apply. The receipt reports accepted/rejected, the
//! applied value or the error, and the resulting revision. Receipt is not sample execution:
//! transport commands schedule audio events whose execution time the player reports separately.
//!
//! Continuous controls (`gesture`) coalesce through [`Engine::execute_merged`]: consecutive
//! values of one gesture form one undo step; the final value is never dropped. Edits and
//! transport transitions never coalesce.

use crate::{Engine, EngineError};
use serde_json::{Value, json};
use std::collections::VecDeque;

/// Most remembered request ids (per engine); older ones are forgotten (bounded memory).
pub const DEDUP_WINDOW: usize = 4096;

/// Who submitted a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Client {
    Gui,
    Shortcut,
    ControlChannel,
    Remote,
    Assistant,
    Cli,
}

/// A command submission.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Envelope {
    /// Unique per submission (a retry reuses it). Empty = no deduplication.
    #[serde(default)]
    pub request_id: String,
    pub client: Client,
    pub command: String,
    #[serde(default)]
    pub params: Value,
    /// The session revision the client last observed; `None` = unconditional.
    #[serde(default)]
    pub expected_revision: Option<u64>,
    /// Continuous-control gesture: consecutive envelopes with the same key merge into one undo
    /// step (fader drags). `None` for discrete edits and transport transitions.
    #[serde(default)]
    pub gesture: Option<String>,
}

/// Why an envelope was rejected before any mutation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rejection {
    /// `expected_revision` is behind the engine: the client must refresh its snapshot.
    StaleRevision { expected: u64, current: u64 },
    /// The command (or its target) is not available; the reason is the command's own.
    Disabled(String),
    /// Unknown command id.
    UnknownCommand,
    /// Parameter validation failed.
    BadParams(String),
    /// The command failed while running (document restored).
    Failed(String),
}

/// What the client gets back.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Receipt {
    pub request_id: String,
    pub accepted: bool,
    /// True when this receipt answers a duplicate delivery (no second mutation happened).
    #[serde(default)]
    pub duplicate: bool,
    /// The command's result on success.
    #[serde(default)]
    pub applied: Value,
    #[serde(default)]
    pub rejection: Option<Rejection>,
    /// Engine revision after this envelope (unchanged when rejected).
    pub revision: u64,
}

/// Remembered receipts for deduplication.
#[derive(Debug, Default)]
pub struct Dedup {
    recent: VecDeque<(String, Receipt)>,
}

impl Dedup {
    pub(crate) fn find(&self, id: &str) -> Option<&Receipt> {
        self.recent.iter().rev().find(|(k, _)| k == id).map(|(_, r)| r)
    }

    pub(crate) fn remember(&mut self, r: &Receipt) {
        if r.request_id.is_empty() {
            return;
        }
        self.recent.push_back((r.request_id.clone(), r.clone()));
        while self.recent.len() > DEDUP_WINDOW {
            self.recent.pop_front();
        }
    }
}

impl Engine {
    /// Validates and runs one envelope (see the module docs). Never panics; never leaves a
    /// half-applied edit (`execute` restores the document on failure).
    pub fn submit(&mut self, env: &Envelope) -> Receipt {
        if !env.request_id.is_empty()
            && let Some(prev) = self.dedup.find(&env.request_id)
        {
            let mut r = prev.clone();
            r.duplicate = true;
            r.revision = self.revision;
            return r;
        }
        let reject = |rejection: Rejection, revision: u64| Receipt {
            request_id: env.request_id.clone(),
            accepted: false,
            duplicate: false,
            applied: Value::Null,
            rejection: Some(rejection),
            revision,
        };
        if let Some(expected) = env.expected_revision
            && expected != self.revision
        {
            let r = reject(Rejection::StaleRevision { expected, current: self.revision }, self.revision);
            self.dedup.remember(&r);
            return r;
        }
        let result = match &env.gesture {
            Some(key) => self.execute_merged(&env.command, &env.params, key),
            None => {
                self.end_merge();
                self.execute(&env.command, &env.params)
            }
        };
        let receipt = match result {
            Ok(v) => {
                Receipt { request_id: env.request_id.clone(), accepted: true, duplicate: false, applied: v, rejection: None, revision: self.revision }
            }
            Err(e) => {
                let rejection = match &e {
                    EngineError::UnknownCommand(_) => Rejection::UnknownCommand,
                    EngineError::Disabled(_, why) => Rejection::Disabled(why.clone()),
                    EngineError::BadParams(_, why) => Rejection::BadParams(why.clone()),
                    other => Rejection::Failed(other.to_string()),
                };
                reject(rejection, self.revision)
            }
        };
        self.dedup.remember(&receipt);
        receipt
    }
}

/// Risk category of a command, for clients that must confirm or refuse (remote allowlists, the
/// assistant's interaction policy). Derived from the command id; a command not listed here is
/// `Reversible` when undoable and `View` otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    /// Changes only what is shown (zoom, window, selection).
    View,
    /// Transport: play/stop/locate/record (record creates media; nothing is destroyed).
    Transport,
    /// An undoable session edit.
    Reversible,
    /// Writes or overwrites files, closes a session, or cannot be undone: needs its own
    /// authorization from a remote or assistant client.
    Destructive,
}

/// Implementation status as the registry can state it for this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Implemented,
    /// Registered for id stability but disabled with a reason (B04 modes).
    DisabledNotImplemented,
}

/// Knowledge fields for one command (the grounding corpus for help, remote and assistant).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Knowledge {
    pub id: &'static str,
    pub label: &'static str,
    pub menu: Vec<&'static str>,
    pub shortcut: Option<&'static str>,
    /// Parameter schema in the registry's documentation form.
    pub params: &'static str,
    pub undoable: bool,
    pub risk: Risk,
    pub status: Status,
    /// Why the command is currently disabled in the given engine state, if it is.
    pub disabled_reason: Option<String>,
}

/// Risk of a command id (prefix rules, then undoability).
pub fn risk_of(id: &str, undoable: bool) -> Risk {
    const DESTRUCTIVE: &[&str] = &[
        "session.save",
        "session.save_as",
        "session.close",
        "session.new",
        "session.open",
        "file.export",
        "file.bounce",
        "file.consolidate",
        "file.clear_unused",
        "file.delete",
    ];
    if DESTRUCTIVE.iter().any(|p| id.starts_with(p)) {
        return Risk::Destructive;
    }
    if id.starts_with("transport.") {
        return Risk::Transport;
    }
    if undoable {
        return Risk::Reversible;
    }
    if id.starts_with("view.") || id.starts_with("window.") || id.starts_with("edit.select") || id.starts_with("edit.zoom") {
        return Risk::View;
    }
    Risk::Reversible
}

/// Knowledge for every registered command in the engine's current state.
pub fn knowledge(e: &Engine) -> Vec<Knowledge> {
    crate::command_specs()
        .iter()
        .map(|c| {
            let disabled_reason = (c.enabled)(e).err();
            let status = if c.params.contains("DISABLED: not implemented") { Status::DisabledNotImplemented } else { Status::Implemented };
            Knowledge {
                id: c.id,
                label: c.label,
                menu: c.menu.to_vec(),
                shortcut: c.shortcut,
                params: c.params,
                undoable: c.undoable,
                risk: risk_of(c.id, c.undoable),
                status,
                disabled_reason,
            }
        })
        .collect()
}

/// The knowledge corpus as JSON (one object per command), with the build version.
pub fn knowledge_json(e: &Engine) -> Value {
    json!({ "build": env!("CARGO_PKG_VERSION"), "commands": knowledge(e) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(id: &str, command: &str, params: Value, expected: Option<u64>) -> Envelope {
        Envelope { request_id: id.into(), client: Client::Remote, command: command.into(), params, expected_revision: expected, gesture: None }
    }

    /// R09: a duplicate delivery answers with the first receipt and mutates nothing; a stale
    /// revision is rejected without mutation; a deleted target fails without touching the
    /// document; results carry the resulting revision.
    #[test]
    fn envelopes_deduplicate_reject_stale_and_never_half_apply() {
        let mut e = crate::demo::demo_engine();
        let rev0 = e.revision;
        let r1 = e.submit(&env("req-1", "mix.volume", json!({"track": "Kick", "db": -5.0}), Some(rev0)));
        assert!(r1.accepted && !r1.duplicate, "{r1:?}");
        assert!(r1.revision > rev0);
        let kick = |e: &Engine| e.session().track_by_name("Kick").map(|t| t.mixer.volume_db);
        assert_eq!(kick(&e), Some(-5.0));
        // Retry with the same id and an older expected revision: same receipt, no change.
        let undo_len = e.undo_depth();
        let r1b = e.submit(&env("req-1", "mix.volume", json!({"track": "Kick", "db": -20.0}), Some(rev0)));
        assert!(r1b.accepted && r1b.duplicate);
        assert_eq!(r1b.applied, r1.applied);
        assert_eq!(kick(&e), Some(-5.0), "duplicate did not mutate");
        assert_eq!(e.undo_depth(), undo_len, "no second undo step");
        // Stale revision: rejected visibly, nothing changes.
        let r2 = e.submit(&env("req-2", "mix.volume", json!({"track": "Kick", "db": -20.0}), Some(rev0)));
        assert!(!r2.accepted);
        assert!(matches!(r2.rejection, Some(Rejection::StaleRevision { expected, .. }) if expected == rev0), "{r2:?}");
        assert_eq!(kick(&e), Some(-5.0));
        // Deleted target: the command's own validation fails; document untouched.
        let before = e.session_arc();
        let r3 = e.submit(&env("req-3", "mix.volume", json!({"track": "No Such Track", "db": -1.0}), None));
        assert!(!r3.accepted && matches!(r3.rejection, Some(Rejection::BadParams(_)) | Some(Rejection::Failed(_))), "{r3:?}");
        assert!(std::sync::Arc::ptr_eq(&before, &e.session_arc()));
        // Unknown command and a disabled (not implemented) mode.
        assert_eq!(e.submit(&env("req-4", "no.such.command", json!({}), None)).rejection, Some(Rejection::UnknownCommand));
        let r5 = e.submit(&env("req-5", "options.quickpunch", json!({}), None));
        assert!(matches!(r5.rejection, Some(Rejection::Disabled(ref why)) if why.contains("not implemented")), "{r5:?}");
        // Gesture: three fader values, one undo step, final value kept.
        let depth = e.undo_depth();
        for v in [-6.0, -7.0, -8.0] {
            let mut g = env(&format!("g-{v}"), "mix.volume", json!({"track": "Kick", "db": v}), None);
            g.gesture = Some("fader:kick".into());
            assert!(e.submit(&g).accepted);
        }
        assert_eq!(kick(&e), Some(-8.0));
        assert_eq!(e.undo_depth(), depth + 1, "one undo step for the whole gesture");
    }

    #[test]
    fn knowledge_covers_every_command_with_risk_and_status() {
        let e = crate::demo::demo_engine();
        let k = knowledge(&e);
        assert_eq!(k.len(), crate::command_specs().len());
        let by_id = |id: &str| k.iter().find(|x| x.id == id).cloned();
        assert_eq!(by_id("options.quickpunch").map(|x| x.status), Some(Status::DisabledNotImplemented));
        assert_eq!(by_id("transport.play").map(|x| x.risk), Some(Risk::Transport));
        assert_eq!(by_id("session.save").map(|x| x.risk), Some(Risk::Destructive));
        assert_eq!(by_id("mix.volume").map(|x| x.risk), Some(Risk::Reversible));
        let j = knowledge_json(&e);
        assert!(j["commands"].as_array().is_some_and(|a| a.len() == k.len()));
        assert!(j["build"].as_str().is_some_and(|b| !b.is_empty()));
    }
}
