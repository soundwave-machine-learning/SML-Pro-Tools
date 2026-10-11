# Autonomous build — final report (package revision 1.6)

Repository: soundwave-machine-learning/SML-Pro-Tools · branch `fable/recording-core` (local only;
not pushed, nothing merged) · base `main` = `7414719f3760de1c9359ec9723351a7232442e61` (equal to the
planning SHA). The commit containing this report is printed in the session output and recorded in
`docs/PROJECT_HANDOFF.md` after committing (a commit cannot contain its own SHA).

Execution host: Linux x86_64 cloud container, rustc 1.97.0, no audio devices, no display, no
Windows host, no owner plugins, no owner. Budget: 6 h wall-time proxy from 15:16:41Z (no usage
counter available); see "Budget" below.

## Verdicts
| Verdict | Status |
|---|---|
| PORTABLE IMPLEMENTATION | **PASS** for every executed A1–A4 software row; named rows remain **PARTIAL** (listed below) — so overall **PARTIAL** against the full feature matrix |
| TARGET VALIDATION | **BLOCKED** (no Windows machine / interface / driver / owner plugins) |
| SLOT INFRASTRUCTURE READY | **YES** (software: hosting, SL02 order, state, Pre/Post, bypass-with-latency, audition, persistence) |
| NATIVE SONIC MODELS READY | **NO — DEFERRED BY OWNER** |
| CLOCK/CAPTURE INTEGRITY | **PASS (portable)** — bit-exact bounded disk capture, recovery, stream clock, timestamp placement; physical capture **H1 pending** |
| NATIVE LATENCY TARGET (≤10 ms) | **NOT MEASURED** (no hardware); headless profile only |
| REFERENCE COMPARISON (Pro Tools / Logic / iZotope / LUNA) | **NOT PERFORMED** — no parity claims |
| READY FOR OWNER REVIEW | **YES** — as a reviewable candidate (source + unsigned, untested Windows cross-build + Linux tarball + gauntlet) |
| RELEASE READY | **NO** |

## Sprint statuses
| Sprint | Status | Commit(s) | Report |
|---|---|---|---|
| 1 — Recording, clock, clean baseline (A1) | **PASS (portable)**; H1 pending | `f70ac26` | `docs/sprint_reports/SPRINT_01_REPORT.md` |
| 2 — Vocal editing + Nudge (A2) | **PASS for executed rows**; V01/V03/V07/V13/V14 partial; N07 physical/N09 native, N10 claim-only | `c11b84f` | `SPRINT_02_REPORT.md` |
| 3 — Mixer, slots, own plugins, Visual (A3) | **PASS for executed rows** + two carry-over batches; H2/M-T7/SL-T8/VM-T7/8 pending; M-T3/4/8 deferred by owner | `4295a19`, `525d882`, `a130594`, `b1181dc` | `SPRINT_03_REPORT.md` |
| 4 — Performance, delivery (A4) | **A4 regression PASS**; native H1–H4 BLOCKED; artifacts built (unsigned, untested) | this commit | `SPRINT_04_REPORT.md` |

