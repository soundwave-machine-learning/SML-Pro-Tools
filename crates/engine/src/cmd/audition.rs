//! Listen tool (EDIT_TOOLBAR_SPEC T09): a temporary audition transaction. Allowed only while
//! the transport is stopped and not recording; snapshots the cursor; uses a route eligibility
//! mask (`Session.listen`, realtime only, never saved) instead of touching mute/solo/arm flags;
//! writes no automation, records nothing and leaves no undo entry. Mouse-up, Escape, focus loss,
//! track deletion or device failure call `audition.stop`; a transport start or record terminates
//! it first (see `Engine::execute`). The cursor is restored only while the transaction still owns
//! it: any other successful command in between clears that ownership.

use super::*;
use crate::cmd;
use crate::{Engine, TransportRequest};
use serde_json::json;
use soundcraft_model::{ListenMask, ListenState, TrackKind};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "audition.start", "Listen (audition)", [], None, "{tracks?: [id|name] (default: the selected tracks), at?: position (default: the cursor)} — press-and-hold audition; refused while the transport runs", always, start),
        cmd!(query "audition.stop", "Stop Listen", [], None, "{}", always, |e, _| {
            let was = e.session().listen.active();
            terminate(e);
            Ok(json!({"was_active": was}))
        }),
        cmd!(query "audition.status", "Listen Status", [], None, "{}", always, |e, _| {
            let l = e.session().listen;
            Ok(json!({"active": l.active(), "tracks": l.mask.count(), "from": l.from, "cursor": l.cursor, "cursor_owned": l.cursor_owned, "reason": unavailable(e)}))
        }),
    ]
}

/// Why Listen cannot start right now, if it cannot.
pub fn unavailable(e: &Engine) -> Option<&'static str> {
    if e.transport.recording {
        Some("Listen is unavailable while recording (a take is never stopped to audition)")
    } else if e.transport.playing && !e.session().listen.active() {
        Some("Listen is unavailable while the transport runs")
    } else {
        None
    }
}

fn start(e: &mut Engine, p: &Value) -> Result<Value> {
    let cmd = "audition.start";
    if let Some(why) = unavailable(e) {
        return Err(bad(cmd, why));
    }
    if e.session().listen.active() {
        terminate(e);
    }
    let tracks = tracks_required(e, cmd, p)?;
    let at = position_param(e, cmd, p, "at")?.unwrap_or(e.transport.position).max(0);
    let mut mask = ListenMask::default();
    let mut names = Vec::new();
    let mut muted = Vec::new();
    {
        let s = e.session();
        for t in &tracks {
            let Some(i) = s.track_index(*t) else { return Err(bad(cmd, format!("track {} not found", t.0))) };
            let tr = &s.tracks[i];
            if !matches!(tr.kind, TrackKind::Audio | TrackKind::Instrument | TrackKind::Midi) {
                return Err(bad(cmd, format!("`{}` is not a source track", tr.name)));
            }
            if !mask.set(i) {
                return Err(bad(cmd, format!("track index {i} is beyond the Listen mask capacity ({})", ListenMask::CAPACITY)));
            }
            names.push(tr.name.clone());
            if tr.mixer.mute {
                muted.push(tr.name.clone());
            }
        }
    }
    if mask.count() == 0 {
        return Err(bad(cmd, "no source track to audition"));
    }
    mask.active = true;
    let cursor = e.transport.position;
    let s = e.session_mut_transient();
    s.listen = ListenState { mask, cursor, cursor_owned: true, from: at };
    e.transport_requests.push(TransportRequest::Locate(at));
    e.transport_requests.push(TransportRequest::Play);
    Ok(json!({
        "tracks": names,
        "from": at,
        // Explicit mutes still win (T09): say why a source stays inaudible.
        "inaudible_muted": muted,
    }))
}

