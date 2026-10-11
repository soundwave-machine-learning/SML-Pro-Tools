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
| 2 — Vocal editing + Nudge (A2) | **PASS for executed rows**; V01/V03/V07/V13 closed in the Sprint 4 follow-up (incl. a working Count Off); V14 owner review, V12 owner plugin; N07 physical/N09 native, N10 claim-only | `c11b84f` | `SPRINT_02_REPORT.md` |
| 3 — Mixer, slots, own plugins, Visual (A3) | **PASS for executed rows** + two carry-over batches; H2/M-T7/SL-T8/VM-T7/8 pending; M-T3/4/8 deferred by owner | `4295a19`, `525d882`, `a130594`, `b1181dc` | `SPRINT_03_REPORT.md` |
| 4 — Performance, delivery (A4) | **A4 regression PASS**; native H1–H4 BLOCKED; artifacts rebuilt from `7838032` (unsigned, untested on Windows) | `d41d690`; follow-ups `903992d` `6859781` `1e04af1` `cefa07b` `3eaef10` `b4db1a9` `c684ece` `4b87c16` `6c45d06` `44f0c70` `f440c87` `17ecf26` `b183c02` `81c5f54` `1e3f16d` `03e1911` `7838032` + final docs commit | `SPRINT_04_REPORT.md` |

## Requirement coverage (R01–R26)
| ID | Status | Evidence / gap |
|---|---|---|
| R01 device/input, actual rate/buffer | IMPLEMENTED; native H1 pending | `device.rs` negotiation (fixtures), Playback Engine panel (requested/reported/effective) with output **and input** device pickers (input: all channels at session rate, no substitution; `input_request_*`, `failed_input_switch_*` tests) |
| R02 arm/monitor/transport, dry recording | IMPLEMENTED; H1 pending | dry tap before mixer; Recording panel; Hardware Monitor mode; armed-track input channels checked against the device before recording (no substituted input) |
| R03 bounded disk capture/recovery | PASS (portable) | `capture-tests.log`, `failure-recovery.log`; simulated input disconnect → explicit `DeviceLost` partial (Sprint 4 follow-up); physical unplug H1 |
| R04 remove bundled catalogue | PASS | routes removed and implementations deleted (`plugin-removal-inventory.md`) |
| R05 preserve removed state/media | PASS | placeholders + export acknowledgment test |
| R06 own VST3/CLAP, editors/state/export | software PASS (repo CLAP fixture in insert and Tape slot); background scan; H2 owner plugins pending | `plugin-compatibility.csv` |
| R07 mixer/routing/compensation | PASS (software) | SL02 graph v2, PDC through slots (0–1024), cycles, mute/solo policy |
| R08 simple recording view, usable mixer | IMPLEMENTED; UX pending | Recording panel, Console SLOTS/Width rows (renders) |
| R09 measured responsiveness | PARTIAL | headless profile: rayon overhead (measured) removed and per-block structure hash (measured) skipped in playback; p99 load ≤0.05; 64-frame deadline misses remain at VM-noise level (11–18 / 270 000 over six runs) — native H3 decides |
| R10 regression, candidate, manual, delivery | regression PASS; candidates + manual done; native delivery checks pending | `regression.log`, `artifact-manifest.json`, `USER_GUIDE.md` |
| R11 vocal tool families V01–V14 | PARTIAL | `vocal-coverage.csv`: V01–V11 and V13 verified in software; V12 BLOCKED (owner pitch plugin); V14 interaction needs owner UI review |
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
  (**616 passed, 0 failed, exit 0**).
- Feature suites and commands: `docs/evidence/sprint_01/validation-command-index.md`.

