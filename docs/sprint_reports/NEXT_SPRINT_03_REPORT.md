# Next Sprint 03 — Harden plugin integration (revision 2.2 contract)

Status: **PARTIAL** — generic hosting, sidechain routing, compensation fixtures, offline-render
guards, version records, the opt-in sandboxed instance mode (E03 follow-up) and Bend reopen
hardening are implemented portably with tests (full `cargo xtask ci` exit 0, see "Tests run");
the seven SML plugins are AWAITING_EXTERNAL_PLUGIN (no artifact supplied); native evidence
(real personal plugins on the target machine, editor workflow, W1/W2 callback timing and xruns,
input/sidechain hardware routing) is TARGET_PENDING. Environment P only
(`docs/evidence/next_cycle/sprint_01/environment.json`). Started 2026-10-12T04:40Z from commit
`0baa780`.

## Outcome (contract items 1–9)
| Item | Result | Evidence |
|---|---|---|
| 1 Capability matrix | `engine.plugin_capabilities` reads each discovered CLAP/VST3 plugin from a live instance (ports/buses, aux inputs and how the host treats them, note input, parameter count/units, editor on this platform, state, latency, tail, host notes); AU listed as not probed; deterministic fixtures and the seven external targets labelled as such | `host-capability-matrix.csv`, `capability-matrix-tests.log` |
| 2 Load/editor/focus/close/unload/reload defects | no new defect found in the fixture suites (CLAP floating GUI stub, VST3 view probe, many-instance create/drop, module exit tests rerun in CI); native editor behaviour pending | `ci-sprint3.log` |
| 3 Automation/IDs/units/state/save-reopen/missing/version | parameter ids and units come from the live instance (matrix); state round trips (in-process and sandboxed); `Insert.plugin_version` recorded at insert, `session.plugin_versions` reports same/changed/unrecorded/missing; missing-plugin placeholders unchanged | `render-versions-bend-tests.log` (`plugin_versions_are_recorded_and_compared`) |
| 4 Sidechain, multi-input, latency, parallel/nested PDC, bypass | `Insert.sidechain` (bus) → `Plugin::process_sidechain`; CLAP aux port fed; aux strips sample-accurate, audio tracks one block late (reported); fixtures 0/1/63/128/1024 samples parallel, nested and bypassed align within 1 sample; ordinary insert bypass now keeps its latency as a plain delay; VST3 aux buses stay inactive (recorded) | `sidechain-pdc-tests.log` |
| 5 Offline rendering, tails, errors, determinism | `render_range_report` with `Tail::{None,Samples,Auto}`; `file.bounce_mix/stems` gained `tail`; a hosted plugin that fails to load makes the export refuse (named) unless `acknowledge_missing: true`; sandboxed renders equal in-process renders after PDC (1e-5) | `render-versions-bend-tests.log`, `sandbox-tests.log` |
| 6 Runtime isolation (E03) | `sandbox:<id>` opt-in mode: worker process, one block ahead, bounded in-flight, late blocks counted and silent, crash/hang faults logged, auto-restart with last state (off while recording), status/RSS query; measured | `sandbox-e03.csv`, `sandbox-tests.log`, EXPERIMENT_DECISIONS E03 follow-up |
| 7 Bend hardening | warped clips whose derived media is missing on reopen are re-rendered from the original; queue/stale/undo/immutable-media behaviour rerun | `render-versions-bend-tests.log` (`warped_clip_is_rerendered…`) |
| 8 SML plugin artifacts | none supplied → seven AWAITING_EXTERNAL_PLUGIN rows with the host work each will need; R28 recorded as "integration mechanism undetermined" | `host-capability-matrix.csv` |
| 9 W1 load with a real chain, autosave, state capture | portable control: 16-track CLAP fixture chain (two sandboxed) rendered block by block under autosave snapshots and periodic state captures; native run pending | `w1-hosted-load.csv`, `w1-hosted-load.log` |

## Requirement statuses
See `docs/next_cycle_execution/REQUIREMENT_STATUS.csv`. R23–R29 AWAITING_EXTERNAL_PLUGIN (host
work PASS_PORTABLE on fixtures); R30 PASS_PORTABLE; R31 PARTIAL (portable evidence complete,
native pending).

## Measurements
- E03 follow-up, sandboxed instance mode (`sandbox-e03.csv`, debug build in P, the test
  binary as its own worker, paced like a callback): exchange per block p50 / p99 / max —
  64 frames 23 / 97 / 833 µs (3 late of 2000); 128: 26 / 69 / 1445 µs (1 late); 256: 31 / 64 /
  440 µs (0 late of 800); 1024: 72 / 500 / 3003 µs (0 late); worker RSS ≈ 5.5 MiB; crash →
  audio back with the re-applied parameter in 61 ms after auto-restart; hang declared in
  1.05 s (silent, never a stall > half a block). Output parity with in-process hosting within
  1e-5 after PDC. Release/native numbers remain the E03 reference and the pending gate.
