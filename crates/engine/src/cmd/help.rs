//! Command help / knowledge search (R22): a versioned, searchable view over the real command
//! registry. An answer names only commands this build has; nonexistent tools return nothing;
//! every entry carries its status (implemented or DISABLED: not implemented), risk, undo
//! behaviour and the build's known limits for that feature. Shortcuts shown by the UI come from
//! the active profile (added by the host), never from here.

use super::*;
use crate::cmd;
use crate::envelope::{self, Status};
use serde_json::json;

/// Known limits of this build per command prefix or id (stated, not hidden).
pub const LIMITS: &[(&str, &str)] = &[
    (
        "clip.warp_",
        "Bend: segments between anchors are butt-joined (no crossfade), ratio 0.25..4 per segment, stereo shares one map; listening review is a native gate",
    ),
    ("clip.remove_warp", "restores the original material recorded on the first warp"),
    ("audition.", "Listen: only while the transport is stopped and not recording; never saved; explicit mutes still win"),
    ("automation.draw", "automation lanes only; the clip-gain envelope has its own command (clip.gain_draw)"),
    ("clip.gain_draw", "clip-gain envelope in dB (−144..+36) of one clip"),
    ("nudge.", "keypad vs top-row keys are not distinguished by the UI toolkit (B03); physical keypad use is a native gate"),
    ("options.quickpunch", "DISABLED: not implemented (B04)"),
    ("options.trackpunch", "DISABLED: not implemented (B04)"),
    ("options.destructive", "DISABLED: not implemented (B04)"),
    ("transport.record", "takes are placed from backend capture timestamps; loop record and punch use the documented scheduler"),
    ("file.bounce", "no compressed export formats; monitor controls never reach a bounce"),
    ("mix.monitor_mode", "hardware monitoring means the interface mixes the input; the app cannot verify the interface's own mix"),
    (
        "plugin.",
        "hosted plugins run in-process by default (no crash containment); the opt-in `sandbox:<id>` mode runs a plugin out of process with one block of added latency and no native editor (E03); scanning is isolated",
    ),
    (
        "assistant.",
        "rule-based and grounded: no language model, speech recognition or synthesis is configured (E06); plans run only as validated commands at the current revision",
    ),
    (
        "mastering.",
        "meters follow BS.1770-4 / R128 / Tech 3342 and are checked against analytic test signals (E07), not certified; the reference is monitor-only",
    ),
    ("session.", "autosave keeps rolling validated generations; power-loss guarantees are unverified (E02)"),
];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "help.search", "Command Help", ["Help"], None, "{query: text, limit?: 20} — searches id, label, menu path and parameter docs of this build's commands; returns status, risk, undo and known limits", always, search),
        cmd!(query "help.limits", "Known Limits", ["Help"], None, "{} — the build's stated limits by feature", always, limits),
    ]
}

fn limits(_e: &mut Engine, _p: &Value) -> Result<Value> {
    let rows: Vec<Value> = LIMITS.iter().map(|(k, v)| json!({"applies_to": k, "limit": v})).collect();
    Ok(json!({"build": env!("CARGO_PKG_VERSION"), "limits": rows}))
}

/// Limits that apply to a command id.
pub fn limits_for(id: &str) -> Vec<&'static str> {
    LIMITS.iter().filter(|(k, _)| id.starts_with(k) || id == *k).map(|(_, v)| *v).collect()
}

/// Word tokens of a text (letters/digits runs, lower-case).
fn tokens(text: &str) -> Vec<String> {
    text.to_ascii_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|t| !t.is_empty()).map(str::to_string).collect()
}

/// A query word matches a token by prefix; words of three letters or fewer must match a whole
/// token (so "ara" never finds "separate" or "parabola").
fn word_hits(toks: &[String], word: &str) -> bool {
    if word.len() <= 3 { toks.iter().any(|t| t == word) } else { toks.iter().any(|t| t.starts_with(word)) }
}

fn score(k: &envelope::Knowledge, q: &str) -> i32 {
    let q = q.to_ascii_lowercase();
    if k.id.eq_ignore_ascii_case(&q) {
        return 1000;
    }
    let id = tokens(k.id);
    let label = tokens(k.label);
    let menu = tokens(&k.menu.join(" "));
    let params = tokens(k.params);
    let mut s = 0;
    for word in q.split_whitespace() {
        let hit = if word_hits(&id, word) {
            60
        } else if word_hits(&label, word) {
            50
        } else if word_hits(&menu, word) {
            30
        } else if word_hits(&params, word) {
            15
        } else {
            return 0;
        };
        s += hit;
    }
    s
}

