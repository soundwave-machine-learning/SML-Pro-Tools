# Validation, experiments and evidence rules

No tests have been run against product code while authoring this package. The package verifier checks documents only. Sprint 1 must establish real commands from the actual repo; subsequent contracts remain DRAFT/BLOCKED for execution portions that depend on undiscovered commands or unfrozen algorithms.

## Test environments
P: portable automated harness on available host; record OS/toolchain/CPU and commit. N: Windows x64 native build, real audio interface/driver and native plugin editors. M: real phone browser on the same LAN; record device/OS/browser/security origin/network. L: licensed/reference listening environment and versioned files. These are distinct results; simulation cannot pass N/M/L.

Baseline workload W1: 16 playback audio tracks, two armed/captured inputs, 2,000 timeline clips, at least two nested buses and sends; Console/Visual Split visible. Use 48 kHz first with supported 64/128/256 frame device modes; report requested/effective buffer and plugin chain. Repeat core recording checks at 44.1 kHz. Use clean graph and a declared realistic hosted/native chain separately. W2 stress: 64 tracks, 10,000 clips, 1,000 plugin metadata entries and background analysis/scan. W2 characterizes scalability and cannot silently replace W1 acceptance. Fix fixture hashes, hardware, power settings, displays and build optimization before comparisons.

## Experiments: freeze criteria before implementation scoring
| ID | Question / method | Required output and consequence |
|---|---|---|
| E01 | What actually exists and how is it built? Inspect code/CI/reports; run applicable baseline | Current HEAD, protected changes, exact command matrix, capability/gap ledger, platform limits; G0 |
| E02 | Can project/media recover across crash and storage failure? Kill at each commit boundary; disk-full/rename/slow-writer tests | Durability protocol, measured committed/tail loss, flush interval/storage bounds, recovery fixtures; G1 requires no lost acknowledged durable data |
| E03 | Is runtime plugin isolation compatible with low-latency operation? Minimal worker plus gain/delay/crash/hang fixtures and available real plugin | IPC design, added samples, callback deadline/CPU, editor/state compatibility and chosen supported mode; no containment claim until passed |
| E04 | What stalls menus/commands? Native tracing at W1/W2, cold/warm, 100 menu/search repetitions per scenario | Event-to-usable distributions, thread blocking causes, profiler evidence and fix; supports R19 |
| E05 | Which native Bend service and actual external-plugin interfaces are supported? Existing code/dependency/license audit and supplied artifact inspection | Validated time-stretch baseline and measured host/interface matrix; no plugin algorithm research/build obligation |
| E06 | Which actual model/STT/TTS can run the assistant? Installed/provider capability, license/cost, structured action/voice trials | Actual integration selection and limits, privacy policy, timeout/cancel behavior; unsupported provider remains blocked, no fake conversational pass |
| E07 | Are meters/export correct? Select current applicable standard/reference vectors, compare independent results | Pinned standard and licensed implementation provenance, tolerances, fixtures, decoded-file and routing results; blocks compliance/export readiness |
| E08 | Does native integrated performance survive realistic use? W1 continuous runs, faults and soak | Timing/xrun/crash/recovery distributions and repeatable environment; blocks full native release claim |

## Universal performance and integrity goals
- Clean physical software-monitor latency <=10 ms at a stable supported 64/128 setting; <=5 ms is stretch. Measure with the foundation topology and uncertainty, not just buffer arithmetic. Matched Pro Tools/Reason equivalence requires real same-hardware runs; absence blocks comparison only.
- Callback p99 compute duration <80% of N/Fs, zero missed deadlines/xruns in the defined successful run; log max, jitter and queue depth. A favorable percentile cannot hide a deadline miss. Host callback allocation/lock/I/O violations fail.
- Deterministic sample placement is exact; compensated impulse paths align within one sample. Physical calibration claims report uncertainty and repeatability rather than pretending every interface is sample exact.
- W1 UI p95 input-to-visible command/menu <=50 ms and indexed search <=100 ms; characterize p99/max and delivered frames. Test cold and warm separately. Native comparison with FL Studio is optional evidence, never a manufactured benchmark.
- No lost acknowledged durable media/session state; successful graceful-stop takes preserve every accepted frame. Crash recovery preserves all validated committed frames and reports tail exposure. Unknown power-loss guarantees cannot be called zero-loss.
- Run two separate uninterrupted 30-minute W1 recording tests (clean and declared processor chain). Then an eight-hour playback/edit/automation soak plus a separate two-hour recording endurance run before full release readiness. If runtime/hardware prevents these, report PENDING; do not shorten and label equivalent. These durations are validation requirements, not a promise about the owner's remaining tool time.

