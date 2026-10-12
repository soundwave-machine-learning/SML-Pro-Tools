# Sprint 4 design dossier — assisted studio (R32–R36, R38)

Written 2026-10-12 before the Sprint 4 code. Decisions about this repository; statuses stay in
`REQUIREMENT_STATUS.csv`. E06 is BLOCKED for generative parts (no model/STT/TTS runtime or
product credential in P; recorded in EXPERIMENT_DECISIONS); everything below is built so that a
provider can be configured later without changing the action boundary.

## What exists
- Knowledge: `envelope::knowledge` (every command: id, label, menu, shortcut, params, risk,
  status, disabled reason) + `help.search/limits` (R22), versioned by the build.
- Operate boundary: `Envelope` submissions with request ids, expected revision, dedup, risk
  classes, remote allowlist (R09/R21).
- Measurements: `LoudnessMeter` (BS.1770 K-weighting, gated integrated, momentary/short-term,
  4× true peak), `PeakMeter`, `RmsMeter`, offline `peak_db`/`normalize`; strip meters.
- Export: `file.bounce_mix/stems` (wav/aiff/flac, 16/24/32f, TPDF dither on integer depths,
  tail, failed-plugin refusal); no sample-rate conversion option yet (`offline::resample`
  exists); report = path/bytes/seconds/peak/LUFS/true peak.
- Mastering layout: navigation shell only (R18); reference path and chain do not exist.
- Remote page: transport/arm/markers/faders/meters/monitor; no text input.

## Decisions
1. **Assistant service** = `crates/engine/src/assistant.rs` (engine-side, provider-agnostic):
   `Request {text, context: {selection, revision}, mode?: explain|operate|engineer|auto}` →
   `Response {mode, answer, citations: [command ids / doc ids], plan?: [Envelope], measurements?,
   uncertainty, unsupported?: reason}`. Explain answers from the knowledge corpus + the
   USER_GUIDE section index (doc ids `guide:§n`) with the active shortcut supplied by the host.
   Operate builds a plan of validated envelopes (ids resolved from explicit selection/names,
   expected revision = current); the plan is previewed, then executed atomically in one undo
   transaction through the existing submission path; any invalid/stale/unauthorized step
   rejects the whole plan with zero mutation. Engineer runs deterministic rules on measured
   audio only (peak/true-peak/LUFS/RMS/noise-floor of the selected clips or range, clipping
   locations, silence/breath candidates via the strip-silence analyser, send/bus state) and
   proposes concrete parameters of registered real plugins only (from
   `engine.plugin_capabilities`); each finding is labelled measured / inferred / preference.
   Model boundary: `trait Provider { complete(..) }` with the only implementation
   `Unavailable` (E06); retrieved text is data (the prompt-injection evaluation runs against the
   rule engine: injected imperatives in a clip name/marker/doc never produce a plan step).
2. **Evaluation set**: `docs/evidence/next_cycle/sprint_04/assistant-eval.json` — fixed
   questions with expected mode/citations/outcome: factual (shortcut of Split in both
   profiles, where Nudge lives, Bend limits), unsupported (ARA, wake word, QuickPunch),
   ambiguous target (two tracks named Vox), deleted target, stale revision, duplicate remote
   action, injected instruction in retrieved text. Scored by a test; results CSV.
3. **Voice (R33)**: push-to-talk UI (desktop button + phone page button) with a visible
   transcript field and cancel; STT/TTS go through the provider boundary → `Unavailable` state
   shown honestly; typed text takes the same path. Spoken replies (when a TTS exists) are
   routed to the monitor stage only (playback `speech` ring summed after the bounce point, like
   the metronome but on the monitor path) and muted while recording; a test proves no speech
   sample reaches a bounce or the capture path. Model timeout/cancel exercised with a stub
   provider that sleeps.
4. **Mastering workspace (R34)**: `MasteringSetup` in the session (`session.mastering`:
   source = main mix | imported stereo source id, chain = ordered inserts on the Master
   track's user inserts (applied exactly once: the export path is the main mix), reference =
   {source id, gain_db, enabled} on a monitor-only path that bypasses the master chain and never
   reaches a bounce, level-match gain computed from measured integrated loudness (audition only;
   output normalization only when explicitly asked). Meters window: sample peak, true peak,
   momentary/short-term/integrated LUFS with stated gating (BS.1770-4 absolute −70 / relative
   −10), loudness range (EBU Tech 3342) implemented, spectrum (existing FFT), correlation, mono
   audition (monitor mono). E07: generate the EBU R128 / BS.1770 conformance signals that are
   specified analytically (1 kHz sine at −23 dBFS → −23.0 LUFS ±0.1; the 1770-4 true-peak
   cases; the R128 gating sequences) in a test and compare — no external vector download; what
   cannot be generated is listed as PENDING, no "certified" claim.
5. **Export (R34/R35)**: `file.bounce_mix` gains `rate?` (SRC via `offline::resample`, quality
   stated), `metadata?` (BWF description), `report?: path` (JSON: settings, revision, plugin
   ids/versions/states hashes, measured peak/TP/LUFS/LRA, decoded-file verification: header,
   frames, rate, channels); refuses pending Bend renders? no — the document is always
   consistent (Sprint 3 decision), but the report lists any pending render; refuses faulted
   sandboxed processors and failed loads (exists); clipping/overwrite checks reviewable
   (`overwrite?: false` → refuse existing path).
6. **E08 (R35)**: portable: the full record→master scenario as an engine/UI test (recover/open,
   record through the capture path, comp/edit/nudge, buses + both mixer views, remote envelope,
   assistant explain + reversible action, master + reference, export + reopen) with fault
   injection (autosave during capture, scan in background, invalid theme, remote disconnect,
   cancelled render, model timeout); the durations (30-min W1 recordings, 2 h endurance, 8 h
   soak) and native crash rates are PENDING, never shortened-and-relabelled.
7. **Handoff (R36/R38)**: implementation-derived manual (USER_GUIDE already; add command and
   shortcut references generated from the registry, remote setup, skin guide, hosted-plugin
   limitations from the capability matrix, migration/recovery guide, measured performance
   report); local package via the existing release tooling where the environment allows
   (Linux artifact + install/reopen smoke in a clean temp prefix); final requirement matrix.
