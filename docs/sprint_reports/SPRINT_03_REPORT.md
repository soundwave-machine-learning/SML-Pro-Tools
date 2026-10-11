# Sprint 03 report — Mixer, slots, own plugins and synchronized Visual view (gate A3)

Prerequisites: A1 PASS (`f70ac26`), A2 PASS for executed rows (`c11b84f`). Host: Linux container;
software Vulkan (lavapipe) used for offscreen UI renders. No Windows, no owner plugins.

## Verdict
**A3 PORTABLE: PASS for the executed software rows** (B-T1..6, host/slot fixtures, M-T1/2/5/6,
SL-T1..4/6/7, CL-T4, VM-T1..6 at engine level and UI unit level) with named PARTIAL items below.
**SLOT INFRASTRUCTURE READY: YES (software)** — hosting, order, state, bypass/latency, Pre/Post,
audition overrides, persistence. **NATIVE SONIC MODELS: NO — DEFERRED BY OWNER.**
H2 (owner plugins), M-T7/SL-T8/VM-T7 (performance), VM-T8/B-T7 (owner UX): **pending**.

## Work done (contract items 1–7)
1. **Graph v2 (SL02)** in the mix engine: Summing → trim → Tape/Master-Tape Pre → inserts →
   Width → pre-send tap → fader → Master-Tape Post → post-send tap → panner → route; f64
   accumulation at bus/Main sums; neutral bus identity; one panner per send route (Follow =
   channel coefficients, Independent = send position); v2 mute/solo policy (pre-fader sends survive
   channel mute, solo admits downstream returns/nested buses and a soloed bus's contributors, not
   unrelated sources sharing a return; solo-safe). Legacy v1 sessions keep upstream behaviour
   until `mix.graph_migrate`. Cycle validation over outputs and sends (`bus.validate`).
2. **Easy buses (B01–B06)**: `bus.create_from_selection`, `bus.create_fx_return`, `bus.delete`
   (reroute to parent, remove/reassign sends, undo), `bus.sources`, view-only `bus.spill`. Console
   strips gained a SLOTS section (provider/bypass/trim/Pre-Post menu) and a Width row; Mix header
   has Create Bus / Create FX Return. Send Follow/Independent via `mix.send_level follow_pan`.
3. **External hosting**: dedicated slots host CLAP/VST3/AU instances through the existing
   off-audio-thread creation/adoption path (`instance_specs` slot indices 1000–1002), state capture
   (`mix.slot_state`, UI capture wiring), editors keyed per slot. Missing providers are named
   placeholders. Not done: background/cancellable plugin scan (scan still runs when the menu
   opens), scan-during-recording guard, in-process crash containment (none; documented risk).
4. **M01–M05 / SL01–SL06**: profile (`mix.profile`: Standard / Logic-style, 0/-3/-3c laws, Balance /
   Stereo Pan, balance compensation), slots per channel type with versioned defaults, slot trim,
   bypass keeping latency, category audition bypass (realtime only, never saved/exported),
   Tape-group schema field, `mix.signal_flow` readout. No model knobs.
5. **CL03/04/06**: `mix.monitor_mode` (Tracking/Mixing/Hardware — hardware mode disables software
   input monitoring), `mix.latency_report` (insert+slot latency per strip, worst path), PDC
   through slots verified. Not done: stopped Tracking deactivation/rebuild of high-latency
   plugins, latency-change-during-recording interruption policy, live-path ledger UI panel.
6. **VM01–VM05**: Console/Visual/Split with one model; node gestures, width handles, list,
   filter, snapshots A/B/C. Not done: multi-select group drag, rename in list, route badges,
   clip indicator, automation-mode specific tests.
7. **V12/V13/V-E2E**: named exports (`file.export_path` raw/edited_dry/processed/full_mix) with
   declared contents tested; final V-E2E software stage passes. V12 needs an owner pitch plugin.

## A3 checks (RERUN)
| Check | Command | Result | Evidence |
|---|---|---|---|
| B-T1..B-T6 (+ bundled-plugin rejection) | `cargo test -p soundcraft-engine cmd::bus` | 8 passed | `buses/bus-tests.log` |
| SL-T1/2/3/7, CL-T4, M-T1/2/5, VM-T3 engine, mute/solo, SL-T4 engine | `cargo test -p soundcraft-mix sml_graph` | 9 passed | `slot-graph-tests.log`, `compensation-and-slots.md` |
| SL-T4 commands, audition, latency report, V13 paths | `cargo test -p soundcraft-engine cmd::slots` | 3 passed | `slot-state-tests.log` |
| Host lifecycle incl. CLAP fixture in Tape slot (SL-T6) | `cargo test -p soundcraft-engine --test clap_e2e` | 3 passed | `host-slot-tests.log`, `plugin-compatibility.csv` |
| VM-T1/4/5/6 | `cargo test -p soundcraft-engine -- visual_tests cmd::snapshot` | 4 passed | `visual-mixer-state-tests.log` |
| VM02 mappings (UI unit) | `cargo test -p soundcraft-ui-egui visual_mixer` | 2 passed | `visual-mixer-ui-tests.log` |
| M02/VM03 math | `cargo test -p soundcraft-dsp mixmath` | 4 passed | `mixmath-tests.log` |
| Final V-E2E stage | `cargo test -p soundcraft-engine --test vocal_workflow vocal_e2e_final` | 1 passed | `vocal-e2e-final.json` |
| UI renders (Console/Visual/Split) | `cargo run -p soundcraft-ui-egui --example ui_shot` | rendered | `ui/*.png` |
| Full CI | `cargo xtask ci` | exit 0, 611 passed / 0 failed | `ci-sprint3.log` |