## Requirement coverage (R01–R26)
| ID | Status | Evidence / gap |
|---|---|---|
| R01 device/input, actual rate/buffer | IMPLEMENTED; native H1 pending | `device.rs` negotiation (fixtures), Playback Engine panel (requested/reported/effective) with output **and input** device pickers (input: all channels at session rate, no substitution; `input_request_*`, `failed_input_switch_*` tests) |
| R02 arm/monitor/transport, dry recording | IMPLEMENTED; H1 pending | dry tap before mixer; Recording panel; Hardware Monitor mode |
| R03 bounded disk capture/recovery | PASS (portable) | `capture-tests.log`, `failure-recovery.log` |
| R04 remove bundled catalogue | PASS | routes removed and implementations deleted (`plugin-removal-inventory.md`) |
| R05 preserve removed state/media | PASS | placeholders + export acknowledgment test |
| R06 own VST3/CLAP, editors/state/export | software PASS (repo CLAP fixture in insert and Tape slot); background scan; H2 owner plugins pending | `plugin-compatibility.csv` |
| R07 mixer/routing/compensation | PASS (software) | SL02 graph v2, PDC through slots (0–1024), cycles, mute/solo policy |
| R08 simple recording view, usable mixer | IMPLEMENTED; UX pending | Recording panel, Console SLOTS/Width rows (renders) |
| R09 measured responsiveness | PARTIAL | headless profile: 13 overruns @64 frames found, cause measured (rayon overhead on light strips), adaptive strip scheduling → 0 overruns @64/128/256 on the VM — native H3 pending |
| R10 regression, candidate, manual, delivery | regression PASS; candidates + manual done; native delivery checks pending | `regression.log`, `artifact-manifest.json`, `USER_GUIDE.md` |
| R11 vocal tool families V01–V14 | PARTIAL | `vocal-coverage.csv` (V02/04/05/06/08/09/10/11 verified in software; V12 needs owner plugin) |
| R12 dedicated Nudge | software PASS (N01–N06, N08, N07 unit); N07 physical/N09 native pending; N10 claim-only | Sprint 2 |
| R13 easy bus/FX return | PASS (software); B-T7 owner pending | `buses/bus-tests.log` |
| R14 full vocal walkthrough | software stages PASS; native/owner pending | `vocal-e2e.json`, `vocal-e2e-final.json` |
| R15 Logic-style clean profile | PASS (M-T1/M-T2); comparison claim-only | `slot-graph-tests.log` |
| R16 Summing slot | PASS (software) | |
| R17 slot integrity; models deferred | M-T1/2/5/6 PASS; M-T7 native pending; M-T3/4/8 DEFERRED BY OWNER | |
| R18 Console/Visual/Split one mixer | PASS (software); native interaction pending | renders, VM-T1/2/4/6 |
| R19 spatial gain/pan/width/list/snapshots | PASS (software); list rename, route badges and CLIP cue added | `visual-mixer-coverage.md`, `sprint_04/ui/visual-list-badges.png` |
| R20 automation/undo/persistence/shared routing | PASS (software): undo/persistence; stop/play automation arbitration (`vmt5_snapshot_recall_and_read_automation_arbitration`) | |
| R21 Track Tape / Master Tape with position | PASS (software) | |
| R22 provider lifecycle/PDC/state/category bypass/group schema | PASS (software); tape-group linking schema only (as specified) | |
| R23 spill, independent send pan, gain staging | PASS (software): spill, send pan, dedicated-slot I/O meters (`sml_graph_slot_io_meters_*`) | |
| R24 audio clock, negotiation, provenance | PASS (portable) | `clock-tests.log` |
| R25 monitoring modes, compensation, placement | modes/PDC/timestamp placement PASS (software); calibration CL-T6 pending | |
| R26 bounded realtime work, metrics | capture path fixed; render allocation-free in steady state (serial, test); replaced sessions freed off the audio thread; remaining findings (graph `sync` on structure change, unbounded command queue, state capture) documented; native metrics pending | `realtime-audit.md`, `realtime_alloc` test |

## Actual commands and results (all RERUN in this container)
- Baseline: `cargo xtask ci` failed on environment (missing ALSA headers; then no GPU adapter);
  after installing `libasound2-dev`, `mesa-vulkan-drivers`, wasm target: 541 tests passed.
- Milestone CI: Sprint 1 `ci-sprint1.log` (581 passed), Sprint 2 `ci-sprint2.log` (593),
  Sprint 3 `ci-sprint3.log` (611), carry-overs `ci-sprint3-carryover.log` (616),
  `ci-module-removal.log` (591 — 25 processor-only tests retired), `ci-carryover2.log` (594),
  CL05 optimization `ci` rerun, final regression `docs/evidence/sprint_04/regression.log`
  (**603 passed, 0 failed, exit 0**).
- Feature suites and commands: `docs/evidence/sprint_01/validation-command-index.md`.

