# Sprint 04 report — Target performance, owner workflow and delivery (gate A4)

Entered after A1–A3 portable PASS (with named PARTIAL rows) and the carry-over commits. Host:
Linux x86_64 container — no Windows machine, audio interface, driver, owner plugins or owner.

## Verdict
**A4 PORTABLE REGRESSION: PASS** — `cargo xtask ci` exit 0 on the final source (606 tests, fmt, clippy -D warnings, assets, layers, wasm), `docs/evidence/sprint_04/regression.log`.
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
   (`serial_and_parallel_strips_render_identically`). Result (`callback-profile.csv`):

   | Frames | budget | mode | p50 | p99 | max | p99 load | overruns |
   |---|---|---|---|---|---|---|---|
   | 64 | 1333 µs | adaptive | 25.7 µs | 55.4 µs | 950 µs | 0.042 | **0 / 45 000** |
   | 64 | 1333 µs | serial | 25.7 µs | 54.9 µs | 2948 µs | 0.041 | 2 / 45 000 |
   | 128 | 2667 µs | adaptive | 41.9 µs | 77.8 µs | 358 µs | 0.029 | 0 / 22 500 |
   | 128 | 2667 µs | serial | 41.8 µs | 63.0 µs | 819 µs | 0.024 | 0 / 22 500 |
   | 256 | 5333 µs | adaptive | 74.7 µs | 137.6 µs | 1704 µs | 0.026 | 0 / 11 250 |
   | 256 | 5333 µs | serial | 74.9 µs | 135.5 µs | 2165 µs | 0.025 | 0 / 11 250 |

   The profile session has no hosted plugins, so "adaptive" runs serially and the two rows
   measure the same code path; the 2 serial overruns and the multi-ms maxima are scheduler
   pauses of the shared VM (single run, p99 unaffected). Allocation guard
   (`crates/mix/tests/realtime_alloc.rs`, `stats_alloc` global allocator): **0 allocations and
   reallocations over 200 steady-state blocks in serial mode** (graph v2, buses, sends, FX
   return, width, Tape-slot fixture, profile pans, live input); with 4 heavy strips in parallel
   mode rayon's scheduler itself allocated 3 times in 200 blocks (reported, not asserted).

   Fader command acknowledgment (event → model revision, engine only): p50 13.3 µs, p95 19.9 µs,
   max 76.5 µs (1000 samples, `response-profile.csv`). These are container measurements with a
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
- **CL05 session swap** — replaced `Arc<Session>` returned to the UI thread for dropping
  (`replaced_sessions_are_dropped_off_the_audio_thread`).

## Evidence
`regression.log`, `release-build.log`, `profile-run.log`, `callback-profile.csv`,
`callback-profile-before-adaptive.csv`,
`response-profile.csv`, `package-linux.log`, `package-smoke.log`, `package-smoke.md`,
`windows-cross-build.log`, `artifact-manifest.json`.

## Requirements touched
R09 measured responsiveness: headless measurements only (PARTIAL; native pending). R10 regression
PASS, truthful candidate artifacts produced, manual written; native delivery checks pending.
R14 complete vocal walkthrough: software stages PASS; native/owner walkthrough pending.
