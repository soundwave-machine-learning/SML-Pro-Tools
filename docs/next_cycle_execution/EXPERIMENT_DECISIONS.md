# Experiment decisions (E01–E08) — real entries only

Entries are appended as experiments actually run. "Decision" rows cite the evidence file. Nothing
here is prepopulated as PASS.

## E01 — What exists and how it is built (G0)
- Checkpoint verified 2026-10-11T23:35Z: branch `fable/recording-core`, HEAD
  `396fd0a7a7aa5b62d8dae0e49b3413c7e2e3432b`, clean tree, `main` at `7414719…` untouched, remote
  `origin` = github.com/soundwave-machine-learning/SML-Pro-Tools (branch never pushed).
- Artifact source `7838032` → HEAD diff: 10 files, all under `docs/` (reports, logs, handoff). No
  source change after the artifact build (verified with `git diff --name-only 7838032..396fd0a`).
- Commands: see COMMAND_MATRIX.md. Baseline rerun 2026-10-11T23:37–23:42Z: `CARGO_INCREMENTAL=0 cargo xtask ci` exit 0 — fmt, clippy, test (616 passed, 0 failed, 57 test binaries), assets, layers, wasm; 4m33s wall (`docs/evidence/next_cycle/sprint_01/baseline-ci.log`). This is RERUN evidence; the reported 616 is confirmed by the current count.
- Environment: `docs/evidence/next_cycle/sprint_01/environment.json` (Linux container, no audio
  device, no display, no Windows host, no owner plugins → P only).

## B01 — structural edits on the audio thread (Sprint 1) — CLOSED (portable)
- Finding (inspected): `MixEngine::sync` ran inside `render` on the audio thread and allocated on
  every structural change — new `Strip` buffers, bus accumulators, meter entries, the processing
  order vector, plugin slot vectors, `HashMap` growth — and freed removed strips/slots there too.
- Decision: full off-thread preparation (not the stopped-only interim). `GraphPrep::build`
  (UI thread) allocates maps with reserved capacity, a fresh strip/bus/meter per track/bus, the
  order, the work list and the plugin instances (`prepare_instances`, which now also covers test
  providers). `MixEngine::apply_prepared` (audio thread) moves matching strips/buses/meters across,
  takes fresh ones, places instances, and returns the bundle with every leftover (old maps,
  orphans, retired whole slots, unplaced instances) for the UI thread to drop. Delay-compensation
  rings are pre-sized (`PDC_RESERVE` = 4096 samples) so a latency change within the reserve does
  not allocate. Anything still allocated in the callback is counted (`prepared_fallbacks`) and
  logged once.
- Playback: `Cmd::Session` carries the prepared bundle; a sample-rate change ships a fresh
  engine prepared off-thread (the old one is dropped by the UI thread).
- Evidence: `cargo test -p soundcraft-mix --features test-providers --test structural_alloc`
  (stats_alloc guard): add track, add bus+aux+route, add send, add insert, add Tape-slot provider,
  remove insert, remove track, delete bus → **0 allocations / 0 reallocations / 0 deallocations /
  0 fallbacks** on the calling thread for the apply and the four following blocks, with output
  bit-identical to the offline (allocating) engine. Log: `docs/evidence/next_cycle/sprint_01/ci-b01-b04.log`.
- Residual (documented, not closed): clip-effect chains (`Strip::clip_fx`) are created on first
  use of a clip's EQ/compressor parameters on the audio thread; a compensation latency above
  `PDC_RESERVE` reallocates rings (counted); plugin-state capture (`CaptureStates`) still runs
  between blocks on the audio thread (see E02: deferred to stopped transport).

## B04 — non-functional recording toggles — CLOSED
- QuickPunch, TrackPunch, Destructive Record, DestructivePunch: commands stay registered (ids,
  legacy session flags untouched) but are disabled with the reason "not implemented in this
  build…" through `CommandSpec::enabled`; menus show the reason on hover; executing them is an
  error, never a silent setting change. Test: `record_mode_tests::unimplemented_recording_modes_are_disabled_not_silent`.

