# Dedicated Nudge specification
Revision 1.7 • R12 / Sprint 2 • mandatory before calling vocal editing complete.

## Reference and claim boundary
The reference is Avid's publicly documented Pro Tools nudge behavior: clip movement, content slip, trimming, selection movement and an independent editable nudge value. Exact Windows shortcuts below are grounded in Avid's First 12.3 help and 2019.5 shortcut guide; these are versioned references, not a claim that the latest full Pro Tools release was run. See REFERENCE_BEHAVIOR.md. SML-specific panel design, arithmetic/edge policy and test criteria below are engineering decisions. Label any unresolved behavioral difference in a compatibility matrix; do not advertise complete Pro Tools parity.

## Always-visible Nudge section
Put a dedicated NUDGE group in the Edit toolbar and a larger optional Nudge panel. Show value, unit, previous/next value, left/right arrows, target, and actual displacement in samples/ms. Grid and Nudge have separate state. Include sample, milliseconds, seconds, Bars|Beats/ticks and timecode-frame units. Support custom positive values and obvious presets: 1/10/100 samples; 0.1/1/5/10/20/50/100 ms; musical subdivisions with triplet/dotted; 1 frame. These are SML presets, not a transcription of Pro Tools' complete menu. Default vocal value 1 ms; provide one-click 1 sample and 10 ms favorites. A user can see “10 ms = 480 samples at 48 kHz” before applying it. Below-sample values show requested fractional displacement and accumulated remainder; a key step may legitimately change no integer sample until the threshold is crossed, with explicit feedback and no fabricated movement.

Offer targets: Selected clips; Clip contents; Left trim edge; Right trim edge; Selection/cursor only; Selection start; Selection end. Show the current target in the panel and status readout. Command IDs must be distinct even if they share one validated engine primitive. Panel choice governs panel arrows; keyboard modifiers dispatch their own documented target and temporarily display it, never silently inherit an incompatible panel mode.

## Pro Tools-style Windows preset
`Num+/-` means numeric keypad plus/minus; do not treat every text plus/minus key as a nudge.

| Action | Binding |
|---|---|
| Move selected clip(s), or cursor when no clip is selected | Num+ / Num- |
| Slide content inside fixed clip boundaries | Windows/Start + Num+/- |
| Trim clip start | Alt + Num+/- |
| Trim clip end | Ctrl + Num+/- |
| Move edit selection only (SML command) | Panel/command search; no Pro Tools-compatible default chord asserted |
| Move selection start only | Alt + Shift + Num+/- |
| Move selection end only | Ctrl + Shift + Num+/- |
| Change nudge value | Ctrl + Alt + Num+/- |

Laptop-friendly alternative: comma/period for left/right only when explicit Edit Keyboard Focus is active; configurable named commands and clickable arrows always available. Text entry, plugin editor, modal dialog and shortcut assignment capture must take precedence. OS-reserved Windows chords require native testing and remappable fallback; never globally intercept system keys. Unsupported physical numpad distinction in the current UI backend is a preflight engineering issue, not permission to conflate keystrokes. Preserve key repeat count, no lagging queued burst after key release; group repeats as one undo gesture while preserving discrete presses as discrete actions.

Audit finding: Avid's 2019.5 guide describes Shift+Num+/- as moving start/end data while retaining underlying audio; that is insufficient evidence for the former Selection-only binding. Reserve that chord in the reference preset pending a native context check. The explicit SML Selection-only command remains implemented and remappable. Alt/Ctrl clip-trim and Alt/Ctrl+Shift selection-boundary mappings are separately supported by Avid's dedicated help pages. This is a documented compatibility gap, not an ambiguous implementation command.