## Tests changed or retired
See `docs/evidence/sprint_01/plugin-removal-inventory.md` (each with rationale): PDC/insert tests
ported to test-only providers; bundled-instrument test retired; plugin-state/category/preset tests
use placeholders; MCP agent test uses the bus workflow; 25 dsp processor tests retired with their
code; fuzz/unit/offline service tests replaced or ported. No tolerance was loosened and no
expected output regenerated. Two of my own new tests were corrected after investigation (a pan
reference that used −3.0103 dB where the legacy law is −3.0 dB; an offline-render expectation that
ignored latency pre-roll) — the product behaviour was right in both cases.

## Target / backend / plugin matrix
| Item | State |
|---|---|
| Windows x64 runtime | not available — BLOCKED |
| Audio backend | Linux ALSA compiled (no devices in container); Windows build: WASAPI only; ASIO not compiled (SDK needed) |
| Owner plugins | none supplied — H2 pending |
| Repo CLAP fixture | PASS in insert and Tape slot |

## Measured latency / performance
Physical latency: **not measured**. Headless release profile on the shared VM (not native):
the first run found **13 deadline overruns at 64 frames** (p99 load 0.217); measurement showed
rayon scheduling costing 3–5× the work of light strips. After the adaptive rule (workers only for
≥4 strips with hosted processing; reused work buffer; bit-identical output), `callback-profile.csv`
shows p99 load 0.042 / 0.029 / 0.026 at 64 / 128 / 256 frames and **0 overruns** in the adaptive
rows (forced-serial rows: 2 overruns at 64 frames from VM pauses). Render is allocation-free in
steady state (serial; test). Fader command ack p95 19.9 µs (engine only). Not evidence of native
performance; owner plugins (heavy strips) are not represented.

## Owner review
None performed (owner not available). Nothing approved on the owner's behalf.

## Artifacts (local, gitignored)
| Path | sha256 |
|---|---|
| `dist/release/soundcraft-0.5.0-linux-x86_64.tar.gz` | `7d95b1f7b248440c1c18c0cff13c939638539b4f63421fb3f7e267011f38db0d` |
| `dist/release/sml-recording-0.5.0-windows-x64-gnu-unsigned.zip` | `cebcb166cd519b8b45eeef6ed1ae55912972c8b749eac942be47addaa6844b81` |
Linux CLI smoke PASS; Windows build never executed. Details: `artifact-manifest.json`.

## Blockers (exact)
1. No Windows x64 machine, audio interface, driver or owner plugins → H1–H4, CL-T6/7, N07
   physical, N09, VM-T7/8, SL-T8, M-T7, B-T7 cannot run.
2. ASIO: cpal `asio` feature needs the Steinberg ASIO SDK + LLVM on a Windows build host.
3. Native MSVC packaging (`packaging/windows/package.ps1`) needs PowerShell/MSVC on Windows; signing
   is outside authorization.
4. egui-winit does not distinguish keypad from main-row +/- and exposes no Win modifier (N07 gap).
5. craftrules (`../../craftrules`) and `plan/` are not present in the container; not inferred.

## Budget
Start 15:16:41Z. Sprints 1–3 + carry-overs + Sprint 4 deliverables completed by ≈17:30Z; work
continued afterwards within the cap (see `docs/PROJECT_HANDOFF.md` for the final timestamp).
Final 20 minutes reserved for verification/checkpoint as required.

## Exact next action
On a Windows x64 machine with the owner's interface: extract
`sml-recording-0.5.0-windows-x64-gnu-unsigned.zip` (or build natively with
`packaging/windows/package.ps1` from branch `fable/recording-core`), then execute
`docs/TARGET_MACHINE_GAUNTLET.md` §1 (H4 smoke) → §2 (H1) → §3 (CL-T6) → §4 (CL-T7) → §5 (H2 /
owner walkthrough), recording real evidence. To publish the branch, the owner must push it
(`git push -u origin fable/recording-core`) — pushing was outside this run's authorization.
