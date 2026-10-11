# Sprint 04 report — Target performance, owner workflow and delivery (gate A4)

Entered after A1–A3 portable PASS (with named PARTIAL rows) and the carry-over commits. Host:
Linux x86_64 container — no Windows machine, audio interface, driver, owner plugins or owner.

## Verdict
**A4 PORTABLE REGRESSION: PASS** — `cargo xtask ci` exit 0 on the final source (609 tests, fmt, clippy -D warnings, assets, layers, wasm), `docs/evidence/sprint_04/regression.log`.
**H1–H4 / CL-T6 / CL-T7 / owner review: BLOCKED FOR TARGET VALIDATION** (no target access). The
exact pending procedure is `docs/TARGET_MACHINE_GAUNTLET.md`. RELEASE READY: **NO**.

## Work done (contract items 1–5)
1. **Profiling (headless, release build)**: `crates/engine/examples/sml_profile.rs` — 16 playback +
   2 armed live-input tracks, Vocal Bus, FX return, Master, graph v2, width ≠ 100 %; 60 s of
   consecutive callbacks at 64/128/256 frames, 48 kHz; load = compute/(N/Fs).

   First run (before optimization; strips always distributed over rayon workers): p99 load
   0.217 / 0.116 / 0.046 at 64 / 128 / 256 frames with **13 / 45 000 overruns at 64 frames**
   (max 3.6 ms). A repeat comparing rayon against serial strips
   (`callback-profile-before-adaptive.csv`) showed the work-stealing scheduler costing 3–5× the
   strip work itself for light strips (64 frames: parallel p99 257 µs, 10 overruns; serial p99
   56.5 µs).

   **CL05 optimization (measured, then re-measured):** strips with no hosted processing render
   serially; worker threads are used only when ≥ `PARALLEL_MIN_HEAVY_STRIPS` (4) strips host a
   plugin, dedicated-slot provider or instrument (`MixEngine::parallel_strips` can disable them
   entirely). The strip work list is a reused buffer. Output is bit-identical in both modes
   (`serial_and_parallel_strips_render_identically`).

   **Second CL05 optimization (measured first):** `render` hashed the graph structure every block
   to detect session changes — measured 3.1 µs per block for 21 tracks (a temporary release
   micro-benchmark), ≈12 % of the 64-frame median. The playback engine now announces each
   session hand-off (`MixEngine::set_session_changes_signalled`, `session_changed` on
   `Cmd::Session`); offline renders keep the per-block check. Identical output for the same
   session sequence (`announced_session_changes_resync_the_graph`).

   Result, final run (`callback-profile.csv`; config order: per-block adaptive, per-block
   serial, playback):

   | Frames | budget | config | p50 | p99 | max | p99 load | deadline misses |
   |---|---|---|---|---|---|---|---|
   | 64 | 1333 µs | adaptive + announced changes (playback) | 23.3 µs | 54.0 µs | 2412 µs | 0.041 | 2 / 45 000 |
   | 64 | 1333 µs | adaptive + per-block check | 26.3 µs | 64.6 µs | 1950 µs | 0.048 | 2 / 45 000 |
   | 64 | 1333 µs | serial + per-block check | 26.3 µs | 61.3 µs | 2748 µs | 0.046 | 1 / 45 000 |
   | 128 | 2667 µs | adaptive + announced changes (playback) | 39.9 µs | 90.1 µs | 778 µs | 0.034 | 0 / 22 500 |
   | 128 | 2667 µs | adaptive + per-block check | 42.4 µs | 69.0 µs | 228 µs | 0.026 | 0 / 22 500 |
   | 128 | 2667 µs | serial + per-block check | 54.1 µs | 113.5 µs | 4768 µs | 0.043 | 1 / 22 500 |
   | 256 | 5333 µs | adaptive + announced changes (playback) | 83.1 µs | 148.6 µs | 2158 µs | 0.028 | 0 / 11 250 |
   | 256 | 5333 µs | adaptive + per-block check | 76.9 µs | 177.8 µs | 2147 µs | 0.033 | 0 / 11 250 |
   | 256 | 5333 µs | serial + per-block check | 76.4 µs | 151.9 µs | 2487 µs | 0.028 | 0 / 11 250 |

   **Run-to-run variance (6 runs, `callback-profile-runs.csv`):** the median drop is consistent
   (64 frames: 23.0–23.9 µs with announced changes vs 26.1–30.1 µs with the per-block check).
   Deadline misses at 64 frames occur in **every** config — 18, 11 and 12 per 270 000 callbacks
   (playback / per-block adaptive / serial; 0–9 per run in each config, no stable ranking),
   with maxima of 1.5–2.7 ms against a p99 near 50–65 µs. For this plugin-free session all
   three configs run the same serial strip code, so the misses are consistent with scheduler
   pauses of the shared VM, not with compute (an earlier run not kept in the file, playback
   config first, had 6). **The 64-frame overrun finding is therefore not closed** in this container:
   the rayon cost was removed, but zero misses cannot be shown here. 128 frames: 1 miss per
   135 000 in each config; 256 frames: none. An earlier single run showing "0 overruns" was a
   quiet run, not a stable result, and is superseded by these six.

   Allocation guard (`crates/mix/tests/realtime_alloc.rs`, `stats_alloc` global allocator):
   **0 allocations and reallocations over 200 steady-state blocks in serial mode** (graph v2,
   buses, sends, FX return, width, Tape-slot fixture, profile pans, live input). In parallel
   mode with 4 heavy strips, rayon's scheduler itself allocated 3 times in 200 blocks (reported,
   not asserted).

   Fader command acknowledgment (event → model revision, engine only): p50 13.7 µs, p95 25.1 µs,
   max 173.6 µs (1000 samples, `response-profile.csv`). These are container measurements with a
   shared, noisy scheduler — **not** native results and not a CL-T7 PASS. The native machine
   must still be checked at 64 frames with owner plugins (heavy strips → workers).
