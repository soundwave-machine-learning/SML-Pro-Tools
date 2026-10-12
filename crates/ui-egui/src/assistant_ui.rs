//! The desktop side of the assistant runtime (RC04/RC05): the local model worker and its pending
//! request (cancel, progress, fallback to the grounded rules), push-to-talk capture and
//! recognition off the UI thread, enrollment of command phrases, speech output drained into the
//! player's monitor-only ring, and persistence of the voice/model settings (`assistant.json`
//! beside `ui.json`; enrolled phrases are cepstral features, never audio).

use crate::SoundApp;
use serde_json::{Value, json};
use soundcraft_engine::assistant_runtime::{ChatError, ChatWorker, EndpointConfig, LocalEndpoint};
use soundcraft_speech::recog::{Recognition, Recognizer};
use soundcraft_speech::synth::Voice;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Instant;

/// What a push-to-talk recording is for.
#[derive(Debug, Clone, PartialEq)]
pub enum PttPurpose {
    Ask,
    Enroll(String),
}

struct Pending {
    id: u64,
    question: String,
    revision: u64,
    started: Instant,
    log_index: usize,
}

enum Heard {
    Recognised(Recognition),
    Audio(String, Vec<f32>, u32),
}

/// Settings the user edits in the window (applied with "Apply").
#[derive(Debug, Clone, PartialEq)]
pub struct ModelForm {
    pub enabled: bool,
    pub url: String,
    pub model: String,
    pub timeout_s: f32,
}

impl Default for ModelForm {
    fn default() -> Self {
        let c = EndpointConfig::default();
        ModelForm { enabled: false, url: c.url, model: c.model, timeout_s: c.timeout_ms as f32 / 1000.0 }
    }
}

#[derive(Default)]
pub struct AssistantRuntime {
    worker: Option<(EndpointConfig, ChatWorker)>,
    pending: Option<Pending>,
    next_id: u64,
    #[cfg(not(target_arch = "wasm32"))]
    ptt: Option<(soundcraft_playback::ptt::PushToTalk, PttPurpose)>,
    hearing: Option<Receiver<Heard>>,
    pub enroll_phrase: String,
    pub status: String,
    pub form: ModelForm,
    /// Where the settings are kept (native hosts set it).
    pub store: Option<PathBuf>,
}

/// What `assistant.json` holds.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Stored {
    #[serde(default)]
    voice: Option<Voice>,
    #[serde(default)]
    speak_replies: bool,
    #[serde(default)]
    model: Option<EndpointConfig>,
    #[serde(default)]
    phrases: Recognizer,
}

impl AssistantRuntime {
    pub fn is_busy(&self) -> bool {
        self.pending.is_some() || self.hearing.is_some()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn is_recording(&self) -> bool {
        self.ptt.is_some()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn is_recording(&self) -> bool {
        false
    }
}

impl SoundApp {
    /// Loads `assistant.json` into the engine (missing or unreadable: defaults, logged).
    pub fn assistant_load(&mut self) {
        let Some(path) = self.assistant.store.clone() else { return };
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        match serde_json::from_str::<Stored>(&text) {
            Ok(s) => {
                let v = &mut self.engine.voice;
                v.voice = s.voice.unwrap_or_default();
                v.speak_replies = s.speak_replies;
                v.recognizer = s.phrases;
                v.model = s.model.filter(|c| soundcraft_engine::assistant_runtime::loopback_addrs(&c.url).is_ok());
                if let Some(c) = &v.model {
                    self.assistant.form =
                        ModelForm { enabled: true, url: c.url.clone(), model: c.model.clone(), timeout_s: c.timeout_ms as f32 / 1000.0 };
                }
            }
            Err(e) => log::warn!("assistant settings not loaded ({}): {e}", path.display()),
        }
    }

    pub fn assistant_save(&self) {
        let Some(path) = &self.assistant.store else { return };
        let v = &self.engine.voice;
        let s = Stored { voice: Some(v.voice), speak_replies: v.speak_replies, model: v.model.clone(), phrases: v.recognizer.clone() };
        match serde_json::to_string(&s) {
            Ok(text) => {
                let tmp = path.with_extension("json.tmp");
                if let Err(e) = std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, path)) {
                    log::warn!("assistant settings not saved: {e}");
                }
            }
            Err(e) => log::warn!("assistant settings not saved: {e}"),
        }
    }

