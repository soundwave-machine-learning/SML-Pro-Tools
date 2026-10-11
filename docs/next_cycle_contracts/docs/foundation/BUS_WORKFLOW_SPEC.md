# Easy bus workflow and professional mixer


Revision 1.7 • R13 / Sprint 3.

## Reference behavior
Studio One documents selecting mixer channels and creating a bus that automatically receives their outputs. It also provides hide/show source-channel actions. These are the reference behaviors; SML naming, undo, graph validation and routing safety policies below are original implementation requirements. Reference: Studio One 4.1 manual, Busing, page 162 (link in REFERENCE_BEHAVIOR.md).

## Main action: Create Bus from Selection
Select Lead, Double Left and Double Right → right-click → Create Bus from Selection → type “Vocal Bus” → Enter. One operation creates the bus, routes those channels to it and selects the new bus. No manual numbered send/return pair setup. Also expose this from the mixer toolbar and command search.

B01: New bus has unity trim/fader, centered neutral stereo routing, no plugins, readable name and adjacent placement. Retain source-channel faders/pan/inserts/automation/sends. Replace each selected source's main output with the new bus; do not add a parallel dry path. Preserve unique IDs and undo the entire graph edit in one step.

B02: If selected sources share an output, route the new bus to that same destination to preserve hierarchy. If their outputs differ, show a compact destination choice (default suggestion Main) and explicitly show changed routes before applying. Cancel changes nothing. Existing sends remain visible. Selection containing an ancestor and its child is rejected with an explanatory message unless a proven graph-preserving rule exists; never route duplicate signal paths silently.

B03: Bus channels are actual summing/processing channels, distinct from organizational folders and VCA/control groups. They expose inserts, meter, fader, mute/solo, output and sends. Support Vocal Bus → Mix Bus → Main nesting. Prevent cycles across both output and send edges before committing any graph change. Stereo conversion and pan law tested; bus creation at unity adds no unexplained 3/6 dB gain.

B04: Hide/Show Sources collapses mixer display only; audio and automation continue. Route another track using a named output picker or drag-to-bus drop target with preview; move/reorder visual channel strips never silently changes routing. Search/filter by name; optional colored membership indicator. Clear output breadcrumb shows track → bus → master.

## Separate action: Create FX Return and Sends
Select tracks → Create FX Return → name “Vocal Reverb” → choose installed plugin or leave empty → choose output (Vocal Bus or Main). This adds sends while preserving dry outputs. Default post-fader, send level -infinity until explicitly raised; pre/post switch and numeric send gain visible. New return unity gain, effect wet-only setting is user's responsibility and shown in quick-start. Do not automatically change unknown third-party plugin parameters. Warn that an empty return can duplicate dry signal when its send is raised. No bundled reverb is added.

B05: At least two visible sends per strip; expandable slots. A bus may feed FX return only if graph remains acyclic. Solo-in-place and solo-safe obey the explicit policy below; do not infer behavior from labels.

B06: Deleting a bus never deletes its source tracks or media. Offer explicit reroute-to-current-parent; show sends targeting the bus and choices to remove/reassign them. Validate destinations, apply atomically, undo restores old graph/plugin state. Preserve missing-plugin metadata. No deletion of owner audio files.

## Required tests B-T1–B-T7
Synthetic headless tests plus Windows UI walkthrough; docs/evidence/sprint_03/buses/.
- B-T1: create bus from three tracks already feeding Main; before/after render maximum absolute difference <=1e-5 and gain difference <=0.01 dB. One undo restores routing/order/selection. No duplicate master path.
- B-T2: mixed destinations, cancel, locked/invalid selection and nested ancestor selection; cancel/rejection leaves exact pre-operation graph. Successful action uses explicitly selected destination.
- B-T3: Vocal → Mix → Main hierarchy; output/send cycle attempts fail atomically; save/reopen reproduces graph/levels/IDs.
- B-T4: 0 dB source to post-fader send/fixture return, then -6 dB source fader; expected send amplitude scales by 10^(-6/20) within 1e-5. Pre-fader mode ignores source fader per SL02; mute/solo rules below cover downstream returns.
- B-T5: hide/show sources and drag-reorder yield identical audio render; moving a track to named bus changes only intended graph edge.
- B-T6: bus deletion with children/sends and undo; no orphan references, source loss or altered plugin state; exact graph recovery.
- B-T7: owner creates Vocal Bus and Vocal Reverb send without opening global I/O setup or typing internal bus numbers. Record action count/time and ambiguities; goal <=3 interactions after source selection for the simple same-destination bus action, excluding typing. No false UI-only success.

User guide includes separate examples for subgroup buses and parallel FX returns, and explains that an organizational folder is not necessarily an audio bus. A diagram, if added, must reflect the tested signal path.


## Neutral buses, slots and dual views
New buses: stereo Balance centered, compensation off, Width 100% identity, Clean Summing, Tape None, trims/fader unity. Main defaults to stereo identity without another mono pan stage. This deliberately prevents compensated pan profiles from adding 3 dB at every nested bus. Profile changes do not silently replace these neutral bus defaults. In B-T1 fixtures disable external processing and normalise identity defaults; compare aligned outputs. Rerouting retains source pan/processing and saved slot state. Console/Visual dispatch the same graph commands. No native coloration is implemented in this pass.

## Mute/solo policy for new graph-version sessions
Channel mute gates its final output and post-fader sends, but pre-fader sends remain active unless their own send mute is on. Label this in the send UI/manual. Solo eligibility gates all non-solo-safe source contributions, including pre-fader sends. Soloing a source opens required downstream buses/FX without admitting unrelated sources to a shared return; soloing a bus admits its upstream contributors then required downstream paths. An explicit solo-safe strip remains eligible and may intentionally admit its own sources; make that visible. Do not describe explicit solo-safe behavior as “only selected source.” Mute on an explicitly muted source still wins over solo. Tail processing may continue internally, but final muted routes output zero.

Test with two unrelated tracks sharing an FX return, nested buses, pre/post sends, muted source and solo-safe toggles. No cloned processor instances to solve solo. Preserve legacy stored mute/solo/routing semantics until explicit graph-version migration; do not claim old session identity by default if a behavior changed.

## Spill and send ownership
SL06 defines Spill and independent send pan; SL02 owns exact tap order. Hide/Spill is view-only and restores previous visibility on exit. Graph cycle validation covers output/send edges and supported sidechain dependencies. Unknown topology is a blocking graph issue, never a reason to pass silently.
