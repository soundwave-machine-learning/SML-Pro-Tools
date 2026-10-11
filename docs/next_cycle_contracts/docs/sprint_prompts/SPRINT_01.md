# Sprint 01 — Protect the session

Contract revision 2.2 • Owner scope authorization: latest request for all ideas in four prioritized prompts. Status: PREFLIGHT READY; implementation gated by E01.
Authoritative package file: docs/sprint_prompts/SPRINT_01.md. Requirement ownership: R01, R02, R03, R04, R05, R06, R07, R08, R09.

## Objective
Make the existing recording foundation trustworthy and establish the interfaces that every later feature will use.

## Prerequisites and current evidence
The owner has authorized the new cycle, but the reported checkpoint is `396fd0a7a7aa5b62d8dae0e49b3413c7e2e3432b` on `fable/recording-core`. The narrative report is supplied; actual checkout/raw logs/artifacts still need verification. G0/E01 is the first executable task. No previous product PASS is assumed. No product test result is asserted by this contract.

## Read first
Read package-root START_HERE.md, then PROJECT_BRIEF.md, DECISIONS.md, FOUNDATION_AUTHORITY.md, ROADMAP.md, REQUIREMENTS.md, VALIDATION_AND_EXPERIMENTS.md and AUTONOMOUS_BUILD_PROMPT.md. Then read RELIABILITY_AND_ENGINE.md; ARCHITECTURE.md; all foundation files needed to reconcile the original work, especially CLOCK_LATENCY_SPEC.md and MIXER_SLOT_SPEC.md. Sources S01, S17–S21; decisions D01–D03, D09, D11–D12. Paths are relative to the package docs directory unless stated otherwise.

## Scope and interfaces
Repository audit, real-time engine/capture, session persistence, plugin hosting/scanner/supervision, command/parameter schemas, safe loading and test instrumentation. Preserve existing audio/session architecture. No beatmaking, native summing/tape models or broad UI rewrite. Interface changes must preserve versioned session/parameter IDs or include a tested migration; record actual ownership/thread rules in the execution dossier. Unrelated repository changes are out of scope.

## Carry-forward priorities from the original cycle
Read REPORTED_BASELINE.md. Inspect and preserve reported completed work rather than implementing it again. Close B01 structural callback allocation/lifetime hazards; B02 unexplained 64-frame overruns; B03 keyboard location/shortcut gaps; B04 misleading nonfunctional recording toggles; B09 unverified recovery/runtime containment. Plan native B05–B07 without requesting the absent owner to run hardware tests now. Do not reintroduce old plugins to satisfy B08.

B01 supports two honest outcomes: full safe live structural edits, or a clearly enforced stopped-only boundary while off-thread graph preparation is completed. All GUI/shortcut/remote/assistant paths must respect that boundary. A stopped-only interim policy cannot be reported as full live-edit support.

B04 is a UI honesty fix: unavailable QuickPunch/TrackPunch/destructive modes are disabled/labeled, not deleted from legacy session state. This sprint does not authorize implementing destructive recording algorithms.

## Work in dependency order

1. Run E01. Protect user changes, inventory actual code and reports, discover/build with real CI commands, and record baseline failures. Map the old cycle's foundation tests to actual implementations. Establish the execution ledger and exact command matrix before production changes.
2. Correct demonstrated capture/clock/routing/PDC defects that threaten the foundation. Audit audio-thread allocations, locks, disk I/O, object teardown, queue overflow and device negotiation. Preserve dry input and exact session placement. Add targeted tests for concrete missing invariants, not redundant implementation-mirroring tests.
3. Implement project snapshot autosave and separate media commit/recovery. Run E02 with forced termination at write/flush/commit boundaries, full disk and partial-file recovery. Make missing or stale plugin checkpoint state explicit. Keep previous snapshots and original media untouched.
4. Complete VST3 scanner isolation/cache/quarantine and retain supported CLAP; verify native editor/state lifecycle where the target platform is available. Add Safe Open and diagnostic recovery paths. Verify old bundled processors do not reappear and legacy placeholders round-trip.
5. Run E03 before claiming runtime plugin crash containment. Implement the accepted worker route only if it meets the measured deadline, state/editor and latency contract. Record added IPC samples in the graph ledger. Keep dry capture independent of a failed plugin and prevent silent substituted exports.
6. Freeze the actual shared command/parameter/session/graph/analysis interfaces. Check stale/duplicate actions, bounded queues and transactional undo. Populate real command knowledge fields; no AI model is required for this schema work.
7. Start E05/E06 feasibility early: find a usable pitch-preserving renderer for Sprint 2 Bend, native warp dependency licenses and external plugin interfaces, actual model/STT/TTS options and required reference access. Capture decisions and blocked dependencies. The seven plugin builds are external owner projects; do not build their DSP or UI shells.
8. Establish E04 native profiling fixtures and E07 meter/export research plan. Save native hardware requirements and existing capability gaps so later contracts can be frozen without guessing.