    /// Applies the model form (validated: loopback only).
    pub fn assistant_apply_model(&mut self) {
        let f = self.assistant.form.clone();
        let r = self.engine.execute(
            "assistant.model",
            &json!({"enabled": f.enabled, "url": f.url, "model": f.model, "timeout_ms": (f.timeout_s.max(1.0) * 1000.0) as u64}),
        );
        self.assistant.status = match r {
            Ok(v) if v["enabled"] == true => format!("local model on: {}", v["url"].as_str().unwrap_or("")),
            Ok(_) => "local model off: answers come from the grounded rules".into(),
            Err(e) => e.to_string(),
        };
        self.assistant.worker = None;
        self.assistant_save();
    }

    /// Asks `text`: through the local model when one is on (and the rules do not refuse), else
    /// the grounded rules. Model answers arrive later through [`SoundApp::assistant_poll`].
    pub fn assistant_ask_text(&mut self, text: String, shortcuts: Value) {
        let model = self.engine.voice.model.clone();
        let refused = soundcraft_engine::assistant::refusal(&text).is_some();
        if let (Some(cfg), false, false) = (model, refused, cfg!(target_arch = "wasm32")) {
            if self.assistant.worker.as_ref().is_none_or(|(c, _)| *c != cfg) {
                self.assistant.worker = Some((cfg.clone(), ChatWorker::new(Arc::new(LocalEndpoint::new(cfg)))));
            }
            let (system, user, revision) = self.engine.assistant_model_prompt(&text);
            self.assistant.next_id += 1;
            let id = self.assistant.next_id;
            let submitted = self.assistant.worker.as_ref().map(|(_, w)| w.submit(id, system, user));
            match submitted {
                Some(Ok(())) => {
                    self.ui
                        .assistant_log
                        .push((text.clone(), json!({"mode": "pending", "answer": "thinking… (local model; Cancel stops it)", "revision": revision})));
                    self.assistant.pending =
                        Some(Pending { id, question: text, revision, started: Instant::now(), log_index: self.ui.assistant_log.len() - 1 });
                    return;
                }
                Some(Err(e)) => self.assistant.status = format!("{e}; answered by the rules instead"),
                None => {}
            }
        }
        let r = self
            .engine
            .execute("assistant.ask", &json!({"text": text, "shortcuts": shortcuts}))
            .unwrap_or_else(|e| json!({"mode": "reject", "answer": e.to_string()}));
        self.assistant_speak_reply(&r);
        self.ui.assistant_log.push((text, r));
    }

    pub fn assistant_cancel(&mut self) {
        if let (Some(p), Some((_, w))) = (&self.assistant.pending, &self.assistant.worker) {
            w.cancel(p.id);
        }
        let _ = self.engine.execute("assistant.voice", &json!({"stop": true}));
        if let Some(p) = &self.player {
            p.aux().clear();
        }
    }

    fn assistant_speak_reply(&mut self, r: &Value) {
        if self.engine.voice.speak_replies
            && let Some(a) = r["answer"].as_str()
        {
            // The first sentence or line: speech is for confirmation, the text has the detail.
            let short: String = a.split(['\n', '.']).next().unwrap_or("").chars().take(200).collect();
            if !short.trim().is_empty() {
                self.engine.voice.say(&short);
            }
        }
    }

