# Sprint 02 — Finish the studio workflow

Contract revision 2.2 • Owner scope authorization: latest request for all ideas in four prioritized prompts. Status: SCOPED DRAFT; freeze execution commands and named prerequisites before dependent implementation.
Authoritative package file: docs/sprint_prompts/SPRINT_02.md. Requirement ownership: R10, R11, R12, R13, R14, R15, R16, R17, R18, R19, R20, R21, R22, R37.

## Objective
Deliver the familiar record/edit/mix experience, responsive UI and phone control on the verified foundation.

## Prerequisites and current evidence
G0 and portable G1 passed. Current command and graph interfaces frozen. E05 provides a viable pitch-preserving baseline before Bend completion. Runtime sandbox/native evidence may remain external blockers, but their absence cannot be concealed. No product test result is asserted by this contract.

## Read first
Read package-root START_HERE.md, then PROJECT_BRIEF.md, DECISIONS.md, FOUNDATION_AUTHORITY.md, ROADMAP.md, REQUIREMENTS.md, VALIDATION_AND_EXPERIMENTS.md and AUTONOMOUS_BUILD_PROMPT.md. Then read WORKFLOW_UI_REMOTE.md; ARCHITECTURE.md; all eight behavior specs in foundation (Nudge, vocal, toolbar, buses, profiles, slots, visual, clock). Sources S02–S06, S22–S25; D04–D05 and D09–D14. Paths are relative to the package docs directory unless stated otherwise.

## Scope and interfaces
Audio editing tools and command bindings, routing/mixer views/slots, UI layouts/themes/menu models, shortcut profiles, local remote service and knowledge/help. Reuse working features; no new custom summing/tape sound, instrument tools, public remote deployment or AI-controlled mutation yet. Interface changes must preserve versioned session/parameter IDs or include a tested migration; record actual ownership/thread rules in the execution dossier. Unrelated repository changes are out of scope.

## Work in dependency order

1. Use the gap ledger to finish take/comp/punch/pre-roll, clip gain/fades/strip silence, group edits, undo and persistence. Exercise a real lead/double vocal workflow instead of merely adding controls.
2. Implement every toolbar and Paint variant. Bind Bend to the verified baseline renderer and track revision/cancel ownership. Bind Listen to the foundation audition transaction; transport start must clean it up without stopping a recording unexpectedly.
3. Complete the dedicated Nudge section and exact rational/sample semantics. Prove source slip direction, group atomicity, tempo-map behavior and separate Grid state. Verify real keyboard focus and physical numpad distinctions on target hardware.
4. Build complete pinned shortcut inventories. Implement relevant commands in separate Pro Tools and Studio One profiles, plus editable Custom. Report unsupported/out-of-scope entries, OS/layout conflicts and unverified native mappings. No blank command handlers count as coverage.
5. Verify and finish bus creation/routing, sends, solo/mute, independent send pan and spill. Verify Console/Visual/Split drive one shared state and automation. Keep clean/legacy profiles and slot order compatible; test all Summing/Tape provider lifecycle, bypass and delay cases with fixtures and an available personal plugin.
6. Build unified Record/Edit/Mix/Master layouts, apply working SML Studio naming without changing persistent identities, and complete semantic data-only themes/scaling/fallback. Master layout may be a navigation shell here; mastering DSP/export acceptance belongs to Sprint 4 and must remain labeled incomplete until then.
7. Run E04 and remove demonstrated menu/search stalls. Cache plugin metadata, virtualize large lists, and isolate expensive loads. Record native cold/warm W1 and W2 distributions and audio continuity while operating menus.
8. Build authenticated LAN remote with pairing/revocation, phone controls, acknowledged commands, reconnect snapshots and bounded meters. Implement actual monitor-only volume/dim/mono and expose headphone control only for a real cue route. Test security origin and mobile deployment; no automatic cloud hosting.
9. Publish searchable, build-versioned command help and active shortcuts. Every shipped feature must describe its actual limits. Leave generative assistant integration to Sprint 4, with the service boundary ready.


