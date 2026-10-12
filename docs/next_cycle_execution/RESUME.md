# Resume here

Last checkpoint: see STATE.md. Next safe command: `CARGO_INCREMENTAL=0 cargo xtask ci` (must
stay exit 0), then continue the current module named in STATE.md.

## Open ledger (B01–B09, mirrored from the issued package; closure recorded here only when earned)
| ID | State | Where |
|---|---|---|
| B01 structural edits allocate on the audio thread | CLOSED (portable): prepared graph bundle, 0 alloc/dealloc/fallback guard passing; residue listed in EXPERIMENT_DECISIONS | Sprint 1 |
| B02 64-frame overruns in the VM | NOT REPRODUCIBLE AS COMPUTE in P: control (no work) misses 98/45 000 periods from scheduler preemption; workload p99 ≤ 5 % of budget (EXPERIMENT_DECISIONS B02); native N pending | Sprint 1 |
| B03 numpad vs main row | BLOCKED at the toolkit (record: `docs/evidence/next_cycle/sprint_01/keyboard-backend-inspection.md`); remappable fallback | Sprint 1 |
| B04 non-functional recording toggles | CLOSED: disabled with reason, legacy flags kept, test | Sprint 1 |
| B05 native gates | TARGET_PENDING | — |
| B06 ASIO / GNU cross-build | OPEN — inspected (`backend-inspection.md`): needs ASIO SDK + Windows build host | Sprint 4 |
| B07 small-screen usability | TARGET_PENDING (portable scroll fix exists) | — |
| B08 V12 external pitch plugin | AWAITING_EXTERNAL_PLUGIN | Sprint 3 |
| B09 autosave/media durability, containment | autosave/media: PASS_PORTABLE (E02); runtime containment: E03 decided — feasible, not default; opt-in sandboxed mode designed for Sprint 3 R31 | Sprint 1 |

## Sprint 2 state (2026-10-12) — CLOSED (PARTIAL, portable)
Committed as `feat: complete SML Studio editing mixing and remote workflow` (`79c419f`)
after `CARGO_INCREMENTAL=0 cargo xtask ci` exit 0 (661 tests). Report:
`docs/sprint_reports/NEXT_SPRINT_02_REPORT.md`; evidence `docs/evidence/next_cycle/sprint_02/`.
Native gaps stay TARGET_PENDING in REQUIREMENT_STATUS.csv.

## Sprint 3 state (2026-10-12) — CLOSED (PARTIAL, portable)
Committed as `feat: harden SML Studio plugin hosting and integration` (SHA in STATE.md) after
`CARGO_INCREMENTAL=0 cargo xtask ci` exit 0 (673 tests). Report
`docs/sprint_reports/NEXT_SPRINT_03_REPORT.md`; evidence `docs/evidence/next_cycle/sprint_03/`.

## Sprint 4 state (2026-10-12)
Design decided in `docs/next_cycle_execution/SPRINT_04_DESIGN.md`; drafts of the assistant
service, assistant/mastering commands, E07 meter cases, E08 scenario and UI windows were
prepared in the agent scratchpad and are being integrated. Evidence root
`docs/evidence/next_cycle/sprint_04/` (eval set `assistant-eval.json` already there). Milestone
commit `feat: complete assisted SML Studio mastering workflow`. If resuming mid-sprint:
`CARGO_INCREMENTAL=0 cargo xtask ci` first.
