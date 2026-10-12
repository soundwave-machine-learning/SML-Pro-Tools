# Next Sprint 01 — Protect the session (revision 2.2 contract)

Status: **PARTIAL** (portable gates passed for the implemented modules — full `cargo xtask ci`
exit 0, 631 tests; runtime containment decided by E03 — see below; native/power-loss gates
TARGET_PENDING; R05 log redaction and the written reliability audit not done). Environment P only
(`docs/evidence/next_cycle/sprint_01/environment.json`). Started 2026-10-11T23:35Z from checkpoint
`396fd0a7a7aa5b62d8dae0e49b3413c7e2e3432b` (verified: clean tree, `main` untouched, branch never pushed).

## Outcome
| Item | Result | Evidence |
|---|---|---|
| E01 / G0 | PASS — contracts installed verbatim (`docs/next_cycle_contracts`, verifier PASS), command matrix discovered, baseline `cargo xtask ci` rerun exit 0 (616 tests) | `baseline-ci.log`, `COMMAND_MATRIX.md` |
| B01 structural edits on the audio thread | CLOSED (portable): off-thread `GraphPrep`, allocation-free `apply_prepared`, 0 alloc/dealloc/fallback over 8 structural edits, output bit-identical to the offline engine | `structural_alloc` test, `EXPERIMENT_DECISIONS.md` |
| B02 64-frame overruns | NOT REPRODUCIBLE AS COMPUTE in P (scheduler preemption; p99 compute ≤ 5 % of budget); native pending | `callback-profile-control.csv` |
| B03 numpad / Win key | BLOCKED at the toolkit (egui-winit 0.36.2 discards key location; drops Super on non-mac); remappable fallback documented | `keyboard-backend-inspection.md` |
| B04 non-functional toggles | CLOSED: disabled with reason, legacy flags kept, menus show the reason | `record_mode_tests` |
| B06 ASIO / cross-build | OPEN (needs ASIO SDK + Windows build host) | `backend-inspection.md` |
| B09 / R04 autosave + media recovery (E02) | PASS_PORTABLE: rolling validated generations, index commit, retention/budget, recovery window, unfinished-take discovery, unclean-exit marker; fault injection at every boundary | `autosave` tests, `unfinished_takes`, `autosave_worker` |
| R05 Safe Open | PASS_PORTABLE (`--safe`, `--reset-prefs`); log redaction NOT done | `safe_open_tests` |
| R07 isolated scanning | PASS_PORTABLE (worker process per bundle, timeout/kill, quarantine, fingerprint cache, re-enable/rescan UI) | `scan_isolated` tests |
| E03 runtime isolation | DECIDED: containment feasible, blocking IPC not for the default callback; opt-in sandboxed mode designed for Sprint 3 | `ipc-probe.csv` |
| R09 command contracts | PASS_PORTABLE (engine): envelope, receipt, dedup, stale revision, gesture coalescing, knowledge corpus | `envelope` tests |
| E05 / E06 feasibility | recorded below | `EXPERIMENT_DECISIONS.md` |

## Requirement statuses (R01–R09)
See `docs/next_cycle_execution/REQUIREMENT_STATUS.csv` (authoritative). Summary: R01 PASS_PORTABLE;
R02 PASS_PORTABLE (+B01 guard; physical H1 pending); R03 PASS_PORTABLE (CL-T6/7 pending);
R04 PASS_PORTABLE (power-loss pending); R05 PARTIAL (Safe Open done; log redaction and the
written reliability audit pending); R06 UNVERIFIED→REPORTED carry-over (scan isolated; native
editor/state H2 pending); R07 PASS_PORTABLE (scan) + E03 result for runtime; R08 PASS_PORTABLE;
R09 PASS_PORTABLE (engine side).

