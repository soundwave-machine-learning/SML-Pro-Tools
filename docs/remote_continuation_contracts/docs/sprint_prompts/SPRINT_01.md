# Sprint 01 — Close the remaining software gaps
Revision 3.0. Status: preflight ready; dependent commands/runtime/platform portions gated draft. Requirement ownership: RC01, RC02, RC03.

## Objective
Close the remaining software gaps, preserving completed recording/editing/mixing work and progressing while the owner is remote.

## Prerequisites and starting state
G0 is established by X01 before edits. The report through 9f9b4d3 is a starting lead, not evidence inspected by this package. No prior PASS is assumed for known omitted gaps. Inspect actual source and earlier reports; package author has not inspected or rerun them.

## Read first
START_HERE.md; docs/REPORTED_STATE.md, SPRINT_PLAN.md, VALIDATION.md, AUTONOMOUS_BUILD_PROMPT.md and PROJECT_HANDOFF.md; applicable actual repo instructions/previous contracts; this sprint's RC/X IDs and affected retained R rows. Paths in this paragraph refer to package root/docs as shown.

## Scope
Only subsystems needed for the work below. No separate plugins/custom summing/tape models, beatmaking, broad engine rewrite, paid providers, unauthorized publishing or owner-machine installation. Interface changes preserve IDs/state/schema or include tested migration and precise thread ownership.

## Work

1. Resolve actual SHA/source, protect newer changes, read earlier final handoff/logs and audit all 38 rows, especially twelve PARTIAL entries. Record actual source/test links and missing scope, not just counts. Separate absence of proof from a reproduced failure.
2. Inspect structural add/remove/reorder/re-route/plugin changes and session replacement under active playback/recording. Prepare and retire graph resources off callback with bounded safe publication. Extend allocation/deallocation/lock/I/O guards beyond steady state. If full live structural mutation is unsafe, enforce a visible stopped-only policy consistently across GUI/shortcuts/phone/assistant; keep full live editing partial.
3. Retain previous 64-frame overrun data and measure callback compute against N/Fs, interval jitter, deadlines and queues. Reproduce with declared cloud settings. Investigate with profiler evidence; no unsupported VM-only diagnosis and no silent buffer change/threshold relaxation. Native cause/acceptance can remain pending.
4. Verify one-block-ahead sandbox ownership, variable-block delay, PDC ledger and live path. Inject crash/hang, retained state, stale restart/transport generation, recording restart suppression and export refusal. Never wait indefinitely for a dead process or destroy host objects on callback.
5. Inspect actual physical-key events and supported backend updates. Implement keypad distinction through a lawful compatible native event path where feasible; otherwise expose honest remappable alternatives and queue native checks. Do not conflate top-row +/- with Num+/- or intercept global system shortcuts.
6. Disable/label nonfunctional QuickPunch/TrackPunch/destructive modes consistently while retaining legacy state; no destructive-mode implementation without its own contract. Verify Count Off/device-loss/timestamp fixes remain intact.
7. Verify real autosave/media durability/Safe Open and scanner isolation status rather than infer from CI totals. Correct concrete missing/error paths and seed fault fixtures used in Sprint 4. Start X03/X04 discovery to avoid late blocked dependencies.


## Required commands and evidence
X01 discovers exact commands from current scripts/CI. The prior report names cargo xtask ci; verify and run where applicable, along with relevant targeted tests. Unknown model/build commands remain DRAFT/BLOCKED until discovered; record them before scoring. Apply VALIDATION.md's stimulus/expected result/tolerance/environment for every assigned RC ID and affected R row. Preserve logs, exit code, fixture/settings hash and full source SHA. Do not silently weaken a valid test or regenerate goldens merely to bless failure.

## Human/external gates
Native interface/keyboard/plugin/phone, listening and long target endurance evidence cannot be invented. Queue unavailable checks in TARGET_PENDING.md; continue independent portable work. A missing lawful model, SDK or essential source blocks its dependent task only. Finish concrete reviewable work before asking for an exceptional decision.

## Documentation and review
Update current implementation-derived docs, command capability registry, retained R ledger and RC ledger. Inspect diff scope, callback lifetime, monitoring/export, invalid action handling, state/ID migrations, licenses and test changes. Preserve issued contracts and prior reports. Self-checks are not independent review.

## Stop and checkpoint policy
Stop affected work for unsafe capture/recovery, missing source, incompatible essential license, user-change conflict or unauthorized external/destructive action. Never rebuild absent previous work from prose. Checkpoint full HEAD, branch, changes, exact results and next safe action before context/runtime limits. Local task-owned commits are permitted; no push/merge/publication implied. Hook suggestions cannot enlarge authorization.

## Artifacts
Callback mutation guard logs, source/gap ledger, sandbox latency/fault evidence, keyboard/toggle capability notes, recovery status and actual command matrix. Evidence path: docs/remote_continuation/evidence/sprint_01. Store large reproducible fixture/model data outside Git with licensed provenance and hashes.

## Definition of Done
RC01–RC03 meet software criteria for supported operations, with unresolved native/performance gaps explicit. Unsafe capture/transaction/callback paths block dependent assistant changes. A disclosed stopped-only policy does not claim full live structural edits. Passing compilation, UI shells and configuration scaffolds alone cannot satisfy runtime/platform functionality.

## Automatic continuation gate
Proceed to Sprint 2 safe runtime work after G1; independent build/model discovery may continue earlier. No routine approval request at the boundary; continue independent authorized work while concrete external prerequisites remain pending.

## Commit and report
Local task-owned milestone message: `fix: close recording-core software safety gaps`. Write docs/remote_continuation/reports/SPRINT_01_REPORT.md with actual outcome, full source, changed files, exact commands/results, failures/not-run, external gates, resource usage and next safe action. Preserve user changes; never fabricate successful reports. Update RESUME.md before any session limit.
