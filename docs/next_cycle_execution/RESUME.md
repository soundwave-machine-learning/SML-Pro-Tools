# Resume here

Last checkpoint: see STATE.md. Next safe command: `CARGO_INCREMENTAL=0 cargo xtask ci` (must
stay exit 0), then continue the current module named in STATE.md.

## Open ledger (B01–B09, mirrored from the issued package; closure recorded here only when earned)
| ID | State | Where |
|---|---|---|
| B01 structural edits allocate on the audio thread | OPEN — measurement test written (`crates/mix/tests/structural_alloc.rs`), implementation next | Sprint 1 |
| B02 64-frame overruns in the VM | OPEN | Sprint 1 |
| B03 numpad vs main row | INSPECTED: egui-winit 0.36.2 `key_from_key_code` folds NumpadAdd→Plus, NumpadSubtract→Minus and ignores `location`; egui `Key` has no keypad variants; `Modifiers` drops the Super/Win key on non-mac | Sprint 1 |
| B04 non-functional recording toggles | OPEN | Sprint 1 |
| B05 native gates | TARGET_PENDING | — |
| B06 ASIO / GNU cross-build | OPEN (inspect) | Sprint 1/4 |
| B07 small-screen usability | TARGET_PENDING (portable scroll fix exists) | — |
| B08 V12 external pitch plugin | AWAITING_EXTERNAL_PLUGIN | Sprint 3 |
| B09 autosave/media durability, containment | OPEN | Sprint 1 |