- W1 hosted chain (`w1-hosted-load.csv`, headless debug build): 16 tracks / 2000 clips, the
  CLAP fixture on every track with two instances sandboxed, 3750 blocks of 256 at 48 kHz under
  9 autosave snapshots and 10 state captures: render p50 277 / p95 466 / p99 1323 / max 8399 µs
  against a 5333 µs budget, 8 blocks over budget, 0 sandbox faults, state capture max 0.27 ms.
  A debug-build control, not the native W1 target (callback, GUI, device absent).

## Files / commits
Milestone commit `feat: harden SML Studio plugin hosting and integration` (`f176366`).
New: `crates/mix/src/sandbox.rs`, `crates/engine/src/cmd/host.rs`, tests
`crates/engine/tests/{sandbox.rs (harness-less), w1_hosted_load.rs}`, `SPRINT_03_DESIGN.md`,
evidence under `docs/evidence/next_cycle/sprint_03/`. Changed: dsp (`HostCapabilities`,
`Plugin::accepts_sidechain/process_sidechain`), model (`Insert.sidechain`,
`Insert.plugin_version`; serde defaults, ids untouched), mix (sidechain tap and bus history,
bypass delay per slot, `render_range_report`/`Tail`, instantiate-failure report, `test:sidegate`,
`test:tail`, `hosted_id`), clap-host (aux port feeding, `capabilities`, `instantiate_plugin`,
`extra_search_dirs`, fixture `sidegate` plugin), vst3-host (`capabilities`, bus layout record,
`extra_search_dirs`), engine (`mix.insert_sidechain`, `engine.plugin_capabilities`,
`engine.host_faults`, `session.plugin_versions`, bounce guard + tail, Bend re-render on open,
sandbox id handling), playback (auto-restart off while recording), app/CLI worker entry points,
USER_GUIDE §10, COMMAND_MATRIX.

## Tests run (all P)
Full `CARGO_INCREMENTAL=0 cargo xtask ci` exit 0: fmt, clippy (-D warnings), 673 tests passed /
0 failed, assets, layers, wasm (log `ci-sprint3.log`). Three earlier gate runs failed and were
fixed before this one: a test with a wrong anchor count, the B01 allocation guard (bypass rings
are now reserved off the audio thread in the prepared graph), and the sandbox parity test under
full-CI CPU load (offline renders now wait for the worker instead of substituting silence).
Suites per row above with their logs in the evidence folder.
Fault/edge models actually exercised: missing bus / empty slot for the sidechain command; gate
without a sidechain (closed); one-block lag on tracks; nested and bypassed delays at 0/1/63/128/
1024; render tail cut vs auto vs fixed; a plugin that passes the missing check but fails to load
(refused export, acknowledged export); unknown `tail` value; version changed/unrecorded/missing;
worker crash (abort) and hang (silent) with containment, recording hold, restart; worker reply
limits (frames/channels/state size); late worker replies under CPU starvation (counted, not a
hang; offline renders wait instead of substituting silence); derived media deleted before
reopen; invalid sandbox ids; B01 structural-allocation guard with the per-slot bypass rings.

## Not run / failed / blocked / gaps (named)
- AWAITING_EXTERNAL_PLUGIN: Channel EQ, Gate/Expander, Compressor, Optical Leveler, Tune, Vocal
  Editor, Align — no binary or interface supplied; nothing was built in their place.
- Native (TARGET_PENDING): real personal plugins through the isolated scanner and the matrix;
  native editor open/focus/close on Windows/macOS; W1/W2 callback timing, headroom and xruns
  with the chain, editors and menus; hardware input/sidechain routing; AU probing.
- VST3 aux (sidechain) buses are not activated; 64-bit processing and IMessage not hosted; CLAP
  tail extension not hosted (tail 0) — all recorded in the matrix rows, not hidden.
- Sandboxed mode: no native editor; metadata read in-process once; first blocks after creation
  can be late (instances are created off the audio thread before adoption, so this lands before
  playback); numbers are from a debug build in a container (E03 release numbers remain the
  admission reference).
- Sidechain on audio/instrument tracks lags one block by design (reported); aux/master exact.
- No test was weakened. Pre-existing behaviour changed deliberately: bypassing an insert now
  keeps its latency (compensation preserved), and `processor_latencies` counts bypassed inserts.

## Risks
- `Insert` gained two optional fields (serde defaults, skipped when absent): older builds
  ignore them; a session saved by this build loads in older builds without the sidechain route.
- The sandbox worker is the app binary itself (`--plugin-worker`); a packaged build must ship
  it as such (the CLI has the same entry point).
- Auto-restart hands a faulted plugin its last captured state: the state may predate the fault
  by up to one autosave checkpoint; the fault log says when.

## Next safe task
Sprint 4 (assisted studio) per `SPRINT_04.md`, keeping `CARGO_INCREMENTAL=0 cargo xtask ci` at
exit 0; assistant recipes may use only registered real plugin capabilities
(`engine.plugin_capabilities`).
