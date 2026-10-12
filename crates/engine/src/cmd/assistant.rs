//! Assistant commands (R32/R33): `assistant.ask` answers without mutation; `assistant.execute`
//! runs a previewed plan as one undo step; `assistant.provider` states what is configured.
//! Speech (`assistant.transcribe`, `assistant.speak`) goes through the same provider boundary
//! and reports `unavailable` honestly (E06).

use super::*;
use crate::assistant::{self, Provider, Request, Step, Unavailable};
use crate::cmd;
use serde_json::json;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "assistant.ask", "Ask the Assistant", ["Help"], None, "{text, shortcuts?: {command id: chord}} → {mode: explain|operate|engineer|unsupported|clarify|reject|needs_authorization, answer, citations, plan: [{command, params, describe, risk}], measurements, labels, uncertainty, revision, provider} — never mutates; a plan is executed with assistant.execute", always, |e, p| {
            let text = str_param(p, "text").unwrap_or("").to_string();
            let mut req = Request { text, ..Request::default() };
            if let Some(m) = p.get("shortcuts").and_then(Value::as_object) {
                for (k, v) in m {
                    if let Some(s) = v.as_str() {
                        req.shortcuts.insert(k.clone(), s.to_string());
                    }
                }
            }
            Ok(json!(e.assistant_ask(&req, &Unavailable)))
        }),
        cmd!(noundo "assistant.execute", "Execute Assistant Plan", [], None, "{plan: [{command, params}], revision, request_id?} — the previewed plan at the revision it was built for; one undo step; any rejected step means zero mutation", always, |e, p| {
            let steps: Vec<Step> = p
                .get("plan")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|s| {
                            let command = s.get("command")?.as_str()?.to_string();
                            let risk = crate::envelope::risk_of(&command, crate::find_command(&command).is_some_and(|c| c.undoable));
                            Some(Step { command, params: s.get("params").cloned().unwrap_or_else(|| json!({})), describe: s.get("describe").and_then(Value::as_str).unwrap_or("").to_string(), risk })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let revision = p.get("revision").and_then(Value::as_u64).ok_or_else(|| bad("assistant.execute", "`revision` (the plan's) required"))?;
            let request_id = str_param(p, "request_id").unwrap_or("").to_string();
            Ok(e.assistant_execute(&steps, revision, &request_id))
        }),
        cmd!(query "assistant.provider", "Assistant Provider", [], None, "{} → {name, language_model, speech_to_text, text_to_speech, privacy}", always, |_, _| {
            Ok(json!({
                "name": Provider::name(&assistant::Unavailable),
                "language_model": "unavailable",
                "speech_to_text": "unavailable",
                "text_to_speech": "unavailable",
                "privacy": "no audio or text leaves this machine; no provider is configured (E06)",
                "push_to_talk": "the transcript field accepts typed text; push-to-talk records nothing until a speech provider is configured",
            }))
        }),
        cmd!(query "assistant.transcribe", "Transcribe Speech", [], None, "{pcm16: [..], sample_rate} → {text} | error unavailable", always, |_, _| {
            Err(bad("assistant.transcribe", "speech recognition is not configured (E06): type the request instead"))
        }),
        cmd!(query "assistant.speak", "Speak Reply", [], None, "{text} → monitor-only speech | error unavailable", always, |_, _| {
            Err(bad("assistant.speak", "speech synthesis is not configured (E06); replies are shown as text"))
        }),
    ]
}