fn search(e: &mut Engine, p: &Value) -> Result<Value> {
    let q = str_param(p, "query").unwrap_or("").trim().to_string();
    let limit = p.get("limit").and_then(Value::as_u64).unwrap_or(20).clamp(1, 200) as usize;
    let (total, results) = search_rows_total(e, &q, limit);
    Ok(json!({"build": env!("CARGO_PKG_VERSION"), "query": q, "total": total, "results": results}))
}

/// The best `limit` knowledge rows for `q` (the assistant's Explain grounding).
pub fn search_rows(e: &Engine, q: &str, limit: usize) -> Vec<Value> {
    search_rows_total(e, q, limit).1
}

fn search_rows_total(e: &Engine, q: &str, limit: usize) -> (usize, Vec<Value>) {
    let mut rows: Vec<(i32, Value)> = envelope::knowledge(e)
        .into_iter()
        .filter_map(|k| {
            let s = if q.is_empty() { 1 } else { score(&k, q) };
            (s > 0).then(|| {
                let status = match k.status {
                    Status::Implemented => "implemented",
                    Status::DisabledNotImplemented => "DISABLED: not implemented",
                };
                (s, json!({"id": k.id, "label": k.label, "menu": k.menu, "shortcut": k.shortcut, "params": k.params, "undoable": k.undoable, "risk": k.risk, "status": status, "disabled_reason": k.disabled_reason, "limits": limits_for(k.id)}))
            })
        })
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1["id"].as_str().cmp(&b.1["id"].as_str())));
    let total = rows.len();
    let results: Vec<Value> = rows.into_iter().take(limit).map(|(_, v)| v).collect();
    (total, results)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V-KNOWLEDGE (portable): a versioned question set — real tools answer with their status
    /// and limits, nonexistent tools answer nothing, disabled toggles say so.
    #[test]
    fn versioned_question_set() {
        let mut e = crate::demo::demo_engine();
        let ask = |e: &mut Engine, q: &str| e.execute("help.search", &json!({"query": q})).unwrap();
        let r = ask(&mut e, "split");
        assert_eq!(r["build"], env!("CARGO_PKG_VERSION"));
        assert!(r["results"].as_array().unwrap().iter().any(|x| x["id"] == "edit.split_at"), "{r}");
        let r = ask(&mut e, "quickpunch");
        let row = &r["results"][0];
        assert_eq!(row["id"], "options.quickpunch");
        assert_eq!(row["status"], "DISABLED: not implemented");
        assert!(row["limits"][0].as_str().unwrap().contains("B04"));
        let r = ask(&mut e, "bend warp");
        assert!(r["results"].as_array().unwrap().iter().any(|x| x["id"] == "clip.warp_set"));
        assert!(r["results"][0]["limits"].as_array().unwrap().iter().any(|l| l.as_str().unwrap().contains("butt-joined")));
        for nonexistent in ["melodyne", "pitch correction note editor", "elastique", "dolby atmos renderer", "izotope rx"] {
            let r = ask(&mut e, nonexistent);
            assert_eq!(r["total"], 0, "no invented command for `{nonexistent}`: {r}");
        }
        let r = ask(&mut e, "listen audition");
        assert!(r["results"].as_array().unwrap().iter().any(|x| x["id"] == "audition.start"));
        let r = e.execute("help.limits", &json!({})).unwrap();
        assert!(r["limits"].as_array().unwrap().len() >= 10);
        // Exact id wins; the limit is capped; an empty query lists everything.
        let r = e.execute("help.search", &json!({"query": "edit.undo", "limit": 1})).unwrap();
        assert_eq!(r["results"][0]["id"], "edit.undo");
        assert_eq!(r["results"].as_array().unwrap().len(), 1);
        let all = e.execute("help.search", &json!({"query": "", "limit": 200})).unwrap();
        assert_eq!(all["total"].as_u64().unwrap() as usize, crate::command_specs().len());
    }
}
