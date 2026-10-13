//! The SoundCraft command engine.
//!
//! Every user-visible action is a registered command ([`cmd::CommandSpec`]): menus, shortcuts, the
//! CLI, the JSON control channel and MCP all dispatch the same commands by id with JSON params.
//! The engine owns the [`Session`] behind an `Arc` (copy-on-write), so undo snapshots are cheap.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod assistant;
pub mod assistant_runtime;
pub mod autosave;
pub mod catalog;
pub mod catalog_more;
pub mod clip_group_file;
pub mod cmd;
pub mod demo;
pub mod edit;
pub mod envelope;
pub mod inspect;
pub mod io;
pub mod lossy;
pub mod score;
pub mod warp;

use serde_json::{Value, json};
use soundcraft_model::{Clip, Session, TrackId};
use soundcraft_time::Samples;
use std::sync::Arc;

pub use cmd::{CommandInfo, CommandSpec, command_specs, find_command, nudge_step_samples};
pub use soundcraft_model as model;

/// Shuts the third-party plugin hosts down, running the module exit of every plugin binary they
/// loaded. Call it at the end of `main`, once every plugin instance (mixer, player) is gone: a
/// plugin that has been used can crash when its module exit only runs from an exit handler.
pub fn shutdown_plugin_hosts() {
    soundcraft_vst3_host::shutdown();
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EngineError {
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    #[error("`{0}` is not available: {1}")]
    Disabled(String, String),
    #[error("bad parameters for `{0}`: {1}")]
    BadParams(String, String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Io(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("internal error in `{0}`: {1}")]
    Internal(String, String),
    /// HF18: refused because a take is being captured (the take is never cut short to allow it).
    #[error("`{0}` is refused while recording: {1}")]
    Recording(String, String),
}

/// HF18: commands that replace or discard the open session. They are refused by the engine
/// itself (every caller: menus, shortcuts, control channel, MCP, assistant, CLI scripts) while a
/// take is being captured, including its count-off and pre-roll.
pub const SESSION_REPLACING_COMMANDS: &[&str] = &["session.new", "session.open", "session.close", "session.revert"];

/// The message for a refused session replacement or recovery while recording.
pub const RECORDING_GUARD_MESSAGE: &str = "a take is being recorded. Stop the transport so the take is finished and placed, save the session if you want to keep it, then try again. Nothing was changed and the take continues.";

pub type Result<T> = std::result::Result<T, EngineError>;

/// Requests for the transport/audio engine, drained by the host app each frame.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum TransportRequest {
    /// Stop the old session's transport without applying stop-time edits to the new document.
    ResetSession,
    Play,
    Stop,
    TogglePlay,
    Record,
    Locate(Samples),
    PlaySelection,
    Pause,
    HalfSpeed,
    Scrub(Samples),
    AllNotesOff,
}

/// Snapshot of the transport reported back by the host (playback position etc.).
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize)]
pub struct TransportStatus {
    pub playing: bool,
    pub recording: bool,
    pub position: Samples,
}

/// Clipboard contents: per-track clip copies relative to the copied range start.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Clipboard {
    pub length: Samples,
    pub tracks: Vec<Vec<Clip>>,
    pub automation: Vec<Vec<soundcraft_model::AutomationLane>>,
    /// Copy Special › Clip Gain: per track, (offset from the copied range start, dB) breakpoints.
    pub clip_gain: Vec<Vec<(Samples, f32)>>,
    /// Copy Special › Clip Effects: clip-effect settings (`param` → value) of the first copied clip.
    pub clip_effects: Vec<(String, f64)>,
    /// Markers inside the copied range (starts relative to it), for Paste Special › Merge Markers.
    pub markers: Vec<soundcraft_model::MemoryLocation>,
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    label: String,
    doc: Arc<Session>,
}

/// Engine state: the document, history, clipboard and transport plumbing.
pub struct Engine {
    doc: Arc<Session>,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    pub history_limit: usize,
    pub clipboard: Clipboard,
    pub transport_requests: Vec<TransportRequest>,
    pub transport: TransportStatus,
    /// Command journal (id, params) for macro recording / agents.
    pub journal: Vec<(String, Value)>,
    /// Path of the session file, if saved.
    pub path: Option<String>,
    /// Revision counter; bumps on every document change (UI caches key on it).
    pub revision: u64,
    dirty: bool,
    /// Gesture key for undo coalescing (see `execute_merged`).
    merge_key: Option<String>,
    /// Recent envelope receipts for remote retry deduplication (R09).
    dedup: envelope::Dedup,
    /// Messages for the UI status line / agents.
    pub messages: Vec<String>,
    /// Bend render service (T08): off-thread pitch-preserving renders, polled by the host.
    pub warp: warp::Renderer,
    /// Assistant speech and model settings (RC04/RC05): enrolled phrases, voice, speech waiting
    /// for the host's monitor-only output, the local model endpoint.
    pub voice: assistant_runtime::VoiceState,
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new(Session::default())
    }
}

