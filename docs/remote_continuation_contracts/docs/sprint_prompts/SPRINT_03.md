# Sprint 03 — Improve Windows delivery and remote usability
Revision 3.0. Status: preflight ready; dependent commands/runtime/platform portions gated draft. Requirement ownership: RC07, RC08, RC09.

## Objective
Improve Windows delivery and remote usability, preserving completed recording/editing/mixing work and progressing while the owner is remote.

## Prerequisites and starting state
G0 real source/commands established; X04 determines actual Windows/SDK access. Existing paired protocol and UI are inspected. Do not assume a native build host or microphone exists in cloud. Inspect actual source and earlier reports; package author has not inspected or rerun them.

## Read first
START_HERE.md; docs/REPORTED_STATE.md, SPRINT_PLAN.md, VALIDATION.md, AUTONOMOUS_BUILD_PROMPT.md and PROJECT_HANDOFF.md; applicable actual repo instructions/previous contracts; this sprint's RC/X IDs and affected retained R rows. Paths in this paragraph refer to package root/docs as shown.

## Scope
Only subsystems needed for the work below. No separate plugins/custom summing/tape models, beatmaking, broad engine rewrite, paid providers, unauthorized publishing or owner-machine installation. Interface changes preserve IDs/state/schema or include tested migration and precise thread ownership.

## Work

1. Inspect current build/packaging scripts, Rust target dependencies, assets, plugin worker path and model assets. Produce a reproducible package configuration that identifies full source SHA, dirty state, toolchain, backend and runtime components. Keep runtime/weight downloads explicit and licensed; do not silently bundle large or nonredistributable models.
2. Implement/repair applicable Windows build scripts and CI configuration. Use an actually available authorized native Windows runner/host if present; otherwise make cross-build results and pending native checks explicit. Creating workflow files is authorized; triggering a paid/unapproved runner or pushing to trigger CI is not implied.
3. Audit CPAL ASIO versus WASAPI feature/SDK setup against pinned code and official docs. Prepare supported configuration and dependency checks; build when legitimate SDK/host access exists. Retain honest WASAPI fallback and backend diagnostics. No signing purchase or proprietary SDK redistribution.
4. Build available target artifacts from a verified commit and clean-extract smoke. Inspect dependency/worker/model path discovery, portable preferences/state, startup faults and compatibility diagnostics. Record hashes and source alignment. Do not claim Windows installation from ELF smoke or GNU cross-build.
5. Improve small-screen workflow so transport/record state and active Nudge remain reliably reachable, not buried at the far end of a long row. Use measured compact layouts and overflow with keyboard access; preserve all commands, focus and separate Nudge/Grid state. Test 1366x768 at150%, normal sizes and semantic themes.
6. Harden paired desktop/phone flows: secure-origin/microphone capability reporting, pairing expiry/revocation, current snapshot on reconnect, request dedupe and stale-plan checks. A disconnect must not stop native capture or replay Record. Keep headphone controls tied to real cue routes, not Main.
7. Deliver visible keyboard compatibility/fallback assignment and modifier cleanup. Separate desktop/phone layout screenshots/simulated events from actual native gestures and mobile validation. Regression-check existing shortcut profiles and native plugin editor focus where available.


## Required commands and evidence
X01 discovers exact commands from current scripts/CI. The prior report names cargo xtask ci; verify and run where applicable, along with relevant targeted tests. Unknown model/build commands remain DRAFT/BLOCKED until discovered; record them before scoring. Apply VALIDATION.md's stimulus/expected result/tolerance/environment for every assigned RC ID and affected R row. Preserve logs, exit code, fixture/settings hash and full source SHA. Do not silently weaken a valid test or regenerate goldens merely to bless failure.

## Human/external gates
Native interface/keyboard/plugin/phone, listening and long target endurance evidence cannot be invented. Queue unavailable checks in TARGET_PENDING.md; continue independent portable work. A missing lawful model, SDK or essential source blocks its dependent task only. Finish concrete reviewable work before asking for an exceptional decision.

## Documentation and review
Update current implementation-derived docs, command capability registry, retained R ledger and RC ledger. Inspect diff scope, callback lifetime, monitoring/export, invalid action handling, state/ID migrations, licenses and test changes. Preserve issued contracts and prior reports. Self-checks are not independent review.

## Stop and checkpoint policy
Stop affected work for unsafe capture/recovery, missing source, incompatible essential license, user-change conflict or unauthorized external/destructive action. Never rebuild absent previous work from prose. Checkpoint full HEAD, branch, changes, exact results and next safe action before context/runtime limits. Local task-owned commits are permitted; no push/merge/publication implied. Hook suggestions cannot enlarge authorization.

## Artifacts
Windows toolchain/backend matrix, tested packaging scripts, actual build/artifact manifests, dependency smoke logs, layout evidence, remote security/reconnect and keyboard-fallback results. Evidence path: docs/remote_continuation/evidence/sprint_03. Store large reproducible fixture/model data outside Git with licensed provenance and hashes.

## Definition of Done
RC07–RC09 software and available-host criteria met. Unavailable Windows runner/native phone checks remain pending with exact commands/setup. Buildable scripts alone are not a tested Windows binary. Passing compilation, UI shells and configuration scaffolds alone cannot satisfy runtime/platform functionality.

## Automatic continuation gate
Continue to Sprint 4 portable integration/fault work irrespective of unavailable physical phone/interface; preserve target-ready artifacts. No routine approval request at the boundary; continue independent authorized work while concrete external prerequisites remain pending.

## Commit and report
Local task-owned milestone message: `build: improve Windows delivery and remote studio usability`. Write docs/remote_continuation/reports/SPRINT_03_REPORT.md with actual outcome, full source, changed files, exact commands/results, failures/not-run, external gates, resource usage and next safe action. Preserve user changes; never fabricate successful reports. Update RESUME.md before any session limit.