## Required build and test execution
Run V-VOCAL, V-TOOLBAR, V-NUDGE, V-SHORTCUTS, V-ROUTING, V-MIXERS, V-PROFILES, V-SLOTS, V-LAYOUTS, V-MENUS, V-SKINS, V-REMOTE, V-KNOWLEDGE and V-MONITOR. Include all foundation N01–N10, T-T1–6 and relevant V/B/M/VM/SL cases. Use noncommuting slot fixtures, focus/undo/reopen tests, unchanged source hashes, W1 p95 menu <=50 ms/search <=100 ms, and real phone pair/disconnect/replay tests. E01 command matrix governs actual invocations.

Each result must record requirement/test ID, stimulus/settings/fixture hash, expected result and predeclared tolerance, actual result, commit, command/exit status and environment P/N/M/L. Preserve raw logs/CSV sufficient to reproduce the conclusion. Numerical and timing criteria are in VALIDATION_AND_EXPERIMENTS.md and the applicable behavior/model contract. Do not replace a required native interaction or listening test with a headless assertion.

## Measurements and experiments
Complete E04. Freeze toolbar drawing/warp limits from real algorithms before scoring. Resolve mobile HTTPS/origin design in a native deployment experiment. Native benchmark reference apps are optional for comparative claims, not substitutes for SML measurements.

## Human and external work
Native vocal editing walkthrough at 100/150/200% scale, shortcut use on an actual keyboard, real personal plugin controls and iOS/Android browser controls where available. Audition pitch-preserving Bend on vocals. Missing access leaves the respective claim pending; no routine sprint approval.

## Artifacts
Tool/shortcut coverage CSVs, command docs, skin schema/examples, menu trace distributions, route/slot/PDC tests, remote protocol/pairing guide and native workflow evidence.
Evidence root: `docs/evidence/next_cycle/sprint_02`. Large deterministic media may be kept outside the repository with source/fixture hashes and regeneration commands. Never include private vocals in public artifacts without permission.

## Documentation updates
Update only affected implementation docs, command knowledge, parameter/model dossiers, requirement ledger and current execution state. Preserve issued contracts and historical research; append explicit decision amendments instead of silently changing acceptance criteria. Record every approved model/interface change and any invalidated earlier evidence.

## Definition of Done
Every R10–R22 and R37 behavior functions through the shared engine and command service, with portable tests and available native evidence. G2 interfaces are validated. All required toolbar buttons are operative; required profile entries have actual actions. Mastering layout alone is not R34 completion. Source inspection, passing compilation and visible UI are not substitutes for behavior tests. Report missing tests and failures directly.

## Automatic continue gate
Continue to Sprint 3 modules when G2 host/parameter/render interfaces pass and their E05 actual interface and host-test requirements are established. Continue independent documentation or assistant read-only plumbing while an external native comparison remains pending. Do not ask for approval solely at this boundary. A blocker must identify the exact dependent task; unrelated authorized work continues.

## Stop conditions
Uncompensated route or duplicate DSP, changed legacy-session sound without migration, destructive edit, silent remote replay, invalid security assumptions, fake Bend, missing common command ownership or command loss under UI load. Apply the autonomy policy's external/destructive boundaries. Preserve current working state before reporting a blocker.

## Review
Inspect the actual diff for unintended source deletion, scope expansion, disabled tests, weakened tolerances, fake evidence, identifier/schema changes and source/license issues. Review real-time ownership and export/capture routes after any DSP/UI integration. Independent human listening/platform evidence stays pending until actually obtained; do not label a self-check as independent review.

## Commit, report and resume
Make a local milestone commit only from task-owned changes after applicable checks: `feat: complete SML Studio editing mixing and remote workflow`. If pre-existing failures or partial modules remain, disclose them and do not title the report a pass. Never merge main or push without separate authorization.

Write `docs/sprint_reports/NEXT_SPRINT_02_REPORT.md`: outcome, requirement statuses, files/commits, exact tests/evidence, failures/not-run, external gates, risks, elapsed runtime if known, and next safe task. Update `docs/next_cycle_execution/RESUME.md` before context/runtime exhaustion. Under the autonomous launcher continue per the gate; a standalone manual execution may hand off this report without claiming the whole cycle completed.