    /// Every frame: model results, recognition results, and queued speech into the monitor ring.
    pub fn assistant_poll(&mut self) {
        let done = self.assistant.worker.as_ref().and_then(|(_, w)| w.poll());
        if let Some(d) = done
            && let Some(p) = self.assistant.pending.take_if(|p| p.id == d.id)
        {
            let provider = self.assistant.worker.as_ref().map_or(String::new(), |(_, w)| w.name().to_string());
            let mut r = match d.result {
                Ok(reply) => json!(self.engine.assistant_from_model(&p.question, &reply, &provider, p.revision)),
                Err(ChatError::Cancelled) => json!({"mode": "reject", "answer": "cancelled", "revision": self.engine.revision}),
                Err(e) => {
                    let mut r = self
                        .engine
                        .execute("assistant.ask", &json!({"text": p.question}))
                        .unwrap_or_else(|e| json!({"mode": "reject", "answer": e.to_string()}));
                    if let Some(l) = r["labels"].as_array_mut() {
                        l.push(json!(format!("model unavailable ({e}); answered by the grounded rules")));
                    } else {
                        r["labels"] = json!([format!("model unavailable ({e}); answered by the grounded rules")]);
                    }
                    r
                }
            };
            r["elapsed_ms"] = json!(d.elapsed.as_millis() as u64);
            self.assistant_speak_reply(&r);
            if let Some((_, entry)) = self.ui.assistant_log.get_mut(p.log_index) {
                *entry = r;
            }
        }
        if let Some(rx) = &self.assistant.hearing {
            match rx.try_recv() {
                Ok(Heard::Recognised(rec)) => {
                    self.assistant.hearing = None;
                    match rec.phrase {
                        Some(t) => {
                            self.assistant.status = format!("heard: \"{t}\"");
                            self.assistant_ask_text(t, json!({}));
                            // The visible transcript: marked as heard, not typed.
                            if let Some((q, _)) = self.ui.assistant_log.last_mut() {
                                *q = format!("🎤 {q}");
                            }
                        }
                        None => {
                            let near = rec
                                .ranking
                                .first()
                                .map(|(p, d)| format!(" (closest: \"{p}\", distance {d:.0} > {:.0})", rec.threshold))
                                .unwrap_or_default();
                            self.assistant.status = format!("not recognised: {}{near}. Enrolled phrases only; nothing was done.", rec.reason);
                        }
                    }
                }
                Ok(Heard::Audio(phrase, pcm, rate)) => {
                    self.assistant.hearing = None;
                    let r = self.engine.voice.recognizer.enroll(&phrase, &pcm, rate);
                    self.assistant.status = match r {
                        Ok(()) => format!("enrolled \"{phrase}\""),
                        Err(e) => format!("not enrolled: {e}"),
                    };
                    self.assistant_save();
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.assistant.hearing = None,
            }
        }
        if let Some(player) = &self.player {
            while let Some(speech) = self.engine.voice.take_speech() {
                player.aux().push_speech(&speech, soundcraft_speech::SPEECH_RATE, player.device_rate);
            }
        } else {
            // No audio output (tests, offscreen): queued speech is discarded, never kept.
            self.engine.voice.clear_speech();
        }
    }

    /// Push-to-talk button pressed: opens the default input (refused while recording).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn assistant_ptt_press(&mut self, purpose: PttPurpose) {
        if self.assistant.ptt.is_some() || self.assistant.hearing.is_some() {
            return;
        }
        if self.engine.transport.recording {
            self.assistant.status = "push-to-talk is off while recording (the take's input is never shared)".into();
            return;
        }
        match soundcraft_playback::ptt::PushToTalk::start() {
            Ok(p) => {
                self.assistant.status = format!("listening on {} — release to stop", p.device_name);
                self.assistant.ptt = Some((p, purpose));
            }
            Err(e) => self.assistant.status = format!("push-to-talk unavailable: {e}"),
        }
    }

    /// Push-to-talk button released: recognition (or enrollment features) runs off the UI thread.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn assistant_ptt_release(&mut self) {
        let Some((p, purpose)) = self.assistant.ptt.take() else { return };
        let (pcm, rate) = p.finish();
        let (tx, rx) = channel();
        self.assistant.hearing = Some(rx);
        self.assistant.status = "recognising…".into();
        match purpose {
            PttPurpose::Ask => {
                let rec = self.engine.voice.recognizer.clone();
                let spawned = std::thread::Builder::new().name("assistant-hear".into()).spawn(move || {
                    let _ = tx.send(Heard::Recognised(rec.recognize(&pcm, rate)));
                });
                if spawned.is_err() {
                    self.assistant.hearing = None;
                    self.assistant.status = "could not start recognition".into();
                }
            }
            PttPurpose::Enroll(phrase) => {
                let _ = tx.send(Heard::Audio(phrase, pcm, rate));
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn assistant_ptt_press(&mut self, _purpose: PttPurpose) {
        self.assistant.status = "push-to-talk needs the desktop app".into();
    }

    #[cfg(target_arch = "wasm32")]
    pub fn assistant_ptt_release(&mut self) {}

    /// Seconds the pending model request has been running.
    pub fn assistant_pending_secs(&self) -> Option<f32> {
        self.assistant.pending.as_ref().map(|p| p.started.elapsed().as_secs_f32())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::time::Duration;

    /// A loopback server answering every request with `content` as the model message.
    fn fixture(content: &'static str) -> String {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for c in l.incoming() {
                let Ok(mut c) = c else { continue };
                let mut buf = [0u8; 65_536];
                let _ = c.set_read_timeout(Some(Duration::from_millis(300)));
                let _ = c.read(&mut buf);
                let body = json!({"choices": [{"message": {"content": content}}]}).to_string();
                let _ = write!(c, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len());
            }
        });
        format!("http://127.0.0.1:{port}/v1/chat/completions")
    }

