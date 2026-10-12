# Sprint 02 — Complete the actual assistant runtime
Revision 3.0. Status: preflight ready; dependent commands/runtime/platform portions gated draft. Requirement ownership: RC04, RC05, RC06.

## Objective
Complete the actual assistant runtime, preserving completed recording/editing/mixing work and progressing while the owner is remote.

## Prerequisites and starting state
G1 safe command/capture boundaries pass. X03 selects actual model/STT/TTS assets and verifies code/weights/voice licenses and execution feasibility before shipping claims. Runtime integration is a required goal, not assumed available. Inspect actual source and earlier reports; package author has not inspected or rerun them.

## Read first
START_HERE.md; docs/REPORTED_STATE.md, SPRINT_PLAN.md, VALIDATION.md, AUTONOMOUS_BUILD_PROMPT.md and PROJECT_HANDOFF.md; applicable actual repo instructions/previous contracts; this sprint's RC/X IDs and affected retained R rows. Paths in this paragraph refer to package root/docs as shown.

## Scope
Only subsystems needed for the work below. No separate plugins/custom summing/tape models, beatmaking, broad engine rewrite, paid providers, unauthorized publishing or owner-machine installation. Interface changes preserve IDs/state/schema or include tested migration and precise thread ownership.

## Work

1. Inspect existing grounded assistant, atomic action service and provider seams. Preserve validated actions; identify exactly which current answers/actions are deterministic and which generative modules are absent. A 20/20 fixed evaluation remains its own reported result.
2. Complete X03. Prefer a lawful no-cost/local route compatible with the existing implementation and available hardware. Pin runtime/model/voice versions, provenance, separate licenses, acquisition paths, memory requirements and privacy policy. Do not require paid API use, hidden credentials or owner audio upload.
3. Integrate a real text model through bounded cancellable workers and version-grounded retrieval. Verify actual inference on available environment. Give the model only typed checked action proposals; validate current IDs/revisions/context at application time. Ambiguous/unsupported actions fail safely or ask a focused target question; no arbitrary shell tool.
4. Integrate actual STT and TTS for push-to-talk using licensed recorded/synthetic fixtures in cloud. Connect transcription -> visible transcript -> validated plan; cancellation/focus/stop owns the correct request. Speech output follows the existing monitor-only path, defaults silent while recording and is excluded from capture/export internally.
5. Finish desktop/paired-phone runtime UI: loading/model availability/privacy choices, text fallback, cancel/progress/timeouts and error explanations. Phone microphone/secure-origin/native behavior remain physical gates. No always-listening wake-word scope.
6. Freeze held-out varied/unseen requests and speech cases before tuning. Test grounding/version mismatch, prompt injection, missing tools, malformed/stale/duplicate commands, wrong-track ambiguity and partial-plan rollback. Include context/model limits, offline/download failure, model crash and multi-request backpressure.
7. Measure observed answer/action/STT errors, time-to-response, peak memory and background impact with declared hardware/model. Tune resource admission while preserving recording. A real but unusably slow model remains partial. If lawful runtime/model acquisition fails, implement real seams/config/error handling and document the precise missing dependency; never count a fake model as completion.


## Required commands and evidence
X01 discovers exact commands from current scripts/CI. The prior report names cargo xtask ci; verify and run where applicable, along with relevant targeted tests. Unknown model/build commands remain DRAFT/BLOCKED until discovered; record them before scoring. Apply VALIDATION.md's stimulus/expected result/tolerance/environment for every assigned RC ID and affected R row. Preserve logs, exit code, fixture/settings hash and full source SHA. Do not silently weaken a valid test or regenerate goldens merely to bless failure.

## Human/external gates
Native interface/keyboard/plugin/phone, listening and long target endurance evidence cannot be invented. Queue unavailable checks in TARGET_PENDING.md; continue independent portable work. A missing lawful model, SDK or essential source blocks its dependent task only. Finish concrete reviewable work before asking for an exceptional decision.

## Documentation and review
Update current implementation-derived docs, command capability registry, retained R ledger and RC ledger. Inspect diff scope, callback lifetime, monitoring/export, invalid action handling, state/ID migrations, licenses and test changes. Preserve issued contracts and prior reports. Self-checks are not independent review.

## Stop and checkpoint policy
Stop affected work for unsafe capture/recovery, missing source, incompatible essential license, user-change conflict or unauthorized external/destructive action. Never rebuild absent previous work from prose. Checkpoint full HEAD, branch, changes, exact results and next safe action before context/runtime limits. Local task-owned commits are permitted; no push/merge/publication implied. Hook suggestions cannot enlarge authorization.

## Artifacts
Actual model/voice manifests and runtime smoke outputs, frozen held-out evaluation/results, speech-routing proof, resource/cancel evidence and current provider capability status. Evidence path: docs/remote_continuation/evidence/sprint_02. Store large reproducible fixture/model data outside Git with licensed provenance and hashes.

## Definition of Done
Real text inference and real STT/TTS pipeline observed under declared environment, with checked action/no-leak/cancel gates. If unavailable, RC04/05 remain BLOCKED/PARTIAL explicitly; unrelated later work proceeds. Passing compilation, UI shells and configuration scaffolds alone cannot satisfy runtime/platform functionality.

## Automatic continuation gate
Continue to Sprint 3 packaging and UI work; never expose invalid unsafe plans merely to finish voice. No routine approval request at the boundary; continue independent authorized work while concrete external prerequisites remain pending.

## Commit and report
Local task-owned milestone message: `feat: integrate actual grounded assistant and voice runtime`. Write docs/remote_continuation/reports/SPRINT_02_REPORT.md with actual outcome, full source, changed files, exact commands/results, failures/not-run, external gates, resource usage and next safe action. Preserve user changes; never fabricate successful reports. Update RESUME.md before any session limit.
