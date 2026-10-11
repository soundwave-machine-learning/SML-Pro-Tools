# Preserved foundation: authority and overrides

The twelve files in foundation/ are byte-identical copies of revision 1.7 technical research/specifications. They are requirements and historical research, not evidence that the prior build passed. Their provenance hashes are in PACKAGE_MANIFEST.json.

The following overrides are authoritative across every copied file:

1. Historical Sprint 1–4, A1–A4, H1–H4, six-hour deadline and evidence-folder assignments describe the old cycle. New ownership and paths come from this package. Retain behavioral test IDs as `F:<filename>:<test ID>` to avoid collision.
2. Native plugin creation, pitch correction, note editing and alignment algorithms are outside this DAW cycle by latest owner instruction. PROCESSOR_SPEC.md now governs hosting/integration only. Native Bend/time-edit services remain audio-editor scope. Custom summing/tape sound models remain deferred.
3. New work is not assumed absent merely because a historical audit said so. Inspect the current commit. Do not replace completed components without a demonstrated gap.
4. Foundation files define actual sample math, slot order, route semantics, pan profiles, monitor compensation, vocal editing and toolbar behavior unless a specific new decision supersedes them. Separately developed plugins enter through their actual supported host/clip interfaces, never hidden always-on mixer stages.
5. Source versions remain pinned. A later vendor successor manual is a separately identified reference, not proof of exact Studio One 7 behavior. Unconfirmed keybindings stay gaps; never guess.
6. Existing target values such as clean monitoring <=10 ms, <=1-sample deterministic path alignment and p95 <=50 ms response remain goals, not observations. New validation strengthens recovery, load and native evidence; it does not retroactively certify the old cycle.
7. Automatic continuation and permission rules are governed by AUTONOMOUS_BUILD_PROMPT.md. Any copied instruction to stop after a manual sprint does not impose a routine approval loop on the autonomous run.

Read each foundation file only when its subsystem is touched; log coverage in the reconciliation report. Useful links: [clock](foundation/CLOCK_LATENCY_SPEC.md), [Nudge](foundation/NUDGE_SPEC.md), [vocal edits](foundation/VOCAL_EDITING_SPEC.md), [toolbar](foundation/EDIT_TOOLBAR_SPEC.md), [buses](foundation/BUS_WORKFLOW_SPEC.md), [mixer profiles](foundation/MIXER_MODES_SPEC.md), [slots](foundation/MIXER_SLOT_SPEC.md), [visual mixer](foundation/VISUAL_MIXER_SPEC.md).
