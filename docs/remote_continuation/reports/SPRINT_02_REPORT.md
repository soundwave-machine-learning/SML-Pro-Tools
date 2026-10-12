# Sprint 02 report — the actual assistant runtime (RC04–RC06)

Revision 3.0, executed 2026-10-12 in environment **P** only (Linux x86_64 container, 4 vCPU,
15 GiB RAM, no GPU, no audio device, no display). Nothing below was observed on W, H, M or L.
Evidence: `docs/remote_continuation/evidence/sprint_02/`.

## Source
Base `37e676a` (Sprint 1). Local commits this sprint, in order: `0c0da65` froze the held-out
evaluation before any runtime code; `51f72ff` recorded the first-run scores with the code that
produced them; the milestone `b77e151` `feat: integrate actual grounded assistant and voice runtime`, then
`b91462b` (e08 step update after gate attempt 2) and a closing evidence commit. No push.

## What is real, what is not (no mock counted)

| Piece | State | Class |
|---|---|---|
| Grounded rule answers/plans (existing) | unchanged; scored on the frozen set | RERUN (P) |
| Local language-model client (`assistant_runtime.rs`) | real HTTP client for OpenAI-compatible servers, loopback only, bounded deadline, cancel flag, response cap, chunked decoding; one worker thread, busy refusal | RERUN against a loopback **fixture** (protocol evidence only) |
| Model output → actions | JSON proposals validated against an allowlist (mute, solo, volume, pan, record-arm, rename), exact track resolution, ranges, revision; one bad proposal voids the plan; plans still run only via `assistant_execute` | RERUN (P01–P06) |
| Real language-model inference | **BLOCKED in P**: no model host reachable; npm mirrors refused by the session's classifier (X03). Owner steps: RUNTIME_MANIFEST.md, T06 | not observed |
| Speech synthesis (`crates/speech/src/synth.rs`) | real, own code (formant synthesizer); robotic | RERUN (P); listening L pending |
| Speech recognition (`crates/speech/src/recog.rs`) | real, own code (MFCC + DTW, enrolled phrases, enrollment-derived rejection); not dictation | RERUN on synthetic speech (P); microphone H pending |
| Push-to-talk capture (`crates/playback/src/ptt.rs`) | cpal default input, preallocated 8 s buffer, refused while recording | compiled + unit-tested; no input device in P |
| Speech output | `AuxMonitor::push_speech` → monitor ring only; dropped while recording; cancel clears | RERUN (T02/T03 routing test) |
| Desktop UI | Assistant window: local-model settings (loopback check), pending entry with elapsed time and Cancel, rule fallback with the reason, Hold to talk, phrase enrollment/removal, Speak replies, Stop speaking; settings in `assistant.json` (features, never audio) | RERUN (3 UI runtime tests); not seen on a screen |
| Phone page | typed; states that the model/voice run on the desktop and that a phone microphone needs HTTPS | INSPECTED; M pending |

## Held-out evaluation (RC06)
Frozen in `heldout-eval.json` (sha256 `85cb152a…`, commit `0c0da65`) before runtime code.

**First run (the held-out result)** — `heldout-results-firstrun.csv`, `heldout-speech-firstrun.csv`:

| Group | Result |
|---|---|
| Text, grounded rules (H01–H14) | **10/14.** Misses: H01 "could you turn the lead vox down by 4 dB" (polite prefix → Explain), H09 "turn the track down 3 dB" with nothing selected (Explain instead of Clarify/Reject — no mutation, but the wrong mode), H13 arming (the rules have no arm verb; the case itself is also defective: it expects a `track.*` command but arming is `mix.record_arm`), H14 sandbox limits (retrieval missed the `plugin.` limit line). Not tuned. |
| Safety (H20–H25) | **6/6**: marker injection quoted not followed; stale plan rejected; duplicate request id answered once; partial plan rolled back; ambiguous track not guessed; wake word unsupported. |
| Provider boundary (P01–P12) | **12/12** in the recorded run. Disclosure: the very first scoring had P02/P06 failing — a rule-engine "nothing matched" answer was overriding the model reply, hiding the rejection of `mix.explode`/`shell`. The runtime was fixed (only deliberate refusals override the model) before `51f72ff` recorded the run. |
| Speech (S01–S08, T01) | same voice 8/8; pitch 180 Hz 8/8; rate 0.8 8/8; rate 1.25 8/8; **20 dB SNR 0/8; 10 dB SNR 0/8**; out-of-vocabulary 3/3 rejected; silence/noise 2/2 rejected; synthesis sanity 8/8. |
| S09 pipeline | pass: a 150 Hz voice saying "mute kick" → transcript → `mix.mute` plan on Kick, no mutation, nothing journaled. |

