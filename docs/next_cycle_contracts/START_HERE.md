# SML Studio — the next four sprints
Revision 2.2 • 2026-10-11 • Planning package, not an implementation report.

The owner supplied Claude’s original-cycle final report: branch `fable/recording-core`, final commit `396fd0a7a7aa5b62d8dae0e49b3413c7e2e3432b`, reportedly 616 passing tests. These claims and artifacts remain unverified by this author. Start from that actual checkpoint and reconcile its evidence; preserve the work. See [REPORTED_BASELINE](docs/REPORTED_BASELINE.md) for the remaining gaps and access rules.

The new cycle contains exactly four substantial, resumable sprint contracts:

| Sprint | Outcome | Why this order |
|---|---|---|
| 1 — Protect the session | Verify the current build; dependable capture, recovery, plugin lifecycle and shared command contracts | Every new feature depends on session and timing integrity |
| 2 — Finish the studio workflow | Vocal editing, separate Nudge, both shortcut profiles, both mixer views, fast themed UI and phone remote | Establish the actions and interfaces used by DSP tools and the assistant |
| 3 — Harden plugin integration | Personal plugins, native editors, automation, state, sidechains, PDC and future SML plugin compatibility | Keep the DAW ready for the plugins the owner develops individually |
| 4 — Complete the assisted studio | Expert text/voice assistant, engineering actions, mastering, integrated reliability and release evidence | Validate the complete record-to-master workflow |

These are scope milestones, not a promise to complete an entire professional DAW in six hours. The original six-hour allocation may already be spent. Work within the actual available runtime, checkpoint, and resume. A time limit never turns an incomplete feature into a pass.

Read [PROJECT_BRIEF](docs/PROJECT_BRIEF.md), [DECISIONS](docs/DECISIONS.md), [ROADMAP](docs/ROADMAP.md), then the [autonomous launcher](docs/AUTONOMOUS_BUILD_PROMPT.md). Feature coverage is in [REQUIREMENTS](docs/REQUIREMENTS.md). The launcher directs Claude through all four contracts without routine sprint approvals.

## Copy into Claude with this folder attached

> Execute this revision 2.2 package using docs/AUTONOMOUS_BUILD_PROMPT.md. Read docs/REPORTED_BASELINE.md first. Continue from the reported fable/recording-core checkpoint 396fd0a7a7aa5b62d8dae0e49b3413c7e2e3432b after verifying it and preserving user changes. I am away from my computer: work through the four new DAW contracts autonomously, fixing the carried software gaps and completing independent portable work while native hardware/phone/listening gates stay pending. Develop none of the seven individual plugins in this cycle. Do not silently restart from main if the unpushed checkpoint is unavailable. Checkpoint and report honestly; no routine sprint approval requests.

Status: Sprint 1 preflight is ready to execute. Downstream contracts are scoped drafts whose real build commands, platform capability and external plugin interfaces must be established by the named gates. This is deliberate: current implementation and proprietary sonic algorithms are not known from a prompt.

The original revision 1.7 technical specifications are preserved byte-for-byte in docs/foundation. [FOUNDATION_AUTHORITY](docs/FOUNDATION_AUTHORITY.md) explicitly supersedes their old timing, sprint numbering and plugin-development exclusions for this new cycle. Do not run their old sprint schedule.

Package check: `python3 tools/verify_package.py`. This checks document integrity and coverage only. It does not build or test the DAW.