A test fixture bug found and fixed during this sprint: the first SL-T6 attempt inserted clip
audio without a source entry, so it vanished on save/reopen (render 0.0); fixed in the test, not
in tolerances.

## Requirements
| ID | Status |
|---|---|
| R06 own VST3/CLAP, editors/state/export | Software plumbing PASS with repo fixtures; owner plugins H2 pending; background scan PARTIAL |
| R07 professional mixer/routing/compensation | PASS (software) for the rows above; Post-insert section and tracking deactivation PARTIAL |
| R13 easy bus/FX return | PASS (software); B-T7 owner UX pending |
| R15 Logic-style clean profile | PASS (M-T1/M-T2); Logic comparison claim-only, not performed |
| R16 Summing slot | PASS (software) |
| R17 slot integrity; native models | M-T1/2/5/6 PASS; M-T7 native pending; M-T3/4/8 DEFERRED BY OWNER |
| R18 Console/Visual/Split one mixer | PASS (software); native interaction pending |
| R19 spatial gain/pan/width/list/snapshots | PASS (software) with PARTIALs listed in `visual-mixer-coverage.md` |
| R20 automation/undo/persistence | undo/persistence PASS; automation arbitration not separately tested (PARTIAL) |
| R21 Tape / Master Tape with position | PASS (software) |
| R22 provider lifecycle/PDC/state/category bypass/group-ready schema | PASS (software); tape-group linking not implemented (schema only, as specified) |
| R23 spill, independent send pan, gain staging | PASS (software); slot meters/I-O meter taps not added (PARTIAL) |
| R25 monitoring modes/compensation | modes + PDC PASS; calibrated placement and tracking deactivation PARTIAL |

## Carry-over completed after the A3 commit (before entering Sprint 4)
Mandatory items earlier reports listed as PARTIAL, implemented and tested (full CI exit 0, 616
passed — `docs/evidence/sprint_03/ci-sprint3-carryover.log`):
| Item | Change | Test |
|---|---|---|
| SL02 user Post inserts | `Insert.post_fader` (graph v2), `mix.insert_post` (stopped), insert-menu toggle, signal-flow readout | `mix sml_graph_post_inserts_follow_the_fader` (noncommuting fixture; legacy v1 unchanged) |
| CL03 Tracking deactivation | `mix.tracking_deactivate` / `mix.tracking_restore` (stopped; realtime-only, never saved, exports keep the normal graph); latency report lists latent processors | `engine cl03_tracking_deactivation_is_realtime_only` |
| CL04 latency change while recording | engine raises one event when compensation latency changes mid-take; UI finalizes the take, stops with a named error, resume is explicit | `mix latency_change_while_recording_raises_an_event` |
| R01/CL02/CL06 device selection | `Player::open_requested` (exact host/device/rate/buffer or error; old device kept); Playback Engine window: scan, choose device/buffer, apply stopped, requested vs reported vs effective callback, stream generation, Unknown clock lock/latency | negotiation fixtures (Sprint 1); UI not natively exercised (no devices here) |
| CL04 timestamp placement | output callback records (presentation instant, session sample); input records the first capture instant; takes are placed by `clock::place_by_timestamps` (driver terms inside the instants, residual/manual once), else transport estimate | `playback timestamp_placement_signs`, `playback_anchor_maps_instant_to_session_position` |

Still PARTIAL: background/cancellable plugin scanning; multi-select Visual drag; slot I/O meters;
simplified recording view; loop-record placement uses the loop start (not timestamps — closed in the Sprint 4 follow-up); native
verification of every item above (H1–H3).

### Carry-over batch 2 (full CI exit 0, 594 passed — `ci-carryover2.log`)
| Item | Change | Test |
|---|---|---|
| R04 module deletion | upstream processor implementations deleted; metadata only (see `plugin-removal-inventory.md`) | `ci-module-removal.log` |
| R06 background plugin scan | worker-thread CLAP/VST3/(AU) scan at startup, never started while recording, cancellable between formats; menus show progress/cancel/retry instead of blocking | `ui-egui plugin_scan::tests` (2) |
| R08 recording view | Window › Recording: input/output, selected track arm/monitor/meter, monitoring mode, Stop/Play/Record, Mixer | rendered `ui/recording_panel.png` (not natively exercised) |
| VM02 group gestures | Ctrl/Cmd+click multi-select; group drag with common dB/pan delta, relative offsets kept, −∞ member rejects vertical, Esc restores all | `ui-egui visual_mixer::tests::group_drag_deltas` |
| Nudge UI | legacy grid-style nudge value removed from the toolbar (dedicated NUDGE group is the only nudge value shown; `edit.nudge` command kept for compatibility) | — |
Remaining PARTIAL: slot/I-O meter taps in the inspector; rename/route badges in the Visual list. (Both closed in the Sprint 4 follow-up batch — see `SPRINT_04_REPORT.md`.)
