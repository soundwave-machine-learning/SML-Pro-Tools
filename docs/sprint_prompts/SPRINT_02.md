# Sprint 02 — Vocal editing and dedicated Nudge
Revision 1.6 • PLANNED / NOT EXECUTED • gate A2.

## Objective and prerequisites
Make vocal take selection, comp/edit/gain/fade/strip-silence/timing operations real and precise. Require A1 portable PASS, immutable recorded media and known scheduler/offset contracts. Pro Tools native access is claim-only; native keyboard/hardware checks remain H gates.

## Read set and allowed scope
Read PROJECT_BRIEF, REQUIREMENTS_MATRIX, VALIDATION_PLAN, VOCAL_EDITING_SPEC, NUDGE_SPEC, CLOCK_LATENCY_SPEC and REFERENCE_BEHAVIOR. Inspect actual edit/edit_more/clip/clip_more/track/track_more/transport command callsites; model clips/playlists/selections/undo; time grid/tempo; UI edit_window/shortcuts/toolbar/widgets/dialogs; existing regression tests.

Allowed: native editing commands/UI, take/comp/punch/loop conveniences, source-safe stretch service, Nudge arithmetic/focus/undo and scheduler integration. No ARA/note pitch editor, new auto-align engine, bundled effects or new framework. Own pitch plugins finish through Sprint 3 hosting.

## Concrete work
1. Complete software portions of V01–V11/V14. Expose actual selection/nav, split/trim/heal/duplicate/consolidate, editing modes with explicit ripple opt-in, fades/crossfades/batch, gain envelopes, preview strip silence, groups/locks and context commands. Reuse valid source; menu presence is not completion.
2. Take lanes/playlist audition/phrase comp preserve alternates/alignment, seam undo and single active playback. Finish V07 pre/post-roll/count-in/punch/loop take UI against CL-T3 scheduler edge tests. No destructive overwrite or implied full QuickPunch implementation.
3. Implement all explicit Nudge panel targets, typed separate units/values, stable fractional anchors, tempo/rate transforms, atomic source/group rejection and precise source-slip signs. Native reference preset leaves unsupported Shift-only equivalence unassigned; SML Selection-only is still available by panel/command search/remap. Distinguish keypad/text/plugin focus; no global OS key interception.
4. Preserve/extract existing pitch-preserving editing service, define its limits/tests and reject unsupported mappings. V12 generic plugin integration and V13 processed export are staged to Sprint 3. New note-level pitch editor remains excluded.
5. Complete V-E2E through synthetic takes/comp/edit/undo/save-reopen. Original hashes unchanged. Record real commands and coverage; later routing/host/export steps are not fake dependencies of A2.

## Validation and observable completion
Run A2 software matrix: V01–V11/V14 deterministic checks, N01–06/N08, N07 dispatch/focus unit tests and CL-T3 full recording-convenience integration. Actual physical numpad N07, performance N09, usability and listening remain native/owner gates. Claim-only N10 pending does not stop implementation. No essential portable editing row may be silently dropped for budget.

Artifacts under docs/evidence/sprint_02/: vocal-coverage.csv, vocal-edit-tests.log, vocal-e2e.json, nudge/ logs/fixtures, actual UI captures if available. Start USER_GUIDE Nudge and vocal walkthrough sections. Write docs/sprint_reports/SPRINT_02_REPORT.md. A2 portable PASS permits Sprint 3; lost takes/corrupt mapping/missing mandatory software tools are PARTIAL/BLOCKED, not accepted.

## Execution, report and continuation rules
Follow docs/AUTONOMOUS_BUILD_PROMPT.md for authorization, worktree, six-hour cap and stop/checkpoint policy. Applicable repository instructions remain mandatory. Local ordinary edits/test fixes are authorized by launching the master; no push/merge/install/signing/purchase or delegated agents. No test weakening or fabricated results. Obsolete catalogue-only tests may be retired with explicit rationale and preserved shared coverage.

Run the full inspected `cargo xtask ci` and relevant real feature commands before an accepted milestone commit. Record unavailable native checks as H pending and portable failures as blocking. Write the sprint report only after actual work, update living handoff/command index, inspect diff and print commit SHA after commit. A partial/WIP checkpoint never claims acceptance. At any stop write the final report/next action even if no later sprint ran.
