# Document authority and reading order — revision 1.6
All package paths are relative to the extracted package/repository root; sibling spec names inside docs refer to docs/. Sprint references to source crates apply to the target repository, not this planning ZIP.

## First read
1. docs/PROJECT_BRIEF.md — owner scope and exclusions.
2. docs/PROMPT_AUDIT.md — audit findings, fixes and remaining readiness limits.
3. docs/PROJECT_HANDOFF.md — known baseline and unexecuted work.
4. docs/SOURCE_AUDIT.md — historical inspected source, not current runtime proof.
5. docs/REQUIREMENTS_MATRIX.md — R01–R26 traceability.
6. docs/SPRINT_PLAN.md and docs/VALIDATION_PLAN.md — sequence and gate ownership.
7. docs/AUTONOMOUS_BUILD_PROMPT.md — single execution entrypoint.

## Topic authorities
| Domain | Authoritative behavior | Supporting research |
|---|---|---|
| Clock, live buffering, placement, compensation | docs/CLOCK_LATENCY_SPEC.md | docs/CLOCK_LATENCY_RESEARCH.md |
| Vocal tools and edits | docs/VOCAL_EDITING_SPEC.md | docs/REFERENCE_BEHAVIOR.md |
| Nudge semantics and key mapping | docs/NUDGE_SPEC.md | docs/REFERENCE_BEHAVIOR.md |
| Bus creation, mute/solo, routing graph | docs/BUS_WORKFLOW_SPEC.md | docs/REFERENCE_BEHAVIOR.md |
| Linear summing/pan/legacy mix versions | docs/MIXER_MODES_SPEC.md | docs/MIXER_SOUND_RESEARCH.md |
| Slot and insert order, send taps, export, bypass | docs/MIXER_SLOT_SPEC.md | docs/LUNA_HARRISON_WORKFLOW_RESEARCH.md |
| Two synchronized mixer views, width and snapshots | docs/VISUAL_MIXER_SPEC.md | reference URL in that file |

Exact processing order is owned ONLY by SL02; other documents reference it. M02 owns panner math; VM03 owns native width math. Validation defines portable/native/claim/deferred gate classification and shared measurement conventions. Changelog and research do not mandate old designs. Tests not present in source must be implemented and assigned real commands during execution; no fictional test entrypoint is a blocker before preflight has attempted to establish it.

## Exactly four contracts
- docs/sprint_prompts/SPRINT_01.md
- docs/sprint_prompts/SPRINT_02.md
- docs/sprint_prompts/SPRINT_03.md
- docs/sprint_prompts/SPRINT_04.md

## Package provenance
CHANGELOG.md, PACKAGE_MANIFEST.json, tools/verify_prompt_package.py. The verifier checks package hashes, document links, sprint count and some stale wording; it does not validate audio code or prove semantic completeness. No completed sprint reports ship in this planning package.
