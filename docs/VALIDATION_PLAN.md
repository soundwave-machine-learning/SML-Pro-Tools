# Validation and gate authority — revision 1.6
All DAW tests NOT EXECUTED. This file owns classification/measurement conventions; topic specifications own behavior and stimuli. `docs/evidence/sprint_NN/` stores real logs/CSVs/settings/images; no fabricated reports ship in this package.

## Gate classes and continuation
- Portable software A1–A4: deterministic engine/model/state tests, host-appropriate build/CI and available UI harness checks. Mandatory failures block dependent software. Platform-specific portions explicitly assigned below to H gates do not block independently verified portable code.
- Native H1–H4: real target OS, device/driver/plugins and owner usability/audio-artifact review. Missing access = BLOCKED FOR TARGET VALIDATION. They block release, not portable work through passed A gates.
- Claim-only: N10 Pro Tools edit matching, native Logic curves/nulls, matched FL Studio/Pro Tools/Reason response tests. Absence prevents equivalence claims, not independent delivery.
- Deferred: M-T3/4/8 native summing/tape transfer/aliasing/listening; detailed tape-machine UI and documented future candidates. DEFERRED BY OWNER, not current failure.

Run inspected `cargo xtask ci` at baseline and each accepted milestone. Record actual expanded commands/exit codes. Establish exact added test commands in `docs/evidence/sprint_01/validation-command-index.md`. Missing required portable tests/toolchain/CI failures cannot be relabeled H pending. Fix ordinary setup within authority; otherwise preserve blocked work. A native-only check cannot become a portable passing fixture by renaming it. Local WIP checkpoint is allowed without implying an accepted milestone.

## Software matrix
| Gate | Required checks and stimuli | Expected evidence/outcome |
|---|---|---|
| A1 baseline/removal | Full host-appropriate CI; inventory registry/factories/UI/CLI/MCP/demo/offline dependencies; old built-in session save/reopen/export | catalogue.json and removal inventory: no selectable/shipping catalogue; preserved internal primitives and external hosts; inert missing metadata retained; explicit export acknowledgment/failure |
| A1 capture | 44.1/48 kHz mono/stereo ramp and impulses, regular/irregular callback sizes, finalize/reopen, arm/monitor independent | capture-tests.log: exact frame order/count/channel map; same-format f32 PCM identity; integer conversion <=1 LSB; source immutable |
| A1 recovery | Queue saturation, writer error, simulated disk full/disconnect/process interruption | failure-recovery.log: explicit failure, no silent skips/overwrites, recover durable frames exactly; quantify uncommitted loss; data flush ordered before durable journal update |
| A1 timing | CL-T1/2/5 headless, CL-T3 scheduler/offset primitives | clock-tests.log, device-negotiation.json, realtime-audit.md: exact logical frames/generation/offset signs; no callback allocation/lock/file work in tested host path |
| A2 editing | V01–V11/V14 deterministic portions, staged V-E2E through edit/reopen; N01–06/N08; N07 command/focus unit fixtures; CL-T3 full punch/count-in/loop semantics | vocal-coverage.csv, nudge/ logs, vocal-e2e.json: immutable originals, correct comp/edit coordinates, no fractional drift, atomic undo/rejection |
| A3 graph/host | B-T1..6, host insert/editor lifecycle where host supported, V12 generic-host capability and V13 full export fixtures, final V-E2E fixture | mixer/host/bus logs: valid graph/neutral bus creation/mute/solo, state, missing plugins, actual host render and teardown; personal plugin remains H2 |
| A3 profile/slots | M-T1/2/5/6, SL-T1..7 portable components, CL-T4 including slots/parallel/supported sidechain | clean-mix/slot-order/slot-state/compensation logs: expected coefficients, deterministic path order, <=1 sample fixture alignment; no double gain/pan |
| A3 dual view | VM-T1..6 via engine and available UI harness; stage actual renders if available | state/gesture/snapshot/width logs; no gain on view switch; 100% width exact pass-through; full-width node can move; missing native interaction remains H |
| A4 regression | Repeat full CI and all applicable feature suites on final source; package build/provenance if supported | regression.log and real artifact manifest; no prior gate regression, precise skipped/unavailable checks |

Test framework/commands are discovered and implemented in Sprint 1; NOT EXECUTED is honest until then. A1 does not require Sprint 2 comp UI or Sprint 3 slots to exist; its fixtures establish lower-level interfaces. Later integration repeats those invariants. V12/13 and full V-E2E completion occur after host/routing implementation, not before.

