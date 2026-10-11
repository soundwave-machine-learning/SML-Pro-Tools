# Vocal editing specification
Revision 1.7 • owner scope extension, 2026-10-11.
Status: REQUIRED PRODUCT BEHAVIOR, implementation and validation pending.

## Workflow
Record alternate vocal takes, audition and comp phrases, correct timing using edits/nudges, balance breaths/syllables with clip gain, smooth joins with fades, route lead/doubles/ad-libs to named buses, process using personal plugins, save/reopen and export.

“All the types of tools” is a coverage requirement for the editing workflow. It is not evidence of complete feature equivalence with two commercial DAWs. Audit the following matrix in Sprint 1, repair/expose it in Sprint 2, and report every row as VERIFIED, IMPLEMENTED UNVERIFIED, PARTIAL or MISSING. A menu entry alone does not satisfy a row. V-E2E is staged: Sprint 2 ends after edit/comp save/reopen; Sprint 3 completes buses/host/export; Sprint 4 repeats the native workflow. V07 count-in/punch/loop semantics finish in Sprint 2; Sprint 1 CL-T3 tests its scheduler primitive rather than requiring the future UI. Do not quietly narrow the matrix to meet the six-hour budget.

## Coverage matrix
| ID | Tool family | Required behavior | Primary acceptance |
|---|---|---|---|
| V01 | Selection and navigation | Object and time-range selection; multi-track selection; visible selection start/end/length; horizontal/waveform zoom; loop audition; next/previous clip boundary and transient navigation | One intended operation affects only shown selection; waveform zoom changes no gain |
| V02 | Basic editing | Split/separate at cursor or range, cut/copy/paste, delete, duplicate/repeat, move, non-destructive left/right trim, heal compatible splits, consolidate to a new file | Exact boundaries, undo/redo, originals untouched, no unexplained timeline ripple |
| V03 | Editing modes | Slip, Grid, Relative Grid, Spot/numeric placement, explicit Shuffle/ripple opt-in; temporary snap override; smart/context-sensitive select/move/trim/fade hit zones | Slip default for vocals; changing grid does not change Nudge; ripple never silently enabled |
| V04 | Fades | Visible clip handles, typed fade length, fade-in/out, crossfade, batch fades, equal-gain/equal-power choices and audition | Reversible; source-handle bounds checked; fade shapes/render verified; no automatic cut at a chosen zero crossing unless enabled |
| V05 | Gain cleanup | Clip gain handle and numeric dB; editable clip gain envelope for breaths/plosives/syllables; explicit clip mute; clip-gain step separate from time nudge | Clip gain/fades are in the playback source stage ahead of Tape/user inserts and independent of mixer fader; undo restores all envelope data; no normalization by default |
| V06 | Takes and comping | Alternate take lanes/playlists, name/audition takes, promote a selected phrase to active comp, adjust comp boundary, preserve all source takes, optional bounded crossfade at join | Phrase comes from intended take with original alignment; no double playback; save/reopen preserves comp and alternates |
| V07 | Recording conveniences | Pre/post-roll, count-in, selected-range punch and repeated/loop takes feeding take lanes | Actual record interval and each take boundary tested; no claim of Pro Tools QuickPunch implementation unless its full capture behavior exists |
| V08 | Precision timing | Full dedicated NUDGE_SPEC.md; manual double/ad-lib alignment using displayed waveform/cursor and reversible timing edits | Exact movement and retained inter-clip offsets; no unintended time stretching |
| V09 | Strip silence | Preview threshold, minimum duration and head/tail padding; reversible apply and audition | Low-level breath fixture is preserved under chosen padding/settings; no destructive media rewrite |
| V10 | Grouped editing | Named edit groups, stereo linked channels, temporary group suspension, explicit locks, markers | Group positions and stereo phase relationships preserved; locked member causes atomic rejection rather than partial movement |
| V11 | Time correction | Preserve/audit current pitch-preserving stretch/warp as an editing service; selectable local timing edit, original retained, offline/background render | No varispeed labeled pitch-preserving; named algorithm/limits and artifacts documented; worker render does not block transport |
| V12 | Pitch correction | User-supplied VST3 pitch processor via normal host/transfer workflow supported by that plugin; retain plugin state and bypass | Test named owner plugin before compatibility claim. Integrated note editor/ARA is a separate capability and remains explicitly unsupported unless actually implemented |
| V13 | Printing and handoff | Consolidate/export selected vocal, dry edited stem, processed stem and full mix; distinguish clip gain/fades from track/master processing | Stem start/time origin, tails, sample rate and dry/wet path documented and checked; original editable session retained |
| V14 | Interaction | Discoverable context menu, dedicated edit toolbar, typed values, reset, shortcut search/remap, predictable focus and undo/redo | No edits from typing text/plugin fields; commands shared with CLI/MCP; visuals and audio update together |

