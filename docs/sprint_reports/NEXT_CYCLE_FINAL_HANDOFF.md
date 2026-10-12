# Next cycle — final handoff (four prompts, revision 2.2 contracts)

Branch `fable/recording-core`, pushed to origin. Base checkpoint `396fd0a`. Environment P
(Linux x86_64 container, no audio device, no GUI, no owner plugins, no model/speech runtime).
Every sprint ran without routine approvals; each has a report, a CI log and evidence folder.

| Sprint | Commit | Gate | Report |
|---|---|---|---|
| G0/E01 baseline + contracts | `798d49e` | CI exit 0 | `sprint_reports/NEXT_SPRINT_01_REPORT.md` (prelude) |
| 1 Protect the session (R01–R09, B01–B04, B09) | `7bffb39`, `3f7e50f` | CI exit 0 (631) | `NEXT_SPRINT_01_REPORT.md` |
| 2 Studio workflow (R10–R22, R37) | `79c419f`, `0baa780` | CI exit 0 (661) | `NEXT_SPRINT_02_REPORT.md` |
| 3 Plugin hosting (R23–R31) | `f176366`, `aac507d` | CI exit 0 (673) | `NEXT_SPRINT_03_REPORT.md` |
| 4 Assisted studio (R32–R36, R38) | see STATE.md | CI exit 0 | `NEXT_SPRINT_04_REPORT.md` |

## Where the product stands
- Portable (P): every one of the 38 rows has an implementation and tests; see
  `next_cycle_execution/FINAL_REQUIREMENT_MATRIX.md` for the per-row status, evidence and note.
- AWAITING_EXTERNAL_PLUGIN (7): the SML plugins are separate projects; host work for each is in
  place and recorded in the capability matrix; nothing was built in their place.
- BLOCKED: E06 generative assistant/speech (no licensed runtime configured); B03 numpad
  distinction (toolkit); B06 ASIO/Windows build host.
- TARGET_PENDING (native N/M/L): `docs/TARGET_MACHINE_GAUNTLET.md` plus the rows marked
  pending in the matrix — W1/W2 callback timing and xruns, 30-min recordings, 2 h/8 h runs,
  real plugins and native editors, phone on iOS/Android, listening reviews, clean-machine
  install/reopen, owner workflow review.

## How to resume
1. `CARGO_INCREMENTAL=0 cargo xtask ci` must be exit 0 on the checked-out commit.
2. On the target machine run the gauntlet and record each row in `REQUIREMENT_STATUS.csv`
   with commit, command, exit code, environment N/M/L and evidence path; never a synthetic PASS.
3. To unblock E06: configure a provider behind `assistant::Provider` (license, context, cost
   and privacy recorded in EXPERIMENT_DECISIONS); the action boundary stays as is.
4. To close R23–R29: supply each artifact, run `engine.plugin_capabilities` and the
   V-EXTERNAL_PLUGIN cases on it; R28 needs an interface decision first.

## Documents
`docs/USER_GUIDE.md` (manual), `docs/COMMAND_REFERENCE.md`, `docs/MIGRATION_AND_RECOVERY.md`,
`docs/next_cycle_execution/{STATE,RESUME,COMMAND_MATRIX,EXPERIMENT_DECISIONS,PERFORMANCE_REPORT,
FINAL_REQUIREMENT_MATRIX,SPRINT_0{2,3,4}_DESIGN}.md`, the four sprint reports, and
`docs/evidence/next_cycle/sprint_0{1..4}/`.