## Required build and test execution
Run V-BASELINE, V-CAPTURE, V-CLOCK, V-RECOVERY, V-RELIABILITY, V-HOST, V-ISOLATION, V-LEGACY and V-COMMANDS. Use foundation CL-T1–5 portable cases plus applicable native CL-T6–7. Recovery must preserve every acknowledged committed frame; malformed state must not destroy the previous valid session. Scanner faults must not kill the host. Runtime containment is a separate tested result. The prior report names `cargo xtask ci`; verify its current implementation and execute where applicable. Other actual tool command strings come from E01 and are stored before each invocation. Extend steady-state callback guards to structural-mutation cases; do not use the steady-state test as proof of safe graph changes.

Each result must record requirement/test ID, stimulus/settings/fixture hash, expected result and predeclared tolerance, actual result, commit, command/exit status and environment P/N/M/L. Preserve raw logs/CSV sufficient to reproduce the conclusion. Numerical and timing criteria are in VALIDATION_AND_EXPERIMENTS.md and the applicable behavior/model contract. Do not replace a required native interaction or listening test with a headless assertion.

## Measurements and experiments
E01 and E02 required for G0/G1; E03 required for runtime-isolation claim. E05 renderer feasibility required before Sprint 2 Bend can complete. E06 provider discovery is early risk reduction. Missing Windows/interface access leaves native capture/host claims pending.

## Human and external work
Actual interface loopback, native plugin editors and recovery walkthrough on the target system. The owner need not approve ordinary architecture decisions. Missing hardware blocks native readiness, not independent portable work.

## Artifacts
Baseline/reconciliation report, real command matrix, current requirements ledger, interface contracts, recovery protocol/fixtures, scan/runtime host matrix, callback/latency measurements, E01–E06 decision entries and first checkpoint.
Evidence root: `docs/evidence/next_cycle/sprint_01`. Large deterministic media may be kept outside the repository with source/fixture hashes and regeneration commands. Never include private vocals in public artifacts without permission.

## Documentation updates
Update only affected implementation docs, command knowledge, parameter/model dossiers, requirement ledger and current execution state. Preserve issued contracts and historical research; append explicit decision amendments instead of silently changing acceptance criteria. Record every approved model/interface change and any invalidated earlier evidence.

## Definition of Done
All R01–R09 are implemented and evidenced at their applicable scope, with runtime sandbox result and any platform restriction explicit. G1 capture/persistence/command invariants pass before new mutation pathways. Do not call this whole sprint complete if mandatory runtime containment remains unresolved; independent Sprint 2 work may proceed with that limitation recorded. Source inspection, passing compilation and visible UI are not substitutes for behavior tests. Report missing tests and failures directly.

## Automatic continue gate
Continue directly to Sprint 2 tasks whose G1 prerequisites pass. Any known data-loss, invalid command transaction or timing defect blocks affected dependent tasks until repaired. Do not ask for approval solely at this boundary. A blocker must identify the exact dependent task; unrelated authorized work continues.

## Stop conditions
Unprotected user changes; undiagnosed capture corruption; recovery overwriting originals; unresolved build baseline; unsafe callback or worker wait; incompatible essential dependency. Apply the autonomy policy's external/destructive boundaries. Preserve current working state before reporting a blocker.

## Review
Inspect the actual diff for unintended source deletion, scope expansion, disabled tests, weakened tolerances, fake evidence, identifier/schema changes and source/license issues. Review real-time ownership and export/capture routes after any DSP/UI integration. Independent human listening/platform evidence stays pending until actually obtained; do not label a self-check as independent review.

## Commit, report and resume
Make a local milestone commit only from task-owned changes after applicable checks: `feat: establish recoverable SML Studio foundation`. If pre-existing failures or partial modules remain, disclose them and do not title the report a pass. Never merge main or push without separate authorization.

Write `docs/sprint_reports/NEXT_SPRINT_01_REPORT.md`: outcome, requirement statuses, files/commits, exact tests/evidence, failures/not-run, external gates, risks, elapsed runtime if known, and next safe task. Update `docs/next_cycle_execution/RESUME.md` before context/runtime exhaustion. Under the autonomous launcher continue per the gate; a standalone manual execution may hand off this report without claiming the whole cycle completed.