## Native and owner matrix
| Gate | Configuration / stimulus | Criterion and evidence |
|---|---|---|
| H1 recording/clock | Native Windows x64 release build, named interface/driver, 44.1/48 kHz, actual supported configuration; CL-T6 calibration and CL-T7 stability | correct physical input/levels, dry files/recovery, no dropped/duplicated frames, valid offsets and uncertainty; hardware-recording.md, recording-alignment.csv |
| H2 own plugins | At least named owner VST3 EQ and dynamics plugin; 20 editor open/close cycles, monitoring, save/reopen/render; owner pitch plugin if available for V12 | native editor/state/process/teardown works for named versions; missing user pitch plugin keeps V12 compatibility pending; owner-plugin-matrix.csv |
| H3 latency | CL-T6 physical input-to-output monitoring plus separate output-input overdub calibration, clean live path at supported 64/128 frames | proposed <=10 ms; <=5 ms stretch only. Measurement topology/uncertainty and driver estimates separate; rtl-matrix.csv. No arithmetic-only hardware PASS |
| H3 stability/response | CL-T7 two 30-minute runs (empty slots and declared plugin workload), 16 playback/2 record tracks, Split open; capture uninterrupted. Separate short transport/arm/seek stress | zero xruns/lost frames; callback p99 <0.8*(N/Fs), all missed deadlines logged; p95 event-to-visible <=50 ms, p95 active frame time <=25 ms, no host interaction stall >100 ms. Retain >=60 FPS target on a 60 Hz benchmark with explicit measurement resolution; no rounding a slower result to PASS. response-profile.csv, callback-profile.csv |
| H3 specific interaction | N07 physical keypad/focus; N09 100 nudge actions/16 tracks/2000 clips; VM-T7 cross-view timing; SL-T8 slot workload; M-T7 native | exact actions/no stale burst; apply same response/xrun gates. These can share instrumented workload where stimuli overlap, not fabricate independent runs |
| H4 portable candidate | Actual Windows artifact, clean extraction and native launch/setup/record/reopen/plugin/export; 1366×768 and 1920×1080 at 100/125/150% scale | readable/reachable controls/no overlap; windows scroll instead of forcing five tall inserts onto small display; package-smoke.md and actual images/hashes/architecture/SHA |
| Owner UX/audio | B-T7 simple bus creation <=3 interactions after selection excluding typing; V-E2E + VM-T8 + Nudge chapter walkthrough; listen for join/stretch/monitor glitches | owner observations, unresolved gaps and fixes; no invented approval |

All relevant native checks are mandatory for release except explicitly conditional personal-plugin availability and claim-only comparisons. A missing named pitch plugin is an explicit V12 compatibility gap, never “all plugins work.” Partial target coverage supports a reviewable candidate only. No owner audio or plugin binaries/licenses in repository evidence; generated fixtures preferred.

## Measurement conventions
Normalize fixture input/output ranges as specified, capture exact source/sample rate/layout/pan/slot positions and use identity paths at unity. Compare deterministic audio only; disable dither, random modulation/noise and automatic normalization for null tests. Tail/preroll/latency alignment must be recorded. Do not realign away an offset in a recording-placement test.

Absolute waveform residual <=1e-5 is for deterministic aligned paths; the tighter M-T1/VM-T3 limits apply to their stated fixtures. A dB tolerance does not apply to exact-zero expected coefficients; assert zero/absolute error. Identity width/empty slot/screen-open tests use exact sample equality with the same graph/profile/format baseline, not an old precision/pan algorithm. Test f32/f64 boundaries explicitly.

For callback load use frames serviced N / Fs, not observed inter-callback wall time. For frame rate record displayed-frame intervals/measurement resolution on a specified display. Separate event-to-visual, event-to-audible and smoothing delay. 60 Hz is not proof of responsive audio. Prolonged baseline device limits stay visible rather than disappearing into an average.

Physical calibration, latency and recording confidence obey CL04/CL-T6; unknown driver timing is Unknown, never zero. Exact headless frame math and <=1-sample synthetic compensation do not prove <=1-sample analog alignment. If the physical measurement cannot resolve that precision, record the uncertainty and block that precision claim; do not invent a tolerance waiver. Native latency target readiness remains separate from portability.

## Reports and readiness
Each report includes requirement/check IDs, command, configuration, exit/result, provenance, evidence and gap. Use INSPECTED for source/log reads; RERUN only for actual executed checks. Test-only providers are never shipping sound models. End with portable status, target/H status, current slot readiness, deferred native sound-model status, claim restrictions and exact next action. Release requires mandatory A/H/owner gates; software success alone is not a finished professionally validated DAW.