## Tests changed or retired
See `docs/evidence/sprint_01/plugin-removal-inventory.md` (each with rationale): PDC/insert tests
ported to test-only providers; bundled-instrument test retired; plugin-state/category/preset tests
use placeholders; MCP agent test uses the bus workflow; 25 dsp processor tests retired with their
code; fuzz/unit/offline service tests replaced or ported. No tolerance was loosened and no
expected output regenerated. My own new tests were corrected where they were wrong, never the
product to fit them: in Sprint 4 a swapped `Clip::audio` argument in the Tab test and an
underflow in the click-placement test (beat 0 at sample 0). Earlier, two tests were corrected after investigation (a pan
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
- First run: **13 deadline misses / 45 000 callbacks at 64 frames** (p99 load 0.217). Measured
  cause: rayon scheduling cost 3–5× the work of light strips. Fixed with adaptive strips
  (workers only for ≥4 strips that host processing; reused work buffer; bit-identical output).
- Per-block graph-structure hash measured at 3.1 µs (≈12 % of the 64-frame median); playback now
  announces session changes instead. 64-frame median 26.1–30.1 → 23.0–23.9 µs over six runs.
- p99 load now 0.03–0.05 at 64/128/256 frames. **Deadline misses at 64 frames remain** in
  every config (11–18 per 270 000 callbacks over six runs, maxima 1.5–2.7 ms vs p99 ≈ 50–65 µs).
  This is consistent with VM scheduler pauses, but zero misses cannot be shown here, so the
  64-frame finding stays **open for the native run**. 256 frames: 0 misses.
- Render is allocation-free in steady state (serial; test). Fader command ack p95 25.1 µs
  (engine only).
Not evidence of native performance; owner plugins (heavy strips → workers) are not represented.

## Owner review
None performed (owner not available). Nothing approved on the owner's behalf.

## Artifacts (local, gitignored)
| Path | sha256 |
|---|---|
| `dist/release/soundcraft-0.5.0-linux-x86_64.tar.gz` | `b3bff56adb04fb354503c0b756cc9cd12f4fcacb6b05eaf64720f84cd64637f1` |
| `dist/release/sml-recording-0.5.0-windows-x64-gnu-unsigned.zip` | `3bff44b1ecf1fb41f6888519f6cb1befc5cce1991ddbd70256e54502dcfe2b56` |

Both built from source commit `7838032` (clean tree; later commits are docs only). Linux CLI smoke
PASS from clean extraction; Windows build never executed. Details: `artifact-manifest.json`.
`dist/` is gitignored: the files existed in the ephemeral build container and were attached to
the session output; rebuild commands are in the validation command index.

## Unfinished requirements (exact)
Software, not done in this run:
- **CL05 realtime residue** (`realtime-audit.md`): graph `sync` still allocates on the audio
  thread when the session *structure* changes (add track/insert/route); the command queue is
  unbounded; plugin state capture runs between blocks on the audio thread.
- **64-frame deadline misses** in the headless profile: 11–18 per 270 000 callbacks remain
  (consistent with VM pauses, unproven).
- **N07**: the UI toolkit does not distinguish the numeric keypad and has no Win modifier.
- **Upstream Options toggles without behaviour**: QuickPunch, TrackPunch, Destructive Record and
  DestructivePunch only store a setting (outside this package's scope; not claimed). Count Off
  had the same problem and was fixed.
- **UI screen/scale matrix**: offscreen renders at 1600×1000 @2x and 1366×768 @150 % only; the
  latter found clipped faders/toolbar groups (fixed by scrolling, not by a redesign). Native
  check pending.

Needs the owner or the target machine (not executable here): H1–H4, CL-T6/7, N07 physical,
N09, N10, VM-T7/8, SL-T8, M-T7, B-T7, V12 (owner pitch plugin), V14 owner UI review, owner
listening/approval. Deferred by the owner: native summing/tape models (M-T3/4/8).

## Blockers (exact)
1. No Windows x64 machine, audio interface, driver or owner plugins → H1–H4, CL-T6/7, N07
   physical, N09, VM-T7/8, SL-T8, M-T7, B-T7 cannot run.
2. ASIO: cpal `asio` feature needs the Steinberg ASIO SDK + LLVM on a Windows build host.
3. Native MSVC packaging (`packaging/windows/package.ps1`) needs PowerShell/MSVC on Windows; signing
   is outside authorization.
4. egui-winit does not distinguish keypad from main-row +/- and exposes no Win modifier (N07 gap).
5. craftrules (`../../craftrules`) and `plan/` are not present in the container; not inferred.

## Budget
Start 15:16:41Z (wall-time proxy; no usage counter available). Sprints 1–3 and carry-overs were
done by ≈17:00Z, and Sprint 4 deliverables by ≈17:30Z. Follow-up batches (CL05 optimizations,
carried PARTIAL rows, review-found H1 risks, Count Off, small-screen layout) ran until ≈19:40Z.
The final verification/checkpoint (artifact rebuild, regression, reports) ran from ≈19:40Z,
ahead of the reserved final 20 minutes (20:56–21:16Z).

## Exact next action
On a Windows x64 machine with the owner's interface: extract
`sml-recording-0.5.0-windows-x64-gnu-unsigned.zip` (or build natively with
`packaging/windows/package.ps1` from branch `fable/recording-core`), then execute
`docs/TARGET_MACHINE_GAUNTLET.md` §1 (H4 smoke) → §2 (H1) → §3 (CL-T6) → §4 (CL-T7) → §5 (H2 /
owner walkthrough), recording real evidence. To publish the branch, the owner must push it
(`git push -u origin fable/recording-core`) — pushing was outside this run's authorization.
