# Architecture and interface contracts

This specifies boundaries to inspect and adapt to existing code; it is not permission for a rewrite or a claim the repository already implements them.

## One session, one engine, several clients
GUI, shortcuts, LAN remote and assistant submit typed commands to the same application command service. A command has stable ID/version, typed arguments/units, explicit stable object IDs, expected session revision, originating client, gesture/transaction ID and unique request ID. Validate authorization, context, object existence, locks, range, transport restrictions and revision before mutation. Remote retries are deduplicated; stale commands fail visibly. Return accepted/rejected status, applied values and resulting revision. Receipt is not sample execution: native transport schedules exact audio events and reports actual execution time separately.

Continuous controls use bounded latest-value coalescing plus a guaranteed final gesture value; edits and transport transitions cannot be dropped or coalesced as ordinary fader updates. Queue overflow returns explicit backpressure. Undo groups one deliberate gesture/transaction. Audio-thread dispatch uses bounded prevalidated messages; structural changes build off-thread and swap at a safe boundary. Allocation, I/O, plugin scanning, AI inference and arbitrary state serialization stay outside callbacks. Do not promise general lock freedom without inspecting ownership/reclamation.

The command registry also supplies title, help text, enabled predicate, parameter schema, focus contexts, undo/reversibility, risk category, shortcut-profile entries, UI location, examples and real implementation status. A knowledge answer can name only commands documented for its build. Parameter IDs survive presets, automation, save/reopen and remote/assistant references; labels are not IDs.

## Audio and editing
Device clock is authoritative; session samples and transport generations use foundation/CLOCK_LATENCY_SPEC.md. Dry capture records input before playback clip transforms, mixer inserts or monitoring. Graph ledger includes input/output adapters, hosted and native DSP, sandbox IPC, slots, sidechains, compensation and pending output. Audio route cycles fail. Monitor controls and assistant speech have separate non-export paths.

Preserve foundation/MIXER_SLOT_SPEC.md exactly unless a tested explicit migration is needed. Tape and Summing providers remain separate from regular inserts. EQ/dynamics/tuning do not appear as undisclosed fixed coloration. Console and Visual views edit the same gain/pan/width and automation state. Snapshot recall is one checked operation with a documented automation interaction.

Immutable original media -> native comp/edit map -> shared pitch/time edit map and rendered revision -> clip gain/fades at the documented existing stage -> channel source. Sprint 1 must reconcile actual gain/render order with the existing foundation, including any cached gain-dependent analysis. Do not silently move gain ahead of analysis or bake it twice. Preserve declared stage order and hash inputs to analysis. Capture always bypasses these playback transformations.

The owner develops Tune, Vocal Editor and Align separately. Inspect actual delivered interfaces before selecting insert or clip integration; do not assume they expose native edit maps. Native DAW timing edits use composable maps and avoid repeatedly stretching an already stretched cache. Plugin-specific pitch/alignment algorithms are outside this cycle. Analysis/cache keys include source hash, channel layout, range, algorithm/version, rate, parameters and edit revision. A clip has one active render and one replaceable pending revision; session admission and memory are bounded. Stale results never publish over newer edits. During playback use the last valid render with a clear pending state; export must wait for current renders or fail explicitly.

## Persistence and supervision
Project snapshots, original media, media commit journal, plugin state checkpoints and analysis caches have separate durability policies. Valid original media and edit maps are sufficient to regenerate disposable analysis/render caches; document deterministic version changes. Recovery never deletes or overwrites the only original. Plugin workers expose health and latency without blocking capture. Runtime process isolation is an explicit bridge architecture with bounded IPC, not a try/catch around native crashes.

## Interface freeze outputs
At G1 write actual code-level contracts for CommandRegistry, ParameterRegistry, SessionSnapshot, MediaCommit, PluginHost/Worker, GraphLatency, ClipAnalysisJob, EditMap, RenderResult and MonitorRoute. Names here are conceptual, not required class names. Include ownership/thread rules, schema migrations, units, errors, lifecycle, bounds and replacement strategy. G2 checks consumers against those real contracts before introducing AI or remote mutations.