/// End the audition: stop the transport, clear the mask, restore the cursor while still owned.
pub fn terminate(e: &mut Engine) {
    if !e.session().listen.active() {
        return;
    }
    let l = e.session().listen;
    let s = e.session_mut_transient();
    s.listen = ListenState::default();
    e.transport_requests.push(TransportRequest::Stop);
    if l.cursor_owned {
        e.transport_requests.push(TransportRequest::Locate(l.cursor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_model::{ChannelFormat, Session, TrackId};

    fn engine() -> (Engine, TrackId, TrackId) {
        let mut s = Session::default();
        let a = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Lead"));
        let b = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Dbl"));
        s.add_track(TrackKind::Aux, ChannelFormat::Stereo, Some("Vox Bus"));
        s.edit.selected_tracks = vec![a];
        (Engine::new(s), a, b)
    }

    /// T-T4: cleanup after every exit condition; record attempts; explicit flags untouched;
    /// the mask covers exactly the audited sources; serialization never carries the state.
    #[test]
    fn listen_transaction_masks_restores_and_never_persists() {
        let (mut e, a, b) = engine();
        e.transport.position = 5_000;
        let snap = e.session_arc();
        let r = e.execute("audition.start", &json!({"at": 12_000})).unwrap();
        assert_eq!(r["tracks"], json!(["Lead"]));
        let l = e.session().listen;
        assert!(l.active() && l.mask.contains(0) && !l.mask.contains(1) && !l.mask.contains(2));
        assert_eq!((l.cursor, l.from), (5_000, 12_000));
        assert!(matches!(e.transport_requests.as_slice(), [TransportRequest::Locate(12_000), TransportRequest::Play]));
        e.transport_requests.clear();
        e.transport.playing = true;
        // No undo entry, no flag change, nothing in the saved form.
        assert_eq!(e.undo_depth(), 0);
        let t = e.session().track(a).unwrap();
        assert!(!t.mixer.solo && !t.mixer.mute && !t.mixer.record_arm);
        let json = serde_json::to_string(e.session()).unwrap();
        let back: Session = serde_json::from_str(&json).unwrap();
        assert!(!back.listen.active(), "audition state is never serialized");
        assert_eq!(serde_json::to_string(&*snap).unwrap(), json, "the document is byte-identical to before");
        assert!(!e.is_dirty(), "an audition never dirties the document");
        // Stop while owned: transport stops and the cursor goes back.
        e.execute("audition.stop", &json!({})).unwrap();
        assert!(!e.session().listen.active());
        assert!(matches!(e.transport_requests.as_slice(), [TransportRequest::Stop, TransportRequest::Locate(5_000)]));
        e.transport_requests.clear();
        e.transport.playing = false;
        // Another command during the audition takes the cursor: no restore on stop.
        e.execute("audition.start", &json!({"tracks": [b.0], "at": 100})).unwrap();
        e.transport_requests.clear();
        e.transport.playing = true;
        e.execute("transport.locate", &json!({"at": 777})).unwrap();
        assert!(!e.session().listen.cursor_owned);
        e.transport_requests.clear();
        e.execute("audition.stop", &json!({})).unwrap();
        assert!(matches!(e.transport_requests.as_slice(), [TransportRequest::Stop]), "{:?}", e.transport_requests);
        e.transport_requests.clear();
        e.transport.playing = false;
        // Record / play terminate the audition first, then run normally.
        e.execute("audition.start", &json!({"at": 100})).unwrap();
        e.transport_requests.clear();
        e.transport.playing = true;
        e.execute("transport.record", &json!({})).unwrap();
        assert!(!e.session().listen.active());
        assert!(matches!(e.transport_requests.as_slice(), [TransportRequest::Stop, TransportRequest::Locate(5_000), TransportRequest::Record]));
        e.transport_requests.clear();
        // Refused while playing or recording; a non-source track is refused.
        e.transport.playing = true;
        assert!(e.execute("audition.start", &json!({})).is_err());
        e.transport.playing = false;
        e.transport.recording = true;
        assert!(e.execute("audition.start", &json!({})).is_err());
        e.transport.recording = false;
        assert!(e.execute("audition.start", &json!({"tracks": ["Vox Bus"]})).is_err());
        assert!(!e.session().listen.active());
        assert_eq!(e.undo_depth(), 0);
    }
}