## B02 control measurement
Run alone (release, CPU idle): the no-work control misses 98 of 45 000 64-frame periods from
scheduler preemption alone (max lateness 7.1 ms); the 16+2-track workload computes p99 57–67 µs
(≤ 5 % of the 1333 µs budget) with 3–4 millisecond-scale misses of the same preemption shape.
At 128/256 frames: 14/1 control misses, 0–1/0 workload misses, p99 ≤ 3.5 % / 2.6 % of budget.
B02 is not reproducible as a compute overrun in environment P; the native 64-frame device
callback (N) stays pending. Table and reading: `EXPERIMENT_DECISIONS.md` (B02); raw:
`callback-profile-control.csv`, `profile-control-run.log`.

## E03 runtime isolation experiment
One worker process per plugin over pipes: round trip p50 42 µs, p99 122 µs, max 3.2 ms at 64
frames (11 of 45 000 blocks over budget), ~6 frames added latency at p99; a crash is detected in
6.8 ms and a hang in 16.2 ms, the host continues (silence policy). Decision: containment works,
the blocking round trip does not belong in the default callback path; an opt-in one-block-ahead
sandboxed mode is the Sprint 3 R31 design with these numbers as admission criteria. No runtime
containment is claimed for this build. Raw: `ipc-probe.csv`, `ipc-probe-run.log`.

## Files / commits
Commits on `fable/recording-core` after `396fd0a`: `798d49e` (E01), `7bffb39`
(`feat: establish recoverable SML Studio foundation`, CI exit 0 — `ci-sprint1-batch1.log`),
then the measurement/docs commit that follows it.
New: `crates/engine/src/{autosave,envelope}.rs`, `crates/mix/src/scan_isolated.rs`,
`crates/mix/tests/structural_alloc.rs`, `crates/engine/examples/ipc_probe.rs`,
`docs/next_cycle_contracts/`, `docs/next_cycle_execution/`, `docs/evidence/next_cycle/sprint_01/`.
Changed: `crates/mix/src/lib.rs` (GraphPrep/apply_prepared, DelayLine reserve, safe mode),
`crates/playback/src/{lib,capture}.rs`, `crates/engine/src/{lib,cmd/mod,cmd/options,cmd/setup_more}.rs`,
`crates/engine/examples/sml_profile.rs`, `crates/ui-egui/src/{lib,windows,menus,panels,plugin_scan,mix_window}.rs`,
`crates/{clap,vst3}-host/src/{lib,registry}.rs`, `apps/soundcraft{,-cli}/src/main.rs`, `docs/USER_GUIDE.md`.

## Tests run (all P)
Baseline and batch `cargo xtask ci` (logs in the evidence folder); the targeted suites named per
row above. Test counts alone are not coverage: the fault models actually exercised are listed
in EXPERIMENT_DECISIONS.md.

## Not run / failed / blocked
- Native: H1–H4, CL-T6/7, physical keypad (B03), power-loss (E02), real plugins through the
  isolated scanner, ASIO build (B06) — TARGET_PENDING.
- Not implemented in this sprint: log/crash-report redaction (R05), a written reliability audit
  document beyond the ledgers (R05), E04/E07 fixtures (deferred to Sprints 2/4 as the contract allows).
- No test was weakened; two of my own new tests were corrected for their own bugs (Tab test
  argument order was last cycle; this cycle: the allocation guard's fallback window counted the
  reference engine, fixed by narrowing the window; the discovery test needed a short durable interval).

## Risks
- `PDC_RESERVE` (4096 samples) bounds allocation-free latency changes; above it the ring
  reallocates (counted, logged). Clip-effect chains still allocate on first use (documented).
- Isolated scanning requires the app binary to act as its own worker; packaged builds must ship
  the same executable (true for the Linux tarball and the Windows zip).
- Autosave media duplication for large unsaved takes is one copy per source id (bounded by the
  session's unsaved sources; the capture file itself already exists on disk).

## Next safe task
Sprint 2, task 1 (vocal workflow gaps) after this report's commit; `CARGO_INCREMENTAL=0 cargo xtask ci`
must stay exit 0.