## Requirement test families
Every `V-...` family in REQUIREMENTS.md expands into cases in the repo ledger with requirement, stimulus, expected result/tolerance, environment, command, fixture hash and evidence path. Use these mandatory stimuli:

| Families | Stimulus and expected behavior | Environment |
|---|---|---|
| BASELINE / AUTONOMY | Reconcile original reports with actual diff and commands; preserve dirty files; resume after checkpoint without repeated edits | P |
| CAPTURE / CLOCK | Variable blocks/seek/loop/rate restart, encoded input, driver errors, impulse latency/PDC; foundation CL-T cases | P,N |
| RECOVERY / RELIABILITY / RELEASE | Kill during project/media commit, disk full, malformed state, device removal, GUI stall, model/plugin failure; reopen intact committed data and explicit faults | P,N |
| HOST / ISOLATION / LEGACY | Scan crash/hang, runtime worker fault, native editors, state/PDC/sidechain, missing legacy effect; no scan on audio thread or hidden restoration | P,N |
| COMMANDS | Stale revision, duplicates, deleted/locked targets, overflow, grouped undo and final coalesced value; exact acknowledged mutation or none | P,N |
| VOCAL / TOOLBAR / NUDGE | Every foundation V/T/N case; source hashes unchanged, exact math, all tool variants, one gesture undo and correct focus | P,N,L for render quality |
| SHORTCUTS | Full versioned applicable inventory, distinct profiles, numpad/top-row, modifiers, text/plugin focus and OS interception; zero no-op bindings | P,N |
| ROUTING / MIXERS / PROFILES / SLOTS / MONITOR | Impulse/noncommuting providers, send pan, bus spill, shared view automation and slot order/PDC; monitor/reference excluded from export | P,N |
| LAYOUTS / MENUS / SKINS | W1/W2 native timings, scale/theme invalid import/focus; no loss of commands or capture; targets above | P,N |
| REMOTE | Pair/revoke, unauthorized Origin, duplicate/stale record, two clients, disconnect/reconnect, actual cue controls; native transport preserved | P,M,N |
| KNOWLEDGE / ASSISTANT / VOICE | Versioned question set, nonexistent tools, ambiguous track, injected retrieved instructions, invalid/stale actions, canceled speech/model timeout; no unauthorized mutation/audio leakage | P,N,M |
| EXTERNAL_PLUGIN | Available real plugin load/editor/state/automation/sidechain/bypass/render; named future-plugin cases AWAITING_EXTERNAL_PLUGIN until supplied, never assumed compatible | P,N |
| EDIT_RENDER | Native timing maps/Bend, cancel/revision race, undo/reload, duration/pitch-preservation and immutable source; inspect actual clip-aware integration requirements without creating pitch/alignment algorithms | P,N,L |
| HOST_EVIDENCE | Repeated host render, block/rate changes, rapid automation, state/crash/reload and compensation; separate plugin DSP quality from host accuracy | P,N |
| MASTERING / HANDOFF | Standard meter vectors, reference/cue/speech exclusion, dither/rate/depth/tail, decoded output; install/reopen/migrate in clean test environment | P,N |

## Evidence storage and reporting
Actual implementation writes `docs/evidence/next_cycle/sprint_01` through `sprint_04`. Each evidence manifest records commit, UTC time, command/exit code, environment, fixture/config hash and result type. Keep compact CSV/JSON/logs in repo; large reproducible audio/media artifacts need hashes and regeneration instructions rather than repository bloat. Never create synthetic PASS logs. A generated expected waveform is a fixture, not an observed test result.

Every sprint report states implemented, tested, not run, failed, blocked, reference limitations, changed files, regressions, next safe action and actual runtime used if available. Test counts alone do not prove coverage. Any risk-based omission names the concrete untested behavior and prevents its unconditional release claim. Do not repeat all tests after a prose-only change; rerun affected meaningful gates and required release checks.