## Engine semantics (SML contract)
Use integer sample positions for audio edits and validated high-precision/rational unit conversion. Do not derive positions from pixel movement, frame rate or current zoom.
- Clip move: change timeline start by signed delta, keep source offset/length/fades/gain relationship; move selected/grouped clips as one rigid set. No snap-to-grid override or automatic ripple for nudge.
- Content slip: timeline start/end fixed. Moving audible content right by D reduces source offset by D (for unwarped equal-rate clips); verify this sign using a known impulse. Require available source handles. Warped/rate-converted clips need a defined source-to-timeline mapping; never reuse an unwarped formula blindly.
- Left trim: timeline start and source offset move together while the right edge stays fixed. Right trim changes duration, with start/source offset fixed. Extend only inside source handles; no zero/negative-length clip.
- Selection/cursor only: translate range without altering audio. Independent boundary actions keep the other endpoint fixed; invalid inversion is rejected with visible explanation. Avid-context selection of whole versus partially intersecting clips must be documented and tested before a compatibility PASS; SML explicit object mode uses selected IDs, never all clips on a track by accident.
- At project start, locked member, invalid handle, integer overflow or unsupported mapping, reject the compound edit atomically with reason. Do not clamp different clips independently, which would alter alignment. This reject policy is SML-specific unless a native Pro Tools check confirms equivalence.
- For SML clip move, fades and clip-relative gain envelope move with the clip. For content slip, clip-relative fades/gain envelope stay anchored to the fixed clip window while underlying source changes; display that distinction. Trim retains the envelope time reference and clips its evaluation to the new window without deleting hidden envelope nodes. These explicit edit-envelope rules are original SML behavior, not a Pro Tools equivalence claim. At a shared crossfade, recompute only a valid supported overlap; reject invalid operation instead of silently moving the neighbor. Track automation follows edits only when explicit setting is enabled. Source media never rewritten by ordinary nudge.
- Nudge/units/focus preferences persist. Save/reopen preserves edited coordinates exactly. Command response reports requested and applied delta, target and changed IDs; error means no partial edit.

## Rounding and tempo
Sample-unit movement is exact. For time units, accumulate signed high-precision position from the gesture's anchor and round only the final sample coordinate: at 44.1 kHz, ten 1 ms nudges must total 441 samples, rather than ten independently rounded 44-sample steps. Keep a per-target/unit exact logical anchor and signed cumulative step count across discrete presses and direction reversal until direct reposition, target-set, source-map, rate, tempo-map or unit change resets it. Convert anchor plus cumulative displacement, rounding ties-to-even once; therefore a reversed sequence returns to its anchor without requiring Undo. Reset accumulated conversion state on direct reposition or unit change using a documented rule. Do not infer 1 ms from the displayed truncated sample count.

Musical moves use the tempo map's bidirectional tick/sample transforms at the anchor. Test across tempo changes; multiplying a start-position step by count is insufficient for general musical movement. Define the group anchor and preserve relative sample offsets under a group timing move. Timecode frames use session frame-rate rational values; drop-frame numbering is a display convention, not dropped audio frames. Claim only supported frame-rate/unit combinations. Clip gain nudge uses a separate dB setting. One pure-time group anchor is the earliest selected clip start; equal-start tie uses stable ID. Keep pairwise sample offsets. On rejected commands, do not advance cumulative nudge state. Undo/redo restores that logical state as well as displayed coordinates. Native numpad/focus/load tests are target gates; headless command semantics remain A2.

## Acceptance suite N01–N10
All automated tests are headless and repeated at 44.1/48/96 kHz for edit math (recording release support remains 44.1/48 kHz). Actual keyboard/latency tests also run on Windows. Persist logs and numerical fixtures under docs/evidence/sprint_02/nudge/.
1. N01: +1 then -1 sample moves clip exactly and returns bit-identical edit state; unchanged source bytes; repeat at several zooms.
2. N02: +10 ms = +480 samples at 48 kHz; 1000 positive then 1000 negative 1 ms steps at 44.1 kHz return to anchor with zero drift; forward endpoint exactly +44100 samples.
3. N03: impulse fixture content slip shows correct audible sign, fixed timeline bounds and unchanged clip length; unavailable handles reject without mutation.
4. N04: left/right trim and selection boundaries change only specified values; near-zero-length invalid edits leave full state unchanged.
5. N05: rigid multi-clip/group move preserves all pairwise offsets, stereo channel alignment, lengths and source offsets; one locked member or negative target rejects whole command.
6. N06: Slip/Grid/Relative Grid/Shuffle settings and display zoom produce the same sample-unit nudge delta. Musical stepping crosses 120→90 BPM boundary and round-trips via independent tick-map expected values.
7. N07: real keypad versus text-row keys, every modifier, held-repeat and laptop mapping; text/plugin focus causes zero edit commands; command log and visible target agree.
8. N08: undo/redo entire object/selection/fade/gain/automation state; save/reopen identical positions/units; no partially applied invalid command.
9. N09: 100 accepted nudge key actions during playback, 16 tracks/2000 clips; no lost/double commands; p95 input-to-visible change <=50 ms, no stale burst after release, zero xruns at tested target configuration.
10. N10: compare reference examples in a legally available Pro Tools instance when available, record version/platform and any deviations. Native comparison is required for full behavioral equivalence claims; documented-target implementation can be delivered with comparison pending and clearly labeled.
