# Assistant runtime manifest (RC04/RC05, Sprint 2)

State at the Sprint 2 commit, environment P. Nothing below was run on the owner's machine.

| Part | What ships | Version / provenance | Licence | Size / memory | Status |
|---|---|---|---|---|---|
| Grounded rules | `crates/engine/src/assistant.rs` | this repo | MIT OR Apache-2.0 (project) | — | working (P); held-out text 10/14 |
| Language-model client | `crates/engine/src/assistant_runtime.rs`: OpenAI-compatible `POST /v1/chat/completions` over plain HTTP/1.1 to **loopback only**; one bounded, cancellable worker thread; JSON-only action proposals validated against an allowlist | this repo | project | prompt ≈ 2.7 k characters (≈ 700 tokens) per question | protocol + validation verified against a loopback fixture (P01–P12 pass); **real inference BLOCKED in P** (no model reachable, X03) |
| Language model | **none shipped** | — | the owner's choice | depends on the model | owner action (below) |
| Speech synthesis | `crates/speech/src/synth.rs`: rule-based cascade/parallel formant synthesizer, hand-written lexicon + letter-to-sound rules | this repo (original code; textbook method; formant targets are standard published averages) | project | no weights; ~3 ms per sentence (0.001× real time) | working (P), robotic; intelligibility not judged by listeners (L pending) |
| Speech recognition | `crates/speech/src/recog.rs`: enrolled-command recognizer, MFCC + DTW, enrollment-derived rejection | this repo (original code; textbook method) | project | templates are features (≈ 50 frames × 12 floats per recording); 1.5 ms (8 phrases) / 8.5 ms (64) per utterance | working on synthetic speech (P); microphone use H pending |
| Push-to-talk capture | `crates/playback/src/ptt.rs` (cpal default input, 8 s preallocated mono buffer) | this repo | project | 8 s × device rate × 4 bytes | compiled and unit-tested; **no input device in P** (H pending) |
| Speech output | `AuxMonitor::push_speech` (monitor ring only) | this repo | project | — | routing proven by test (T02/T03) |

Process peak resident memory while measuring (16-track session + recognition + synthesis):
42 MB (`evidence/sprint_02/assistant-measure.csv`). Recognition and synthesis looping on another
thread did not raise the mixer's 256-frame block compute (p99 106 µs vs 127 µs idle, budget
5333 µs) in P.

## Privacy policy (as implemented)
- The model endpoint must resolve to loopback; anything else is refused before a socket opens
  (`loopback_addrs`, case P12). No API keys, no accounts, no paid service.
- The prompt contains the session's track names/ids/levels and the command help for the
  question. Marker/clip text is not sent to the model.
- Push-to-talk audio stays in memory; enrolled phrases are saved as cepstral features in
  `assistant.json` (never audio); voice commands are not written to the command journal.

## Turning on a real local model (owner action, H)
1. Install a local OpenAI-compatible server, e.g. llama.cpp's `llama-server`, Ollama or LM Studio
   (each documents its own installation; check each one's licence on its site).
2. Download a model whose licence you accept (for example an instruction-tuned model in GGUF
   format for llama.cpp). Its licence, not SoundCraft's, governs it.
3. Start the server on this computer (for llama.cpp: `llama-server -m <model.gguf> --port 8080`).
4. In SoundCraft: Window › Assistant › *Local language model*: tick *Use the local model*, keep
   `http://127.0.0.1:8080/v1/chat/completions` (Ollama: port 11434), Apply.
5. Ask "mute Kick": the reply should arrive with an elapsed time and a plan you run yourself.
   Record the model name, its answer time and memory use in TARGET_PENDING T06.
