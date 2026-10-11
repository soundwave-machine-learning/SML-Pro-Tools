# Visual mixer coverage (VM01–VM05, VM-T1–VM-T8)

| Item | Status | Evidence |
|---|---|---|
| VM01 Console / Visual / Split in the Mix window, persisted per session (`mix.view`) | IMPLEMENTED; rendered | `ui/console.png`, `ui/visual.png`, `ui/split.png` |
| VM01 one node per visible track/bus/return keyed by id; Master not a node; name/colour/selection/mute/solo/activity; list with select/hide/numeric gain/width; filter | IMPLEMENTED (route badge, clipping cue and rename in list not done) | UI source; `visual-mixer-ui-tests.log` |
| VM02 x = existing pan/balance, y = existing fader; −60..+12 dB view without clamping stored values; edge indicator; centre/0 dB guides; Shift axis lock; Alt fine; double-click reset; Escape cancel; stereo centre/span rule with narrowing | IMPLEMENTED | `mixmath` + `visual_mixer` unit tests |
| VM02 multi-selection group drag, locks | NOT IMPLEMENTED (single-node gestures only) | — |
| VM03 native Width in Console and Visual, same parameter, M/S formula, exact 100 % identity, 10 ms-scale per-block ramp, mono disabled | IMPLEMENTED | `mixmath-tests.log`, `slot-graph-tests.log` (vmt3) |
| VM04 one model: both views dispatch `mix.volume/mix.pan/mix.width`; one undo per gesture; latest value kept; deletion mid-gesture isolated | IMPLEMENTED (engine-level VM-T1/T4/T6) | `visual-mixer-state-tests.log` |
| VM04 automation read/touch arbitration in Visual | NOT VERIFIED (Visual reads the same parameters, so Read playback moves nodes; no dedicated test) | — |
| VM05 A/B/C snapshots (scope: gain, pan, width, mute, solo, visibility; excluded listed), stopped recall, undoable, missing ids reported, persisted | IMPLEMENTED | `cmd::snapshot` tests |
| VM05 Create Bus / FX Return from the Visual header | IMPLEMENTED (shared commands) | UI |
| VM-T2 view open/close/Split does not change audio | PASS (engine render with view state) | sml_graph slt1 |
| VM-T7 performance (16 tracks, Split, 60 s drag) | NOT RUN — native H3 gate | — |
| VM-T8 owner walkthrough, 100/125/150 % scaling | NOT RUN — owner gate | — |

Not claimed: iZotope layout/DSP parity.