    fn app() -> SoundApp {
        let mut e = soundcraft_engine::Engine::default();
        e.session_mut().add_track(soundcraft_model::TrackKind::Audio, soundcraft_model::ChannelFormat::Mono, Some("Kick"));
        SoundApp::new(e, None, crate::Services::default())
    }

    fn wait(app: &mut SoundApp) {
        let t0 = Instant::now();
        while app.assistant.is_busy() && t0.elapsed() < Duration::from_secs(10) {
            app.assistant_poll();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// RC04 desktop path: the window's ask goes to the local model off the UI thread, shows a
    /// pending entry, and the reply becomes a checked plan; nothing runs until "Run plan".
    #[test]
    fn model_answers_become_checked_plans_without_running() {
        let mut app = app();
        app.assistant.form = ModelForm {
            enabled: true,
            url: fixture("{\"answer\":\"Muting Kick.\",\"actions\":[{\"command\":\"mix.mute\",\"track\":\"Kick\",\"params\":{\"value\":true}}]}"),
            model: "fixture".into(),
            timeout_s: 5.0,
        };
        app.assistant_apply_model();
        let rev = app.engine.revision;
        app.assistant_ask_text("mute the kick".into(), json!({}));
        assert_eq!(app.ui.assistant_log.last().unwrap().1["mode"], "pending");
        wait(&mut app);
        let r = &app.ui.assistant_log.last().unwrap().1;
        assert_eq!(r["mode"], "operate", "{r}");
        assert_eq!(r["plan"][0]["command"], "mix.mute");
        assert!(r["elapsed_ms"].is_u64());
        assert_eq!(app.engine.revision, rev, "nothing ran");
        assert!(!app.engine.session().track_by_name("Kick").unwrap().mixer.mute);
    }

    /// A model server that is not running: the rules answer and say why.
    #[test]
    fn an_unreachable_model_falls_back_to_the_rules() {
        let mut app = app();
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        app.assistant.form =
            ModelForm { enabled: true, url: format!("http://127.0.0.1:{port}/v1/chat/completions"), model: "x".into(), timeout_s: 2.0 };
        app.assistant_apply_model();
        app.assistant_ask_text("mute kick".into(), json!({}));
        wait(&mut app);
        let r = &app.ui.assistant_log.last().unwrap().1;
        assert_eq!(r["mode"], "operate", "{r}");
        assert!(r["labels"].to_string().contains("model unavailable"), "{r}");
        // A non-loopback endpoint is refused when applied.
        app.assistant.form.url = "http://192.168.0.2:8080/v1/chat/completions".into();
        app.assistant_apply_model();
        assert!(app.assistant.status.contains("not this machine"), "{}", app.assistant.status);
    }

    /// Settings round-trip through assistant.json; enrolled phrases are features, not audio.
    #[test]
    fn settings_persist_without_audio() {
        let dir = std::env::temp_dir().join(format!("sml-assistant-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut app = app();
        app.assistant.store = Some(dir.join("assistant.json"));
        let mut pcm = vec![0.0f32; 3_200];
        pcm.extend(soundcraft_speech::synth::synthesize("play", Voice::default()));
        pcm.extend(vec![0.0f32; 3_200]);
        app.engine.voice.recognizer.enroll("play", &pcm, 16_000).unwrap();
        app.engine.voice.speak_replies = true;
        app.assistant_save();
        let text = std::fs::read_to_string(dir.join("assistant.json")).unwrap();
        assert!(text.len() < pcm.len() * 4, "features are smaller than the audio");
        let mut b = app_fresh(dir.join("assistant.json"));
        b.assistant_load();
        assert_eq!(b.engine.voice.recognizer.phrases.len(), 1);
        assert!(b.engine.voice.speak_replies);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn app_fresh(store: PathBuf) -> SoundApp {
        let mut a = app();
        a.assistant.store = Some(store);
        a
    }
}
