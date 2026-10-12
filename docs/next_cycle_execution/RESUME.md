# Resume here

Last checkpoint: see STATE.md. Next safe command: `CARGO_INCREMENTAL=0 cargo xtask ci` (must
stay exit 0), then continue the current module named in STATE.md.

## Open ledger (B01–B09, mirrored from the issued package; closure recorded here only when earned)
| ID | State | Where |
|---|---|---|
| B01 structural edits allocate on the audio thread | CLOSED (portable): prepared graph bundle, 0 alloc/dealloc/fallback guard passing; residue listed in EXPERIMENT_DECISIONS | Sprint 1 |
| B02 64-frame overruns in the VM | ANALYSED with a paced no-work control (see EXPERIMENT_DECISIONS B02 once the run completes) | Sprint 1 |
| B03 numpad vs main row | BLOCKED at the toolkit (record: `docs/evidence/next_cycle/sprint_01/keyboard-backend-inspection.md`); remappable fallback | Sprint 1 |
| B04 non-functional recording toggles | CLOSED: disabled with reason, legacy flags kept, test | Sprint 1 |
| B05 native gates | TARGET_PENDING | — |
| B06 ASIO / GNU cross-build | OPEN — inspected (`backend-inspection.md`): needs ASIO SDK + Windows build host | Sprint 4 |
| B07 small-screen usability | TARGET_PENDING (portable scroll fix exists) | — |
| B08 V12 external pitch plugin | AWAITING_EXTERNAL_PLUGIN | Sprint 3 |
| B09 autosave/media durability, containment | autosave/media: PASS_PORTABLE (E02); runtime containment: E03 pending | Sprint 1 |