2. **Native H1–H3, CL-T6/7**: not executable here; gauntlet written with topologies, counts,
   thresholds and evidence file names.
3. **Native vocal workflow (VM-T8/V-E2E native, N07/N09/VM-T7/SL-T8, screen/scale matrix)**:
   not executable; the software V-E2E (Sprints 2–3) passes; UI renders exist for Console/Visual/
   Split and the Recording panel at 1600×1000 @2x only.
4. **Packaging** (inspected `packaging/linux/package.sh`, `packaging/windows/package.ps1`):
   - Linux x86_64 tarball built with the repo script (`--skip-build --formats tar` after
     `cargo build --release --locked -p soundcraft -p soundcraft-cli`); clean-extraction CLI smoke
     PASS (headless workflow + bounce); GUI cannot start without a display here.
   - Windows: `package.ps1` needs PowerShell + MSVC on Windows — not runnable. A **cross-built**
     unsigned portable zip (`x86_64-pc-windows-gnu`, mingw-w64; only Windows system DLL imports)
     was produced for owner testing. Cross-compiling is **not** a Windows runtime pass. WASAPI only
     (ASIO not compiled; needs the ASIO SDK on a Windows build host).
   - Artifacts (gitignored, local): see `artifact-manifest.json` for paths, sizes, sha256.
     Both were **rebuilt from `3eaef10`** after the follow-up batches (release build, tarball,
     clean-extraction CLI smoke and mingw cross-build re-run; logs overwritten with the new runs).
5. **Docs**: `docs/USER_GUIDE.md` (recording, Nudge, vocal comping, buses/FX returns, slots,
   profile/pan/width, Console/Visual/Split, monitoring/exports, device/latency/tracking, plugin
   risks — see §10), `docs/TARGET_MACHINE_GAUNTLET.md`.

## Follow-up batch (after the A4 regression, same rules)
Carried PARTIAL rows closed in software, each with a test (full `cargo xtask ci` rerun before
commit; result in `regression.log`):
- **R01 input device picker** — Playback Engine *Scan input devices* + picker; opens the device at
  the session rate and chosen buffer with every float input channel
  (`device::input_request`), stopped only; a failed request keeps the current input and is not
  remembered (`device::tests::input_request_captures_every_input_and_negotiates_exactly`,
  `tests::failed_input_switch_keeps_the_current_input_and_request`). Render:
  `ui/playback-engine-input.png`. Physical device behaviour remains H1.
- **R23 slot I/O meters** — every dedicated slot publishes input/output peak and an active flag
  with the strip meter (no extra allocation); Console SLOTS rows draw in/out bars and dBFS in the
  tooltip (`sml_graph_slot_io_meters_report_slot_input_and_output`: trim, bypass, deactivation,
  Summing on a bus, Master Tape on Main).
- **R19 Visual list** — route badge (destination + active sends), CLIP cue from the shared meter,
  double-click rename through `track.rename`; two-line rows so values fit the 230 px list
  (`route_badge_names_destination_and_active_sends`; render `ui/visual-list-badges.png`).
- **V07/CL04 loop recording placement** — loop passes are split at the capture index of the loop
  start derived from the timestamp (or transport) capture start: audio captured before the loop
  start is skipped, late capture shortens and shifts only the first take; punch-in while already
  looping is no longer misaligned (`loop_passes_follow_the_timestamp_capture_start`).
- **R20 automation arbitration** — recall refused while playing; a recalled static value never
  overrides a Read envelope in rendered audio; Off makes the recalled value audible; saving
  while playing captures static model values (`vmt5_snapshot_recall_and_read_automation_arbitration`).
- **V01/V03/V13 coverage** — V01 selection/zoom audio-neutral test and Tab clip-edge walk;
  V03 Smart-tool zone functions extracted unchanged and tested; V13 row updated to the Sprint 3
  `v13_named_export_paths` test.
- **R01/H1 input-channel integrity** (bug found in review): a take mapped track inputs modulo
  the capture width, so "In 5" on a 2-input device silently recorded input 1. Recording now
  refuses to start (and `add_recording` refuses atomically) when an armed track's input
  channels are missing; a one-input device feeding a stereo track stays dual mono. A failed
  take start also clears the player's recording flag
  (`recording_never_substitutes_another_input_channel`). Input monitoring had the same
  upstream modulo mapping: a missing channel now monitors silence instead of another input
  (`monitoring_never_substitutes_another_input_channel`).
  A take recorded from an input running at another rate than the session is resampled (as
  before) and the status line now says so (UI-only change; no test).
- **CL05 session swap** — replaced `Arc<Session>` returned to the UI thread for dropping
  (`replaced_sessions_are_dropped_off_the_audio_thread`).

## Evidence
`regression.log`, `release-build.log`, `profile-run.log`, `callback-profile.csv`,
`callback-profile-before-adaptive.csv`, `callback-profile-before-announced-sync.csv`,
`callback-profile-runs.csv`,
`response-profile.csv`, `package-linux.log`, `package-smoke.log`, `package-smoke.md`,
`windows-cross-build.log`, `artifact-manifest.json`.

## Requirements touched
R09 measured responsiveness: headless measurements only (PARTIAL; native pending). R10 regression
PASS, truthful candidate artifacts produced, manual written; native delivery checks pending.
R14 complete vocal walkthrough: software stages PASS; native/owner walkthrough pending.
