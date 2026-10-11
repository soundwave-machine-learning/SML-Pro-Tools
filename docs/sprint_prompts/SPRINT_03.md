# Sprint 03 — Mixer, slots, own plugins and synchronized Visual view
Revision 1.6 • PLANNED / NOT EXECUTED • gate A3.

## Objective and prerequisites
Deliver correct mixer graph/hosting/compensation and two usable synchronized views. Require A1 and A2 portable PASS. Stable IDs, immutable media, clock and edit contracts exist. Personal-plugin/native access may remain H pending.

## Read set and allowed scope
Read all current mixer and clock authorities: BUS_WORKFLOW_SPEC, MIXER_MODES_SPEC, MIXER_SLOT_SPEC, VISUAL_MIXER_SPEC, CLOCK_LATENCY_SPEC, relevant research, VALIDATION_PLAN and previous reports. Inspect crates/model mixer/session; crates/mix/src/lib.rs and actual graph/latency/panner workers; UI mix_window/widgets/dialogs/ops_windows; engine mix/clap/vst3/plugin_state commands; vst3-host/clap-host tests; playback snapshots/editors/state lifecycle.

Allowed: stereo mixer, bus/send/solo, external host fixes, slot/provider infrastructure, linear pan/width, monitoring/PDC, exports and Visual. Native summing/tape algorithms, proprietary extension loader, new host sandbox/AAX/AU-on-Windows work, VCA or additional channelstrip processor catalogue are excluded.

## Concrete work
1. Implement versioned graph order ONLY from SL02; preserve legacy behavior through explicit migration. New bus/Main neutral stereo identity avoids pan gain at every level. Follow/Independent sends branch before the route panner and apply one panner. B mute/solo semantics include shared returns, nested buses and explicit solo-safe exceptions. Validate cycles/format changes atomically.
2. Complete B01–06 easy bus/FX-return creation, names/outputs/spill/deletion/undo. Console strips expose input/trim/polarity, expandable regular inserts (five accessible, not forced tall at small screen), dedicated slots, at least two sends, pan/width, fader/mute/solo/arm-monitor, output/meter and accessible Main. UI labels show implemented taps; no hidden limiter/normalizer.
3. External VST3-first and preserved CLAP: background scan/cancel, native editor where supported, parameter fallback visibly labeled, state/reorder/remove/render/teardown and missing-provider placeholders. State capture/creation/destruction obey CL05. Record in-process crash-containment limitation; no claim arbitrary plugins are safe/compatible. No scans during recording if they risk capture.
4. Implement M01–05 clean/profile contracts and SL01–06 current slots: identity plus user host adapters, future source/bus hooks, Tape positions, provider metadata/state/trim/bypass/latency and group-ready schema. No model drive/noise/oversampling knobs until actual later research. Host plugin sees its declared bus buffer, not every track.
5. CL03/04/06: live-path latency ledger, Tracking/Mixing/Hardware Monitor, stopped deactivation/rebuild, full graph/parallel compensation and latency-change interruption policy. Temporary monitor overrides cannot silently affect normal-state exports. Clear normal/monitor difference notices; dry capture unchanged.
6. Implement VM01–05: one canonical gain/pan/width/state/automation model, Console/Visual/Split, meters/list, node gestures/focus/undo and A/B/C scoped snapshots. Full-width stereo-pan center can move using the visible bounded-span rule; 100% Width uses exact identity. Window visibility/geometry never changes audio.
7. Complete V12 host capability with deterministic fixtures, named owner pitch plugin only when available; finish V13 named Raw/Edited Dry/Processed/Full Mix export paths and V-E2E bus/host/export stages. No claim that arbitrary processed stems reconstruct a nonlinear full mix.

## Validation and observable completion
Run A3 software: B-T1..6, host lifecycle/format/state/render fixtures, M-T1/2/5/6, SL-T1..7 portable portions, CL-T4 and VM-T1..6. Preserve all recording/edit tests and full CI. M-T3/4/8 are DEFERRED BY OWNER; M-T7/SL-T8/VM-T7/8 plus personal plugins are native/owner H gates. A3 cannot fail merely because deferred sound models are absent, but missing slot/Visual/graph behavior is incomplete.

Artifacts under docs/evidence/sprint_03/: signal-flow.md, mixer/host/slot/compensation test logs, plugin-compatibility.csv, visual-mixer-coverage.md, buses/ logs, vocal-e2e-final.json and actual rendered UI captures. Update USER_GUIDE graph/slots/monitor/export/snapshots. Write docs/sprint_reports/SPRINT_03_REPORT.md. A3 portable PASS permits Sprint 4; graph/PDC/capture regressions block acceptance.

## Execution, report and continuation rules
Follow docs/AUTONOMOUS_BUILD_PROMPT.md for authorization, worktree, six-hour cap and stop/checkpoint policy. Applicable repository instructions remain mandatory. Local ordinary edits/test fixes are authorized by launching the master; no push/merge/install/signing/purchase or delegated agents. No test weakening or fabricated results. Obsolete catalogue-only tests may be retired with explicit rationale and preserved shared coverage.

Run the full inspected `cargo xtask ci` and relevant real feature commands before an accepted milestone commit. Record unavailable native checks as H pending and portable failures as blocking. Write the sprint report only after actual work, update living handoff/command index, inspect diff and print commit SHA after commit. A partial/WIP checkpoint never claims acceptance. At any stop write the final report/next action even if no later sprint ran.