impl Engine {
    pub fn new(session: Session) -> Self {
        install_panic_hook();
        Engine {
            doc: Arc::new(session),
            undo: Vec::new(),
            redo: Vec::new(),
            history_limit: 200,
            clipboard: Clipboard::default(),
            transport_requests: Vec::new(),
            transport: TransportStatus::default(),
            journal: Vec::new(),
            path: None,
            revision: 1,
            dirty: false,
            merge_key: None,
            dedup: envelope::Dedup::default(),
            warp: warp::Renderer::default(),
            messages: Vec::new(),
            voice: assistant_runtime::VoiceState::default(),
        }
    }

    pub fn session(&self) -> &Session {
        &self.doc
    }

    pub fn session_arc(&self) -> Arc<Session> {
        Arc::clone(&self.doc)
    }

    /// Mutable access (copy-on-write). Callers outside commands should prefer `execute`.
    pub fn session_mut(&mut self) -> &mut Session {
        self.revision = self.revision.wrapping_add(1);
        self.dirty = true;
        Arc::make_mut(&mut self.doc)
    }

    /// Mutate realtime-only state (audition masks): the revision bumps so hosts resync, but the
    /// document is not marked dirty — nothing serializable changed.
    pub fn session_mut_transient(&mut self) -> &mut Session {
        self.revision = self.revision.wrapping_add(1);
        Arc::make_mut(&mut self.doc)
    }

