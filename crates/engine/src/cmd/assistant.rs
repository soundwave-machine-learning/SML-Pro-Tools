//! Assistant commands (R32/R33): `assistant.ask` answers without mutation; `assistant.execute`
//! runs a previewed plan as one undo step; `assistant.provider` states what is configured.
//! Speech (RC05): `assistant.enroll`/`assistant.vocabulary` manage the enrolled command phrases,
//! `assistant.transcribe`/`assistant.hear` recognise them (a heard phrase is answered exactly as
//! typed text; plans are only previewed), `assistant.speak` queues built-in synthesis for the
//! host's monitor-only output. `assistant.model` configures the local model server that the
//! desktop Assistant window uses (see `assistant_runtime`). These are `query` commands: they
//! never touch the document and are never journaled (the journal must not hold enrolled audio).

use super::*;
use crate::assistant::{Request, Step, Unavailable};
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
        cmd!(query "assistant.provider", "Assistant Provider", [], None, "{} → {name, language_model, speech_to_text, text_to_speech, privacy, phrases}", always, |e, _| {
            let v = &e.voice;
            Ok(json!({
                "name": "SoundCraft assistant: grounded rules, optional local language model, built-in speech",
                "language_model": match &v.model {
                    Some(c) => format!("local OpenAI-compatible server at {} (model `{}`); used by the desktop Assistant window; no model ships with SoundCraft", c.url, c.model),
                    None => "off: answers come from the grounded rules (turn on a local model server in Window › Assistant)".to_string(),
                },
                "speech_to_text": format!("built-in enrolled-command recognizer (MFCC + DTW, {} phrase(s) enrolled): recognises enrolled phrases only, not dictation; needs a quiet room (measured on synthetic speech: reliable at 30 dB SNR and better, fails at 10 dB)", v.recognizer.phrases.len()),
                "text_to_speech": "built-in rule-based formant synthesizer: intelligible-but-robotic English; monitor output only, silent while recording",
                "privacy": "nothing leaves this machine: the model endpoint must be loopback; enrolled phrases are stored as features, never as audio",
                "push_to_talk": "hold the button (desktop) to record from the default input; never while recording; no always-listening mode",
                "phrases": v.recognizer.phrases.iter().map(|p| p.text.clone()).collect::<Vec<_>>(),
                "speak_replies": v.speak_replies,
            }))
        }),
        cmd!(query "assistant.transcribe", "Transcribe Speech", [], None, "{pcm: [f32] | pcm16: [i16], sample_rate} → {transcript: text|null, reason, candidates, threshold} — enrolled phrases only", always, |e, p| {
            let (pcm, rate) = pcm_param("assistant.transcribe", p)?;
            let rec = e.voice.recognizer.recognize(&pcm, rate);
            Ok(json!({"transcript": rec.phrase, "reason": rec.reason, "candidates": rec.ranking.into_iter().take(3).collect::<Vec<_>>(), "threshold": rec.threshold}))
        }),
        cmd!(query "assistant.hear", "Push-to-Talk Turn", [], None, "{pcm: [f32] | pcm16: [i16], sample_rate} → {transcript, reason, candidates, response: assistant.ask result | null} — the plan is only previewed", always, |e, p| {
            let (pcm, rate) = pcm_param("assistant.hear", p)?;
            Ok(json!(e.assistant_hear(&pcm, rate)))
        }),
        cmd!(query "assistant.enroll", "Enroll Voice Phrase", [], None, "{phrase, pcm: [f32] | pcm16: [i16], sample_rate} → {phrases} — one more recording of a command phrase (max 64 phrases × 4)", always, |e, p| {
            let phrase = str_param(p, "phrase").ok_or_else(|| bad("assistant.enroll", "`phrase` required"))?.to_string();
            let (pcm, rate) = pcm_param("assistant.enroll", p)?;
            e.voice.recognizer.enroll(&phrase, &pcm, rate).map_err(|m| bad("assistant.enroll", &m))?;
            Ok(json!({"phrases": vocabulary(e)}))
        }),
        cmd!(query "assistant.vocabulary", "Voice Phrases", [], None, "{remove?: phrase} → {phrases: [{text, recordings}]}", always, |e, p| {
            if let Some(r) = str_param(p, "remove") && !e.voice.recognizer.remove(r) {
                return Err(bad("assistant.vocabulary", format!("no enrolled phrase `{r}`")));
            }
            Ok(json!({"phrases": vocabulary(e)}))
        }),
        cmd!(query "assistant.speak", "Speak Reply", [], None, "{text} → {seconds} — queued for the monitor output only (never a render, bounce or capture; dropped while recording)", always, |e, p| {
            let text = str_param(p, "text").unwrap_or("").to_string();
            let n = e.voice.say(&text);
            if n == 0 {
                return Err(bad("assistant.speak", "nothing to say"));
            }
            Ok(json!({"seconds": n as f64 / f64::from(soundcraft_speech::SPEECH_RATE), "queued": e.voice.queued_speech()}))
        }),
        cmd!(query "assistant.voice", "Assistant Voice", [], None, "{pitch_hz?: 50..400, rate?: 0.5..2, speak_replies?: bool, stop?: true} → settings", always, |e, p| {
            let v = &mut e.voice;
            if let Some(x) = p.get("pitch_hz").and_then(Value::as_f64) {
                v.voice.pitch_hz = (x as f32).clamp(50.0, 400.0);
            }
            if let Some(x) = p.get("rate").and_then(Value::as_f64) {
                v.voice.rate = (x as f32).clamp(0.5, 2.0);
            }
            if let Some(b) = p.get("speak_replies").and_then(Value::as_bool) {
                v.speak_replies = b;
            }
            if p.get("stop").and_then(Value::as_bool) == Some(true) {
                v.clear_speech();
            }
            Ok(json!({"pitch_hz": v.voice.pitch_hz, "rate": v.voice.rate, "speak_replies": v.speak_replies}))
        }),
        cmd!(query "assistant.model", "Assistant Model Server", [], None, "{enabled?: bool, url?, model?, timeout_ms?} → config — a LOCAL OpenAI-compatible server (llama.cpp, Ollama, LM Studio); non-loopback hosts are refused", always, |e, p| {
            let mut c = e.voice.model.clone().unwrap_or_default();
            if let Some(u) = str_param(p, "url") {
                c.url = u.trim().to_string();
            }
            if let Some(m) = str_param(p, "model") {
                c.model = m.trim().chars().take(128).collect();
            }
            if let Some(t) = p.get("timeout_ms").and_then(Value::as_u64) {
                c.timeout_ms = t.clamp(1_000, 600_000);
            }
            let enabled = p.get("enabled").and_then(Value::as_bool).unwrap_or(e.voice.model.is_some());
            if enabled {
                crate::assistant_runtime::loopback_addrs(&c.url).map_err(|m| bad("assistant.model", m.to_string()))?;
                e.voice.model = Some(c.clone());
            } else {
                e.voice.model = None;
            }
            Ok(json!({"enabled": e.voice.model.is_some(), "url": c.url, "model": c.model, "timeout_ms": c.timeout_ms}))
        }),
    ]
}