## E02 — project autosave and media recovery (Sprint 1) — portable PASS, native/power-loss PENDING
- Inspected: the previous autosave was a 120 s `save_copy` (no fsync, no validation, single
  file overwritten in place) and its recovery helper `latest_autosave` had **no caller**: autosaves
  were written but never offered. Capture journals existed (`capture::recover`) but nothing
  discovered unfinished takes.
- Decision: `soundcraft_engine::autosave::Store` — rolling generations `gen-NNNNNN.scraft`
  (write tmp → `sync_all` → re-read, parse + FNV-1a checksum → rename → dir sync → index tmp →
  `sync_all` → rename), retention 20 / 256 MiB budget never removing the newest; recovery picks the
  newest validated generation from the index, or a newer validated file the index does not name
  yet. Unsaved recorded sources are written once per source id under `media/`. UI: 60 s worker
  thread, plugin-state capture only while stopped with the age recorded, `File › Recover
  Autosave…` (generations + unfinished takes via `capture::discover_unfinished_takes`), a
  `running.lock` marker that reports an unclean previous exit.
- Fault model tested (P): disk-full on the temporary write; kill before rename; kill before the
  index write; kill before the index rename; truncated newest generation; retention/budget;
  media reuse; unfinished take killed between durable checkpoints. Tests:
  `cargo test -p soundcraft-engine --lib autosave`, `cargo test -p soundcraft-playback --lib unfinished_takes`,
  `cargo test -p soundcraft-ui-egui --lib autosave_worker`.
- Not tested: OS/power failure (needs a native machine with a power cut), measured committed/tail
  loss on real storage (capture durable interval is 48 000 frames ≈ 1 s at 48 kHz — matches the
  proposed ≤1 s target but is not measured natively), concurrent writers across processes.

## R07 — isolated plugin scanning (Sprint 1) — PASS_PORTABLE (scan isolation only)
- Inspected: the previous scanner was a worker *thread* calling the hosts' in-process `scan()`,
  which `dlopen`s every bundle (`crates/vst3-host/src/ffi.rs:245`, CLAP likewise): a plugin that
  crashes or hangs on load took the app down or froze the scan. No cache, no quarantine.
- Decision: `soundcraft_mix::scan_isolated` — one worker **process** per bundle
  (`soundcraft --scan-plugin <path>` / `soundcraft-cli scan-plugin`), JSON descriptors over stdout,
  20 s timeout with kill, output readers never joined on a hung worker, 4 MiB output bound;
  outcomes: Ok / Crashed(signal) / TimedOut / Failed(exit code + stderr) / BadOutput; fingerprint
  cache (path, size, mtime) in `<settings>/plugin-scan-cache.json` written atomically; quarantine
  entries persist until re-enabled; rescan reuses the cache. The host registries adopt the
  descriptors (`adopt_scanned`) without loading anything. Playback instantiation remains
  in-process (documented compatibility mode; runtime containment is E03).
- Evidence (P): `cargo test -p soundcraft-mix --lib scan_isolated` — a shell worker modelling
  crash (SIGSEGV), hang (killed at the timeout; whole scan < 5 s), exit-code failure and garbage
  output: host survives, each bundle quarantined with its reason, good bundle listed; second scan
  spawns no worker (5/5 cached); changed bundle and re-enabled quarantine are rescanned (2);
  forced rescan keeps quarantines (1). UI: `plugin_scan` tests; the insert menu shows the
  quarantine list with Re-enable and Rescan.
- Not tested: real CLAP/VST3 bundles through the worker on the target machine (no plugins in
  this container; the repo CLAP fixture is exercised in-process by `clap_e2e`).

## R05 — Safe Open (Sprint 1) — PASS_PORTABLE (software)
- `soundcraft --safe`: `soundcraft_mix::set_hosted_plugins_disabled(true)` makes every hosted id
  create no instance (the existing missing-plugin placeholder path: audio passes through, state
  and ids kept in the session), and the scanner does not start; `--reset-prefs` renames `ui.json`
  to a timestamped backup (restoration = rename back; logged). The unclean-exit notice suggests
  `--safe`. Test: `safe_open_tests::safe_open_disables_hosted_plugins_only`.
