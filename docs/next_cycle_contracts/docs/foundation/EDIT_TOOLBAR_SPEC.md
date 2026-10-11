# Audio editing toolbar — revision 1.7
Status: REQUIRED; product implementation and tests NOT EXECUTED. Scope: the Arrange/audio-editing mouse-tool family, its alternate/link behaviors and Paint submenu. This is not a promise of every control in every commercial editor or edition.

## Reference and gap audit
Primary references inspected 2026-10-11:
- Studio One 5 Reference Manual, Arrange View Mouse Tools: https://pae-web.presonusmusic.com/downloads/products/pdf/Studio_One_5_Reference_Manual_05072020.pdf
- The current manual reached from Studio One documentation search identifies itself as Fender Studio Pro 8.1: https://fenderstudiopromanual.fender.com/en/Content/Editing_Topics/Arrange_View_Mouse_Tools.htm
- Clip Gain Curves: https://fenderstudiopromanual.fender.com/en/Content/Editing_Topics/Clips_and_Clip_Gain_Curves.htm

Documented reference inventory: Arrow, Range, Split, Eraser, Paint, Mute, Bend and Listen; linked Arrow/Range and a configurable alternative tool. Paint includes Freehand, Line, Parabola, Square, Triangle, Saw, Sine and Transform. Reference Listen auditions a track while held. These names establish coverage, not proprietary implementation or exact parity. Modern pitch-curve features are not imported by this toolbar requirement; V12 remains authoritative for pitch scope.

In revision 1.6, V01–V05/V11/V14 described broad selection/edit/gain/timing functionality. A dedicated Eraser interaction, full Paint submenu, explicit Bend-marker lifecycle, Listen gesture and configurable alternate/link behavior were not all independently contracted. Actual repository implementation remains unknown until preflight.

## T01 — visible tools and command ownership
Eight named tool buttons are mandatory, in the reference inventory order. Each has tooltip, active state, cursor feedback, accessible name and command-search entry. Reuse shared edit commands; do not implement a second editor in widget callbacks. Tools must operate in both timeline and audio editor with the same underlying semantics. Document intentional differences in available context.

The following are SML implementation requirements. They reuse VOCAL_EDITING_SPEC.md and NUDGE_SPEC.md for edit semantics and add the missing interaction contracts.

| ID | Tool | Required SML audio behavior |
|---|---|---|
| T02 | Arrow | Object selection/move; discoverable edge trim and clip fade/gain handles; selection and lock rules shared with V01–05/V10 |
| T03 | Range | Single/multitrack time selection with numeric start/end/length; disjoint ranges; move/copy/delete/split-range commands acting only on the highlighted material |
| T04 | Split | Preview line and exact time readout; cut selected eligible clips at the pointer or selection boundaries using existing V02 semantics |
| T05 | Eraser | Remove timeline instances, preserving media files; click and drag sweep; affected instances visibly previewed; one undo restores the whole gesture |
| T06 | Paint | Clip-gain and audio/plugin-parameter automation editing; all eight submenu entries; no creation of instrument parts |
| T07 | Mute | Reversible clip mute, separately labeled from channel mute; range of affected clips shown; supports sweep and selected clips |
| T08 | Bend | Insert/move/delete local warp anchors; display source and destination time; render through V11 pitch-preserving service; immutable original retained |
| T09 | Listen | Press-and-hold audition from the pointer using a temporary audition transaction with explicit cleanup and recording protection |

Disjoint selection is a set of intervals, never an implicit consolidate-to-new-file operation. Copy/move uses a shared delta preserving gaps and track offsets. Locks reject a compound edit atomically. Eraser/mute click on a selected member targets the selection; otherwise the clicked instance only. A mute sweep chooses its target state from the first clip and touches each ID once; revisiting a clip cannot toggle it twice. Eraser does not ripple unless the existing explicit ripple mode is enabled. Split applies only to eligible clips that contain the split time; zero-length pieces are rejected.

## T10 — combined and alternate tools
Provide an explicit Link Arrow/Range toggle with visible hit-zone feedback. At sufficient track height, upper body selects ranges and lower body selects objects; explicit fade/trim/gain handles take priority. At compact heights disable the split hit zone with an explanation and retain direct Range access. Freeze the effective tool for a pointer gesture after pointer-down; moving across zones cannot switch the operation midway.

Allow a user-selected alternate tool invoked by a held modifier while Arrow is active. Expose the assignment beside Arrow and in shortcut preferences. Ship a selectable Studio One-style top-row 1–8 tool preset; it must not overwrite the Pro Tools-style Nudge keypad bindings. Resolve modifier conflicts per context and display the actual assignment. Key release/focus loss must clear temporary tool state. Text entry and plugin editor focus must not trigger editing shortcuts.

## T06 detail — drawing and Transform
Target type, parameter name, time range and units must remain visible; drawing on a clip never silently writes its mixer automation. Gain and automation envelopes remain distinct data. Shape selector: Freehand, Line, Parabola, Square, Triangle, Saw, Sine, Transform. Implement actual numerical curves, not icons or fixed bitmap previews. Document orientation, endpoint convention, amplitude/baseline, phase/cycle count for periodic shapes, and bounded point-density/error criteria in the Sprint 1 command index before tests are authored.