/// `pcm` (f32) or `pcm16` samples and `sample_rate`; at most 30 s.
fn pcm_param(cmd: &str, p: &Value) -> Result<(Vec<f32>, u32)> {
    let rate = p
        .get("sample_rate")
        .and_then(Value::as_u64)
        .and_then(|r| u32::try_from(r).ok())
        .filter(|r| (8_000..=192_000).contains(r))
        .ok_or_else(|| bad(cmd, "`sample_rate` 8000..192000 required"))?;
    let max = rate as usize * 30;
    let pcm: Vec<f32> = if let Some(a) = p.get("pcm").and_then(Value::as_array) {
        a.iter().take(max).map(|v| v.as_f64().unwrap_or(0.0) as f32).collect()
    } else if let Some(a) = p.get("pcm16").and_then(Value::as_array) {
        a.iter().take(max).map(|v| v.as_i64().unwrap_or(0).clamp(-32_768, 32_767) as f32 / 32_768.0).collect()
    } else {
        return Err(bad(cmd, "`pcm` or `pcm16` samples required"));
    };
    Ok((pcm, rate))
}

fn vocabulary(e: &Engine) -> Vec<Value> {
    e.voice.recognizer.phrases.iter().map(|p| json!({"text": p.text, "recordings": p.templates.len()})).collect()
}
