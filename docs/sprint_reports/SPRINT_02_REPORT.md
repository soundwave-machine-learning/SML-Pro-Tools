# Sprint 02 report — Vocal editing and dedicated Nudge (gate A2)

Prerequisite: A1 portable PASS (commit `f70ac26`). Host: same Linux container (no Windows,
no Pro Tools, no keypad hardware).

## Verdict
**A2 PORTABLE: PASS for the executed software rows** listed below, with explicitly PARTIAL rows
(V01, V03, V07 placement, V13, V14) carried forward and named — they are not counted as passing.
N07 physical keypad, N09 performance, editing usability/listening: **native/owner gates pending**.
N10 Pro Tools comparison: **claim-only, not performed**.

## Work done
1. **Dedicated Nudge engine** (`engine cmd::nudge`, `model nudge`): seven targets with distinct
   command ids (`nudge.clips`, `nudge.contents`, `nudge.trim_start`, `nudge.trim_end`,
   `nudge.selection`, `nudge.selection_start`, `nudge.selection_end`) plus `nudge.apply`,
   `nudge.set`, `nudge.info`; typed value/unit separate from Grid; per-target/unit exact anchor
   with signed step count, ties-to-even single rounding, reset on reposition/target/unit/value/
   rate/tempo change; tempo-map musical steps; content-slip sign; trims keep envelope time
   reference; atomic rejection (locks, project start, handles, zero length, stretched clips =
   unsupported mapping); panel state persists and is restored by undo/redo.
2. **Keyboard map** (`ui-egui nudge_keys`): SML reference preset (Num±, Alt/Ctrl trims,
   Alt+Shift/Ctrl+Shift selection boundaries, Ctrl+Alt value change, Shift-only unassigned,
   laptop comma/period only under explicit Edit Keyboard Focus); text/plugin/dialog/shortcut
   capture focus produce no edits. Wired into the edit-window shortcut handler.
   **Recorded blocker**: egui-winit 0.36 maps NumpadAdd/Subtract to the same `Key` as the main
   row and exposes no Windows/Super modifier, so keypad-vs-text distinction and Win+Num±
   (content nudge) cannot be implemented from the UI layer without a backend change. Content
   nudge is available from the panel and command search.
3. **NUDGE toolbar group**: value, unit, target, ◀ ▶, 1 smp / 1 ms / 10 ms favourites, exact
   sample readout.
4. **Vocal tools**: existing commands audited and covered by new deterministic tests (V02, V04,
   V05, V06, V09, V11); loop-record pass splitting moved into the engine
   (`io::add_loop_recording`) and tested (V07).
5. **Staged V-E2E** through comp/breath clear/gain ride/nudge+undo/group/save/reopen: render
   equals an independently constructed reference (max abs error 0.0, tolerance 1e-5); source
   hashes identical in memory and after reopen.
6. `docs/USER_GUIDE.md` started (recording, Nudge, vocal walkthrough).

## A2 checks (RERUN)
| Check | Command | Result | Evidence |
|---|---|---|---|
| N01–N06, N08 (+ rejection does not advance state) at 44.1/48/96 kHz where specified | `cargo test -p soundcraft-engine cmd::nudge` | 8 passed | `nudge/nudge-engine-tests.log` |
| N07 unit level (modifier matrix, focus, laptop, distinct ids) | `cargo test -p soundcraft-ui-egui nudge_keys` | 4 passed | `nudge/nudge-keys-tests.log` |
| V02/V04/V05/V06/V09/V11 + V-E2E | `cargo test -p soundcraft-engine --test vocal_workflow` | 7 passed | `vocal-edit-tests.log`, `vocal-e2e.json` |
| V07 loop passes / CL-T3 | `cargo test -p soundcraft-engine loop_record` (+ Sprint 1 clock tests) | 1 passed | `loop-record-tests.log` |
| Full CI | `cargo xtask ci` | exit 0, 593 passed / 0 failed | `ci-sprint2.log` |

Coverage per row: `docs/evidence/sprint_02/vocal-coverage.csv`.

## Gaps and blockers (carried, not hidden)
- N07 physical keypad / Win modifier: toolkit limitation above; native test pending.
- N09 (100 actions / 16 tracks / 2000 clips, p95 ≤ 50 ms, zero xruns): native gate, not run.
- N10 Pro Tools comparison: not performed; no parity claim. Shift+Num± intentionally unassigned.
- V07: take placement still from the transport start position, not backend capture timestamps
  (CL04 placement wiring PARTIAL); no musical count-in exists (upstream or here) — only pre-roll; count-in is NOT IMPLEMENTED (correction made in Sprint 4).
- V12: needs a named owner pitch plugin (H2).
- V13 named export paths: Sprint 3.
- No UI screenshots captured this sprint.

## Requirements
R11 vocal tool families: PARTIAL (rows above). R12 dedicated Nudge: software PASS (N01–N06,
N08, N07 unit) with N07/N09 native pending and N10 claim-only.
