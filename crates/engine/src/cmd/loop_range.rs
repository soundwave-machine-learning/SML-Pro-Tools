//! Loop locators (HF05): a stored loop range, separate from the edit selection.
//!
//! When a range is stored, Loop Playback loops it; without one it loops the edit selection, as it
//! always did. Loop Record is unchanged: it records over the edit selection (`loop.select` copies
//! the loop range into the selection when you want to record the same span). Selecting never
//! moves the locators; only these commands do. Locator edits are refused while recording.

use super::*;
use crate::cmd;
use serde_json::json;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "loop.set",
            "Set Loop Range",
            [],
            None,
            "{start: position, end: position} (or start + length) — stores the loop locators; Loop Playback then loops this range; refused while recording",
            always,
            set
        ),
        cmd!("loop.to_selection", "Set Loop to Selection", [], None, "{} — copies the edit selection into the loop locators", always, to_selection),
        cmd!(
            "loop.clear",
            "Clear Loop Range",
            [],
            None,
            "{} — removes the stored locators (Loop Playback then loops the edit selection)",
            always,
            clear
        ),
        cmd!(noundo "loop.select", "Select Loop Range", [], None, "{} — sets the edit selection to the loop range (for Loop Record)", always, select),
        cmd!(query "loop.info", "Loop Settings", [], None, "{} → {enabled, range: {start, end, length} | null, source: stored|selection|none, loop_record}", always, info),
    ]
}

fn guard(e: &Engine, cmd: &str) -> Result<()> {
    if e.transport.recording {
        return Err(bad(cmd, "stop recording to change the loop range (a take is never interrupted to move the locators)"));
    }
    Ok(())
}

fn store(e: &mut Engine, cmd: &str, start: Samples, end: Samples) -> Result<Value> {
    // Checked before building the Range (whose constructor would swap an inverted pair).
    if start < 0 || end <= start {
        return Err(bad(cmd, format!("the loop range must start at 0 or later and end after it (got {start}..{end})")));
    }
    let r = Range::new(start, end);
    e.session_mut().edit.loop_range = Some(r);
    Ok(json!({"start": r.start, "end": r.end, "length": r.len()}))
}

fn set(e: &mut Engine, p: &Value) -> Result<Value> {
    guard(e, "loop.set")?;
    let start = position_param(e, "loop.set", p, "start")?.ok_or_else(|| bad("loop.set", "`start` required"))?;
    let end = match position_param(e, "loop.set", p, "end")? {
        Some(x) => x,
        None => start.saturating_add(position_param(e, "loop.set", p, "length")?.ok_or_else(|| bad("loop.set", "`end` or `length` required"))?),
    };
    store(e, "loop.set", start, end)
}

fn to_selection(e: &mut Engine, _p: &Value) -> Result<Value> {
    guard(e, "loop.to_selection")?;
    let sel = e.session().edit.selection;
    if sel.is_empty() {
        return Err(bad("loop.to_selection", "make a time selection first"));
    }
    store(e, "loop.to_selection", sel.start, sel.end)
}

fn clear(e: &mut Engine, _p: &Value) -> Result<Value> {
    guard(e, "loop.clear")?;
    e.session_mut().edit.loop_range = None;
    Ok(json!({"cleared": true}))
}

fn select(e: &mut Engine, _p: &Value) -> Result<Value> {
    let r = e.session().edit.loop_range.ok_or_else(|| bad("loop.select", "no loop range is stored"))?;
    e.session_mut().edit.selection = r;
    Ok(json!({"start": r.start, "end": r.end}))
}

/// The range Loop Playback uses: the stored locators when valid, else the edit selection.
pub fn effective_range(s: &soundcraft_model::Session) -> Option<(Range, &'static str)> {
    match s.edit.loop_range.filter(|r| r.start >= 0 && r.end > r.start) {
        Some(r) => Some((r, "stored")),
        None => (!s.edit.selection.is_empty()).then_some((s.edit.selection, "selection")),
    }
}

fn info(e: &mut Engine, _p: &Value) -> Result<Value> {
    let s = e.session();
    let (range, source) = match effective_range(s) {
        Some((r, src)) => (json!({"start": r.start, "end": r.end, "length": r.len()}), src),
        None => (Value::Null, "none"),
    };
    Ok(json!({"enabled": s.edit.loop_playback, "range": range, "source": source, "loop_record": s.edit.loop_record}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locators_are_separate_from_the_selection_and_validated() {
        let mut e = Engine::default();
        e.execute("edit.select", &json!({"start": 1000, "end": 5000})).unwrap();
        assert_eq!(e.execute("loop.info", &json!({})).unwrap()["source"], "selection", "no locators: the selection loops, as before");
        e.execute("loop.set", &json!({"start": 48_000, "end": 96_000})).unwrap();
        let info = e.execute("loop.info", &json!({})).unwrap();
        assert_eq!(info["source"], "stored");
        assert_eq!(info["range"]["length"], 48_000);
        // Selecting does not move the locators.
        e.execute("edit.select", &json!({"start": 10, "end": 20})).unwrap();
        assert_eq!(e.session().edit.loop_range, Some(Range::new(48_000, 96_000)));
        // Invalid ranges are refused and change nothing.
        for bad in [json!({"start": 500, "end": 500}), json!({"start": 900, "end": 100}), json!({"start": -10, "end": 100}), json!({"end": 100})] {
            assert!(e.execute("loop.set", &bad).is_err(), "{bad}");
        }
        assert_eq!(e.session().edit.loop_range, Some(Range::new(48_000, 96_000)));
        // start + length; to_selection; select; clear; undo.
        e.execute("loop.set", &json!({"start": 100, "length": 900})).unwrap();
        assert_eq!(e.session().edit.loop_range, Some(Range::new(100, 1000)));
        e.execute("loop.to_selection", &json!({})).unwrap();
        assert_eq!(e.session().edit.loop_range, Some(Range::new(10, 20)));
        e.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(e.session().edit.loop_range, Some(Range::new(100, 1000)), "locator edits are undoable");
        e.execute("loop.select", &json!({})).unwrap();
        assert_eq!(e.session().edit.selection, Range::new(100, 1000));
        e.execute("loop.clear", &json!({})).unwrap();
        assert_eq!(e.session().edit.loop_range, None);
        // Recording: locators are frozen.
        e.execute("loop.set", &json!({"start": 0, "end": 4800})).unwrap();
        e.transport.recording = true;
        assert!(e.execute("loop.set", &json!({"start": 0, "end": 9600})).unwrap_err().to_string().contains("stop recording"));
        assert!(e.execute("loop.clear", &json!({})).is_err());
        assert_eq!(e.session().edit.loop_range, Some(Range::new(0, 4800)));
    }

    #[test]
    fn locators_survive_save_and_reopen_and_old_files_have_none() {
        let mut e = Engine::default();
        e.execute("loop.set", &json!({"start": 2400, "end": 7200})).unwrap();
        let json = e.session().to_json().unwrap();
        let back = soundcraft_model::Session::from_json(&json).unwrap();
        assert_eq!(back.edit.loop_range, Some(Range::new(2400, 7200)));
        // A session file written before loop locators existed loads without them.
        let mut v: Value = serde_json::from_str(&json).unwrap();
        v["edit"].as_object_mut().unwrap().remove("loop_range");
        let old = soundcraft_model::Session::from_json(&v.to_string()).unwrap();
        assert_eq!(old.edit.loop_range, None);
    }
}