    /// Replace the whole document (open / new); clears history.
    pub fn replace_session(&mut self, s: Session) {
        self.transport = TransportStatus { position: s.edit.selection.start.max(0), ..TransportStatus::default() };
        self.transport_requests.clear();
        self.transport_requests.push(TransportRequest::ResetSession);
        self.doc = Arc::new(s);
        self.undo.clear();
        self.redo.clear();
        self.revision = self.revision.wrapping_add(1);
        self.dirty = false;
        // RC11: renders of the previous session must never land in this one.
        self.warp.forget_all();
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    /// Marks the document modified (a recovered session has unsaved state by definition).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|h| h.label.as_str())
    }
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|h| h.label.as_str())
    }
    pub fn undo_history(&self) -> Vec<String> {
        self.undo.iter().map(|h| h.label.clone()).collect()
    }

    pub fn undo(&mut self) -> bool {
        let Some(h) = self.undo.pop() else { return false };
        let cur = std::mem::replace(&mut self.doc, h.doc);
        self.redo.push(HistoryEntry { label: h.label, doc: cur });
        self.revision = self.revision.wrapping_add(1);
        self.dirty = true;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(h) = self.redo.pop() else { return false };
        let cur = std::mem::replace(&mut self.doc, h.doc);
        self.undo.push(HistoryEntry { label: h.label, doc: cur });
        self.revision = self.revision.wrapping_add(1);
        self.dirty = true;
        true
    }

    /// Number of undo steps available.
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Puts a previously taken document back (plan rollback); bumps the revision.
    pub(crate) fn restore_document(&mut self, doc: Arc<Session>) {
        if !Arc::ptr_eq(&doc, &self.doc) {
            self.doc = doc;
            self.revision = self.revision.wrapping_add(1);
            self.dirty = true;
        }
    }

    pub(crate) fn dedup_lookup(&self, id: &str) -> Option<envelope::Receipt> {
        self.dedup.find(id).cloned()
    }

    pub(crate) fn dedup_remember(&mut self, id: &str, accepted: bool) {
        if id.is_empty() {
            return;
        }
        self.dedup.remember(&envelope::Receipt {
            request_id: id.into(),
            accepted,
            duplicate: false,
            applied: Value::Null,
            rejection: None,
            revision: self.revision,
        });
    }

    /// Every registered command id (hosts attach the active shortcuts to an assistant request).
    pub fn command_ids(&self) -> Vec<String> {
        command_specs().iter().map(|c| c.id.to_string()).collect()
    }

    /// Record an undo point manually (for interactive gestures committed by the UI).
    pub fn push_undo(&mut self, label: &str, before: Arc<Session>) {
        if Arc::ptr_eq(&before, &self.doc) {
            return;
        }
        self.undo.push(HistoryEntry { label: label.to_string(), doc: before });
        self.redo.clear();
        if self.undo.len() > self.history_limit {
            let excess = self.undo.len() - self.history_limit;
            self.undo.drain(..excess);
        }
    }

    /// Run a command by id. Never panics: escaped panics become `EngineError::Internal` and the
    /// document is restored.
    /// HF18: why the open session may not be replaced or recovered over right now, if it may not.
    pub fn session_replace_blocked(&self) -> Option<&'static str> {
        self.transport.recording.then_some(RECORDING_GUARD_MESSAGE)
    }

    /// HF18: `Err` when the session may not be replaced now (`what` names the request).
    pub fn check_session_replace(&self, what: &str) -> Result<()> {
        match self.session_replace_blocked() {
            Some(why) => Err(EngineError::Recording(what.to_string(), why.to_string())),
            None => Ok(()),
        }
    }

    pub fn execute(&mut self, id: &str, params: &Value) -> Result<Value> {
        self.merge_key = None;
        let spec = find_command(id).ok_or_else(|| EngineError::UnknownCommand(id.to_string()))?;
        if SESSION_REPLACING_COMMANDS.contains(&id) {
            // Checked before anything runs: a refusal changes nothing.
            self.check_session_replace(id)?;
        }
        if let Err(reason) = (spec.enabled)(self) {
            // A call that names its targets (tracks, clips, a range) does not need a selection;
            // the command itself validates them.
            let names_targets = ["track", "tracks", "clip", "clips", "start", "end", "at"].iter().any(|k| params.get(k).is_some());
            if !names_targets {
                return Err(EngineError::Disabled(id.to_string(), reason));
            }
        }
        // T09: a transport start or record first terminates a Listen audition (restoring the
        // cursor while still owned), then runs normally.
        if self.doc.listen.active() && matches!(id, "transport.play" | "transport.record" | "transport.toggle" | "transport.play_selection") {
            cmd::audition::terminate(self);
        }
        let before = Arc::clone(&self.doc);
        let rev = self.revision;
        let dirty = self.dirty;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (spec.run)(self, params)));
        let result = match result {
            Ok(r) => r,
            Err(p) => {
                let msg =
                    p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "panic".into());
                self.doc = before;
                self.revision = rev.wrapping_add(1);
                self.dirty = dirty;
                return Err(EngineError::Internal(id.to_string(), msg));
            }
        };
        match result {
            Ok(v) => {
                // Later user commands win over the audition's cursor restore (T09).
                if self.doc.listen.active() && self.doc.listen.cursor_owned && !id.starts_with("audition.") && !Arc::ptr_eq(&before, &self.doc) {
                    Arc::make_mut(&mut self.doc).listen.cursor_owned = false;
                }
                if spec.undoable && !Arc::ptr_eq(&before, &self.doc) {
                    self.push_undo(spec.label, before);
                }
                if spec.journal {
                    self.journal.push((id.to_string(), params.clone()));
                }
                Ok(v)
            }
            Err(e) => {
                // Commands must not leave half-applied edits behind.
                if !Arc::ptr_eq(&before, &self.doc) {
                    self.doc = before;
                    self.revision = self.revision.wrapping_add(1);
                    self.dirty = dirty;
                }
                Err(e)
            }
        }
    }

    /// Execute as part of a continuous gesture (fader drag, knob turn, slider): consecutive calls
    /// with the same `key` collapse into a single undo step.
    pub fn execute_merged(&mut self, id: &str, params: &Value, key: &str) -> Result<Value> {
        let continuing = self.merge_key.as_deref() == Some(key);
        let before_len = self.undo.len();
        let r = self.execute(id, params)?;
        if continuing && self.undo.len() == before_len + 1 && before_len > 0 {
            // Drop the step just pushed; the earlier one still holds the pre-gesture document.
            self.undo.pop();
        }
        self.merge_key = Some(key.to_string());
        Ok(r)
    }

    /// End a gesture started with `execute_merged`.
    pub fn end_merge(&mut self) {
        self.merge_key = None;
    }

    /// Convenience: execute with `{}` params.
    pub fn run(&mut self, id: &str) -> Result<Value> {
        self.execute(id, &json!({}))
    }

    pub fn message(&mut self, m: impl Into<String>) {
        let m = m.into();
        log::info!("{m}");
        self.messages.push(m);
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }

    /// Tracks targeted by an edit: the selected tracks, or all tracks with playlists if none.
    pub fn target_tracks(&self) -> Vec<TrackId> {
        let s = self.session();
        if s.edit.selected_tracks.is_empty() {
            Vec::new()
        } else {
            s.tracks.iter().filter(|t| s.edit.selected_tracks.contains(&t.id)).map(|t| t.id).collect()
        }
    }
}

fn install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            log::error!("panic: {info}");
            prev(info);
        }));
    });
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    #[test]
    fn merged_gesture_is_one_undo_step() {
        let mut e = Engine::default();
        let t = e.session_mut().add_track(soundcraft_model::TrackKind::Audio, soundcraft_model::ChannelFormat::Mono, None);
        for db in [-1.0, -2.0, -3.0, -4.0] {
            e.execute_merged("mix.volume", &json!({"track": t.0, "db": db}), "fader").unwrap();
        }
        e.end_merge();
        assert_eq!(e.undo_history().len(), 1);
        e.undo();
        assert_eq!(e.session().track(t).unwrap().mixer.volume_db, 0.0);
    }
}