- Crash reports/logs: the existing logger writes `<settings>/logs/soundcraft.log` and the engine
  panic hook restores the document (`Engine::execute`); redaction of private paths in logs is
  NOT implemented (recorded as a gap).

## R09 — shared command contracts (Sprint 1) — PASS_PORTABLE (engine); client adoption follows
- Inspected: `Engine::execute(id, params)` already validates `enabled`, rolls back on error or
  panic, and groups gestures (`execute_merged`); nothing carried a request id, an expected
  revision or a receipt, so a remote retry could apply twice and a stale client could overwrite.
- Decision: `soundcraft_engine::envelope` — `Envelope { request_id, client, command, params,
  expected_revision, gesture }` → `Engine::submit` → `Receipt { accepted, duplicate, applied,
  rejection, revision }`; bounded dedup window (4096 ids); stale revision rejected before any
  mutation; errors mapped to `Rejection::{UnknownCommand, Disabled, BadParams, Failed}`. Knowledge
  side-table: `risk_of` (View / Transport / Reversible / Destructive by id prefix + undoability),
  `Status::{Implemented, DisabledNotImplemented}`, `knowledge_json` (build-versioned corpus for
  help, remote and assistant). Parameter *IDs* for automation/plugins are the existing stable
  `AutoParam`/plugin param ids (unchanged).
- Evidence: `cargo test -p soundcraft-engine --lib envelope` (duplicate → same receipt, no
  second mutation or undo step; stale → rejected; deleted target → document pointer unchanged;
  unknown; disabled B04 mode; gesture = one undo step with the final value).
- Not done yet: the control channel, remote and assistant (Sprints 2/4) must *use* `submit`
  (G2 check); the GUI keeps `execute` (same validation path).

## E05 — Bend renderer / warp dependency (Sprint 1 feasibility) — DECIDED
- Inspected: `crates/dsp/src/offline.rs::time_stretch` — an in-house WSOLA pitch-preserving time
  stretch (own code, MIT OR Apache-2.0 like the repository; no third-party DSP), already used by
  the clip `stretch` field (`crates/model/src/clip.rs`) and verified in the prior cycle's V11 test
  (duration within 1 %, 440 Hz pitch within 2 %). `offline::pitch_shift` builds on it.
- Decision: Sprint 2 Bend binds to this renderer through a native edit map (anchors →
  piecewise ratios) rendered off-thread into a clip render revision; no external warp
  dependency is added (none is needed for a legal baseline; quality limits are those of WSOLA and
  are stated in the user guide). Sprint 3 hardens the render queue/cancel/stale rules.
- External plugin interfaces (Tune / Vocal Editor / Align): no artifacts supplied —
  AWAITING_EXTERNAL_PLUGIN; the host exposes VST3/CLAP inserts only; clip-aware exchange is not
  assumed.

## E06 — assistant model / STT / TTS (Sprint 1 feasibility) — BLOCKED for generative parts
- Inspected (this container): no local speech or language runtime (`whisper`, `ollama`,
  `piper`, `espeak` absent); no owner-provided provider credential for the *product*. The
  coding session's own API proxy variables belong to this agent's harness, not to the product,
  and are out of scope by the launcher's credential rule.
- Decision: Sprint 4 implements the provider-agnostic assistant service (Explain from the
  build-versioned knowledge corpus of R09; Operate through validated `Envelope` submissions;
  Engineer through deterministic measured rules) with a pluggable model/STT/TTS boundary and an
  explicit `Unavailable` state; conversational generation and speech remain BLOCKED until the
  owner configures a provider or a local model (recorded, not faked). Text input works without a
  model (rules + search); push-to-talk UI can be wired with the STT boundary returning
  "unavailable".

## B02 — 64-frame overruns (Sprint 1 measurement, environment P) — NOT REPRODUCIBLE AS COMPUTE; native pending

