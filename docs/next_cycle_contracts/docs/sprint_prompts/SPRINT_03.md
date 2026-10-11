# Sprint 03 — Harden plugin integration

Contract revision 2.2. Status: SCOPED DRAFT; freeze actual commands and G2/E05 integration prerequisites. Owner scope: the seven plugins are created individually outside the DAW cycle.
Authoritative package file: docs/sprint_prompts/SPRINT_03.md. Requirement ownership: R23, R24, R25, R26, R27, R28, R29, R30, R31.

## Objective
Make SML Studio a reliable host for personal and separately developed SML plugins, with correct timing, native editors, automation, persistence and offline rendering.

## Prerequisites and current evidence
G2 shared commands/parameters/routing/edit-render interfaces pass. E01 discovered actual build/test commands. E05 identifies the real native warp service and supplied plugin interfaces. No current product/plugin test result is asserted. Future plugin binaries are not assumed available.

## Read first
Package-root START_HERE.md; docs/DECISIONS.md, FOUNDATION_AUTHORITY.md, ARCHITECTURE.md, REQUIREMENTS.md, PROCESSOR_SPEC.md, RELIABILITY_AND_ENGINE.md and VALIDATION_AND_EXPERIMENTS.md; foundation mixer-slot and clock specs; earlier actual execution reports. INDIVIDUAL_PLUGIN_BRIEF.md is reference-only external-project planning and cannot authorize development in this sprint.

## Scope and interfaces
VST3/working CLAP lifecycle, editor focus, parameter/state/automation, sidechains, graph compensation, render jobs, host diagnostics and supplied-plugin compatibility adapters based on observed interfaces. Preserve schema/IDs or implement tested migration. Do not create EQ, gate, compressor, optical, autotune, note-editor or alignment DSP; custom summing/tape models also remain deferred. No speculative ARA/new-format implementation.

## Work in dependency order
1. Inventory actual host capabilities and available personal plugins. Build a capability matrix for formats, channel/input layouts, sidechains, parameters, editors, state, tails, latency and rendering. Distinguish deterministic host fixtures from actual native plugin tests.
2. Fix demonstrated scan/load/editor/focus/close/unload/reload defects. Validate simultaneous instances and switching between both mixer views. No scan/plugin creation on the audio callback or menu-opening path.
3. Verify automation discovery and stable parameter IDs; numeric units, gestures, state/presets, save/reopen, missing-plugin placeholders and version changes. The assistant receives only documented actual capabilities. Do not assume a vendor label identifies a parameter.
4. Test external sidechains, multi-input routing, latency changes and parallel/nested PDC. Include Summing/Tape slots and ordinary inserts; every provider is processed once at its declared node. Bypass preserves compensation; unsafe quality changes require explicit safe transitions.
5. Harden offline rendering, tail handling, errors and deterministic alignment. Exports use current state and edit-render revision. A plugin failure cannot silently yield a substituted master. Keep nondeterministic DSP distinctions in the evidence.
6. Complete accepted runtime-isolation/compatibility mode work from E03. Exercise plugin crash/hang and editor failure while preserving dry capture. Measure added IPC samples, callback deadlines and memory. Scan isolation alone does not count as playback containment.
7. Harden native Bend/time-edit rendering and bounded analysis/cache queues, stale-result rejection, cancellation, immutable media and undo/reopen. This is the DAW's timing service, not an invitation to create the external pitch/editor/aligner products.
8. Assess supplied SML plugin artifacts individually using PROCESSOR_SPEC.md. If none are available, record AWAITING_EXTERNAL_PLUGIN and complete generic hosting tests with available real personal plugins. For clip-aware editors/aligners, report the actual needed integration mechanism rather than claiming VST3 provides it automatically. Do not block the DAW on future product development.
9. Repeat W1 load tests with a documented real chain, native editors open, menu activity, automation and autosave. Profile stalls and regressions. Fix host defects before adding assistant control in Sprint 4.

## Required build and test execution
Use E01's real command matrix. Run V-EXTERNAL_PLUGIN, V-EDIT_RENDER and V-HOST_EVIDENCE plus affected V-HOST/ISOLATION/CLOCK/SLOTS/RECOVERY cases. Stimuli include load/reload/editor focus, rapid parameters, missing state, sidechain impulses, 0/1/63/128/1024-sample delay fixtures, callback block/rate changes, canceled/stale render and plugin crash/hang. Deterministic PDC aligns within one sample; source media hashes remain unchanged; no duplicate processing, invalid callback operations or silently substituted export. Native W1 timing/headroom/xrun targets remain as specified in VALIDATION_AND_EXPERIMENTS.md.
Record requirement/test ID, actual command/exit code, fixture/plugin hash/version/settings, commit, platform and evidence path. A future binary can only receive compatibility PASS after a real run; generic fixtures establish host plumbing, not that binary's sound or behavior.

## Measurements and experiments
E05: actual warp dependency and plugin interface/license/capability assessment. E03: runtime isolation with measured delay/CPU/editor/state limitations. E04/E08: affected menu and recording performance. Plugin model/sonic research belongs to separate individual projects and is not asserted here.

## Human and external work
Actual native personal-plugin editor workflow, input/sidechain routing and recording/monitor tests on the target interface. Supplied external binaries are optional inputs for named compatibility tests; their absence is not a request to build them. Missing native hardware leaves native readiness pending while portable work continues.

## Artifacts
Host capability/compatibility CSV; state/editor/PDC/sidechain/render logs; callback/latency/load distributions; runtime-fault recovery evidence; native timing-render tests; seven conditional external-project integration records. Evidence root: docs/evidence/next_cycle/sprint_03. Large reproducible media gets hashes and regeneration instructions; no private audio in public artifacts.

## Documentation updates
Update actual plugin help/capabilities, execution ledger, host setup guide, latency diagnostics and integration limitations. Preserve separate-project ideas in INDIVIDUAL_PLUGIN_BRIEF.md. Do not edit it into a DAW build instruction or claim research completion.

## Definition of Done
Generic hosting and native timing/render behavior pass applicable tests, real available plugins are validated on the target environment where accessible, and every future target has a truthful interface/status record. R23–R29 are conditional integration targets, not seven mandatory binary deliveries. Missing external products do not block DAW completion; missing host behavior or required native evidence does. No plugin-development work is included.

## Automatic continue gate
Continue directly to Sprint 4 when G3 generic host/parameter/state/PDC/render gates pass. Assistant recipes can use only actual registered compatible plugins. Continue independent read-only assistance/mastering work while external native evidence is pending, with overall readiness labeled accurately. No routine approval is needed between sprints.

## Stop conditions
Data loss, unsafe callback wait/teardown, undocumented delay, changed source media, fake compatibility results, missing essential host/interface license, destructive user-change conflict or a need for an unprovided proprietary SDK. Stop only affected dependent work and preserve a checkpoint; do not solve a missing external plugin by writing it.

## Review
Inspect the diff for bundled DSP restoration, accidental plugin creation, schema/ID changes, weakened tests, fake evidence, route duplication and unrelated edits. Check real-time ownership and monitor/export separation. Independent platform evidence is pending until actually obtained.

## Commit, report and resume
Local milestone message: `feat: harden SML Studio plugin hosting and integration`. No push/merge/publication. Write docs/sprint_reports/NEXT_SPRINT_03_REPORT.md with changed files/commits, actual tests, unresolved host/native gates and AWAITING_EXTERNAL_PLUGIN entries. Update docs/next_cycle_execution/RESUME.md before runtime/context exhaustion and continue per the gate.