## Repair strategy
Keep existing edit/playlist/strip-silence/stretch code when correct. Removal of built-in insert plugins MUST NOT remove clip gain, fades, time-edit/render primitives, or recording metronome. If these depend on a removed plugin module, extract the necessary internal service and its tests. No new bundled pitch-correction instrument/effect is authorized. Any brand-new ARA integration or note-level pitch editor requires a separately scoped effort; show this gap clearly in the feature matrix.

Each action is one coherent undoable operation. Compound actions (comp phrase promotion plus seam creation, batch edits) undo atomically. Audio source files remain immutable. Fades/clip gain remain attached to their intended clip material, with documented behavior during split, slip, trim and nudge. Automation-follows-edit is a visible policy, not an accidental side effect.

## End-to-end fixture V-E2E
Use generated vocal-like phrase envelopes with known leading/trailing silence, breaths and pulse boundaries; owner vocals are an optional listening test and never committed. Import/record three alternate takes, select two phrases into a comp, trim the breath region, apply a gain envelope, crossfade a join, nudge the doubled phrase +10 ms then undo, group lead/double for a shared move, route to Vocal Bus, add an external fixture, save/reopen, and export dry/processed stems. Exact source hashes unchanged; clip/comp/channel selection persists; deterministic pre-plugin render error <=1e-5 versus independently constructed reference. Listen to actual vocal artifacts on owner machine before declaring professional vocal-editing readiness.

Evidence: docs/evidence/sprint_02/vocal-coverage.csv, vocal-edit-tests.log, vocal-e2e.json, ui-review.md and generated reference hash list. Every matrix row needs a real command/test or an explicit platform/owner blocker; no unnamed passing tests.

## Editing/export precision clarified by audit
V13 has three named paths: Raw Source (original capture/media); Edited Dry (active comp, clip edits/gain/fades/time edits only, before utility trim/Tape/inserts/fader/pan/sends); Processed Track/Bus or Full Mix (declared output tap per SL02). Do not call a bypassed full mixer graph “dry” without labeling remaining processing. Stems share a selected timeline origin, with explicit leading silence and tail range. Nonlinear/shared FX or master processing can make summed stems differ from a full mix; test each selected graph contract, never promise reconstruction by summing arbitrary processed stems.
V11 fixture uses known pitched content/impulses: verify requested duration/boundaries, expected pitch within algorithm-specific declared tolerance, finite output and unchanged source. Establish that real algorithm/tolerance in Sprint 1; absence of a usable existing pitch-preserving service is a recorded implementation gap, not permission to label varispeed as stretch. Artifacts require native listening review, separate from deterministic render tests.

## Explicit toolbar coverage
EDIT_TOOLBAR_SPEC.md T01–T11 is mandatory under V01–V05/V11/V14. It specifies eight audio tools, Link/alternate behavior, Paint shapes, safe audition and Bend-marker lifecycle. A generic edit menu does not satisfy these controls.