Transform uses a selection box to shift/scale existing points in time/value, with numeric controls. Bound operations to the displayed target range; reject non-finite/unsupported results. Automation write/read arbitration follows the existing engine. Clip-gain curves can be bypassed without deleting their points. Replacing points inside a drawn range preserves outside data and creates one undo record. Bound drawing queues and simplify on a worker/UI thread with a declared error tolerance, never on the audio callback; final endpoint must survive coalescing. Waveforms showing gain-adjusted amplitude must label that display mode, distinct from waveform zoom.

## T08 detail — timing integrity
Warp anchors require monotonically increasing source/destination maps and documented minimum spacing and stretch ratios. Reject crossed anchors and unsupported mappings. Grouped/stereo edits use one shared timing map where applicable. Dragging a marker must not fall back to varispeed, restart capture or rewrite source media. Render admission is bounded: at most one active render per clip, one replaceable pending revision per clip, and configurable session-wide worker/queue ceilings. Stale render results cannot replace a newer clip revision. Keep the last valid render until a current result is ready, visibly show pending/error status, and preserve the edit map for retry/reopen. Missing pitch-preserving service is an implementation blocker for T08, not permission to display a working Bend button.

## T09 detail — safe audition
This is our explicit audition policy, not a claim of exact vendor transport behavior. Audition is allowed only with the main transport stopped and no recording active. Disable it during playback/recording with a visible reason; do not stop a take to audition. Snapshot cursor/transport state and use a temporary route eligibility mask without modifying saved mute/solo/arm flags. Reuse the bus/FX routing rules; include needed returns while excluding unrelated dry sources. Honor explicit mutes and show why a muted source is inaudible. Do not write automation, record media or create an undo entry.

Mouse-up, Escape, focus loss, track deletion or device failure must end audition and remove its temporary state. Restore the cursor only if still owned by that audition transaction; later user commands win. A transport-start/record command first terminates audition, then runs normally. No stuck solo after cancellation or reopening a session; audition state is never persisted.

## T11 — adjacent controls and precision
These are SML toolbar requirements, not a claim that each is a separate Studio One mouse tool: discoverable zoom/pan navigation, snap on/off and temporary override, Grid/Relative Grid with visible timebase, quantize settings for explicit audio timing operations, autoscroll, context help, and visible edit/automation modes. Quantize shows affected anchors/events and a preview; changing its amount or changing snap alone never processes audio. Rhythm editing of recorded audio remains supported without MIDI composition.

Keep Nudge a separately labeled section with its own amount, units and target. Its shortcuts and sub-sample arithmetic remain exclusively defined by NUDGE_SPEC.md. A narrow window may use labeled overflow for secondary controls, but selected tool, snap state and Nudge target remain discoverable. No unlabeled icon-only dependency.

## Tests, ownership and evidence
Sprint 1 inventories T01–T11 against actual commands/UI/engine, establishes missing services and real test commands. Sprint 2 implements all portable toolbar behavior; Sprint 3 completes binding to hosted plugin parameters. Sprint 4 repeats native interaction/scaling/accessibility and the vocal walkthrough. No fifth sprint or increase to the six-hour cap. Unfinished tools remain PARTIAL/MISSING and block applicable acceptance.

- T-T1: parameterized eight-tool dispatch across timeline/audio editor, group/lock rules, exact affected IDs/intervals and source hashes; one gesture/one undo; Escape rollback. UI components must exist and dispatch actual commands.
- T-T2: numerical fixtures for every Paint shape and Transform; envelope boundaries, bypass, undo/reload and bounded update queue; no mutations of the wrong envelope domain. Sprint 2 uses native automation fixtures; external parameter binding finishes in Sprint 3.
- T-T3: Bend monotonicity/ratio limits, stereo/group mapping, stale/canceled worker results, exact requested edit map and V11 duration/pitch tests; bounded render resources and no callback work.
- T-T4: Listen cleanup after every exit condition, record attempt, explicit mute/solo state preservation, correct route mask and unchanged session serialization.
- T-T5: alternate/link hit zones, compact-track fallback, modifier release/focus protection, keypad Nudge coexistence, snap override, selection precision and no edit on merely changing grid/quantize controls.
- T-T6: actual native walkthrough at H4 display sizes/scales, tooltips/keyboard access, all eight visible/reachable tools, audition and warp listening review; real evidence only.

A2 requires T-T1–5 portable behavior with native parameter fixtures; A3 requires external parameter binding where hosting is supported. T-T6 is native/owner evidence and may remain pending while portable work proceeds. Report docs/evidence/sprint_02/toolbar-coverage.csv with T IDs, actual commands, status and evidence; store software results in toolbar-tests.log and native evidence in toolbar-ui-review.md. A passing package verifier does not prove any toolbar works.