Command (run alone, CPU otherwise idle, release build, commit 7bffb39):
`./target/release/examples/sml_profile docs/evidence/next_cycle/sprint_01/callback-profile-control.csv docs/evidence/next_cycle/sprint_01/response-profile.csv`
(log: `profile-control-run.log`; the first attempt started with residual build load and was discarded).

Control = a paced no-work loop (spin to the next period; lateness = scheduler preemption only;
a miss is lateness ≥ the period). Workload = 16 play + 2 rec + bus + fx, graph v2, per-block
structure check (the realtime callback's work), measured as compute time per callback.

| frames @48 kHz | budget | control p99 / max lateness | control misses | workload p99 / max compute | workload misses |
|---|---|---|---|---|---|
| 64 | 1333 µs | 105 µs / 7.06 ms | 98 / 45 000 | 57–67 µs (4.3–5.0 % of budget) / 3.6 ms | 3–4 |
| 128 | 2667 µs | 125 µs / 6.17 ms | 14 / 22 500 | 79–92 µs (3.0–3.5 %) / 1.0–4.8 ms | 0–1 |
| 256 | 5333 µs | 57 µs / 5.41 ms | 1 / 11 250 | 112–139 µs (2.1–2.6 %) / 0.6–3.5 ms | 0 |

Reading: with zero work the container's scheduler alone preempts the thread for several
milliseconds often enough to miss ~0.2 % of 64-frame periods; the workload's p99 compute is
≤ 5 % of the budget and its few misses are millisecond-scale preemptions of the same shape, not
compute. B02's "64-frame overruns" therefore cannot be attributed to the mixer in environment P;
they are consistent with scheduler preemption on a non-realtime thread. What remains native:
the owner machine's device callback at 64 frames with a realtime-priority audio thread (N),
which this environment cannot provide. No product change is justified by P data; the Playback
Engine's buffer selection and the xrun counter stay the user's tools. Status: B02 OPEN (native).

## E03 — runtime process isolation of hosted plugins (Sprint 1 experiment) — DECIDED: feasible for containment, not for the default callback path

Command (run alone, release build, rebuilt `soundcraft-cli` with the `plugin-worker-probe`
worker; the first two attempts used a stale CLI binary and are discarded):
`./target/release/examples/ipc_probe docs/evidence/next_cycle/sprint_01/ipc-probe.csv`
(log: `ipc-probe-run.log`). Model: one worker process per plugin, one blocking pipe round trip
per block (write N f32, read N f32), crash = SIGABRT after 100 blocks, hang = silent after 100
blocks, host policy = silence and continue.

| frames | round trip p50 / p99 / max | over budget (of blocks) | p99 added latency |
|---|---|---|---|
| 64 | 41.9 / 121.6 µs / 3.20 ms | 11 / 45 000 | 5.8 frames |
| 128 | 41.1 / 126.5 µs / 7.16 ms | 2 / 22 500 | 6.1 frames |
| 256 | 38.8 / 87.7 µs / 1.29 ms | 0 / 11 250 | 4.2 frames |
| crash | detected in 6.8 ms, host continued | | |
| hang | detected in 16.2 ms (deadline), host continued | | |

Decision: process isolation contains a plugin crash or hang (detection within one or two
periods, host keeps running, silence policy) but costs a blocking syscall pair in the callback
with a p99 of ~0.12 ms per plugin per block (9 % of a 64-frame budget per plugin) and tail
latencies of several milliseconds from scheduling, which alone exceed the 64- and 128-frame
budgets. Running the worker one block ahead would hide the tail at the price of one block of
added latency per isolated plugin. Therefore: (1) the default host keeps in-process plugins with
the isolated *scanner* (R07) as the crash boundary for discovery; (2) an opt-in "sandboxed"
instance mode (worker process, one-block-ahead pipeline, declared added latency, silence on
fault, automatic restart) is the Sprint 3 R31 design, labelled as compatibility mode, never a
default; (3) no claim of runtime crash containment is made for this build. Status: R07 runtime
part DEFERRED to Sprint 3 with these numbers as the admission criteria.