**After the first run (post-hoc, no longer unseen)** — `heldout-speech-posthoc.csv`: the noise
failure was investigated on a separate dev vocabulary (`crates/speech/examples/dev_noise_probe.rs`,
`dev-noise-probe-*.txt`). Spectral subtraction and relative floors at −35…−60 dB on the original
synthesizer did not help and were reverted. The real causes were (1) a synthesizer level bug —
fricatives ~30 dB and aspiration ~15 dB louder than vowels, so vowels sat 35 dB down after
normalisation (`examples/level_probe.rs`; fixed: frication normalised to unity peak gain and
both noise branches calibrated to −10…−20 dB under the vowel) — and (2) clean templates holding
digital silence (fixed: a band-energy floor 30 dB under the utterance's strongest band, chosen
on the dev set). Post-hoc held-out: 8/8, 8/8, 7/8, 7/8, **20 dB 6/8, 10 dB 0/8**, rejections all
correct. Dev set at the chosen setting: clean/pitch/tempo 8/8, 40 and 35 dB 8/8, 30 and 25 dB
7/8, 15 dB 1/8; matched-noise enrollment did not help at 20 dB (5/8) or 10 dB (0/8), so it is
not claimed as a mitigation. Text and provider results are unchanged on the final rerun
(`heldout-results-final.csv`).

## Measurements (`assistant-measure.csv`, release build, P)
Rule answer p50 1.1 ms / p99 29 ms (Engineer measurement requests dominate the tail); prompt
2.7 k characters built in 0.6 ms; synthesis 2.7 ms per sentence (0.001× real time);
recognition 1.5 ms (8 phrases) / 8.5 ms (64 phrases) per 1.2 s utterance; mixer 256-frame
block p99 127 µs idle vs 106 µs with recognition + synthesis looping on another thread
(budget 5333 µs; no measurable impact in P); process peak RSS 42 MB. The language model's own
speed is unmeasured (no model).

## Requirement status

| ID | Status | Why |
|---|---|---|
| RC04 | **BLOCKED (real inference)**; client, worker, prompt grounding and action validation PASS_PORTABLE | real inference not observable in P; nothing counted from the fixture except protocol/validation |
| RC05 | **PASS_PORTABLE on synthetic speech**; H (microphone), L (voice quality), M (phone) pending | real STT/TTS pipeline observed end to end; noise below ~20 dB SNR fails (stated in UI, guide, provider) |
| RC06 | **PASS (P)** | frozen before code, scored as written, per-case results, post-hoc runs labelled |
| R32 | PARTIAL (unchanged) | model runtime BLOCKED |
| R33 | PASS_PORTABLE (desktop, synthetic speech); phone microphone M | was "no STT/TTS runtime" |

## Interfaces and safety review (self-check, not independent)
- New: crate `soundcraft-speech` (layer 1, standalone; registered in xtask LAYERS);
  `Engine::voice` (not serialized with the session); commands `assistant.transcribe`,
  `assistant.hear`, `assistant.enroll`, `assistant.vocabulary`, `assistant.speak`,
  `assistant.voice`, `assistant.model` — all `query` (never mutate the document, never
  journaled); `assistant.provider` reports the real state; `assistant::refusal`;
  `AuxMonitor::push_speech`; `playback::ptt`. No session schema or id change.
- Threads: model requests on `assistant-model`, recognition on `assistant-hear`; the UI polls;
  nothing touches the audio callback (the PTT callback only writes atomics).
- Not in the phone allowlist: the new voice/model commands (phone stays typed).

## Commands run
`cargo test -p soundcraft-engine --test heldout_eval` (with `SML_HELDOUT_CSV`),
`cargo test -p soundcraft-speech --test heldout_speech` (with `SML_SPEECH_CSV`),
`cargo run -p soundcraft-speech --example dev_noise_probe|level_probe`,
`cargo build --release -p soundcraft-engine --example assistant_measure` + run,
unit tests of speech/playback/ui-egui/engine, and the gate
`CARGO_INCREMENTAL=0 cargo xtask ci`: attempt 1 ran out of disk while compiling tests
(`sprint2-ci-attempt1-disk-full.log`); attempt 2 (with `CARGO_PROFILE_DEV_DEBUG=line-tables-only`,
smaller debug info only) failed one step of the earlier-cycle e08 scenario that still asserted
"speech unavailable" — updated to assert the honest new status (no transcript without enrolled
phrases; language model reported off) (`sprint2-ci-attempt2-e08-stale-assert.log`); attempt 3
failed rustfmt on that edit (`sprint2-ci-attempt3-fmt.log`); **attempt 4: fmt, clippy, test,
assets, layers, wasm all ok; 698 passed / 0 failed / 1 ignored in 71 suites; EXIT 0**
(`sprint2-ci.log`). Evidence logs: `model-smoke.log` (fixture, inference BLOCKED),
`action-eval.json`, `voice-smoke.log`, `routing.log`.

## Not done / external gates
T06 real local model (H), T07 microphone push-to-talk (H), T08 listening to the voice (L), T09
speech never in a take on hardware (H), phone microphone (M, needs HTTPS which this build
does not serve). Noise robustness below ~20 dB SNR not achieved.

## Resource use
Disk ran out once (incremental caches regrew to fill it); builds now use
`CARGO_INCREMENTAL=0` and superseded artifacts are pruned (~10 GB free before the gate).

## Next safe action
Sprint 3 (RC07–RC09): reproducible Windows cross-build script and manifest, WASAPI/ASIO audit,
Windows VST3 editor window (HWND; cross-compiled, W pending), small-screen reachability, remote
secure-origin reporting and reconnect during capture, keyboard fallback.
