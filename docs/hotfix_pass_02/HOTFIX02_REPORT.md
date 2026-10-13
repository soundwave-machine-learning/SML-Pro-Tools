# SML Studio — Hotfix Pass 02 (HF07–HF13)

Date: 2026-10-13. Package: "SML Studio Hotfix Pass 02", revision 1.0 (seven tickets).
Environment: **P (portable)**, a Linux x86_64 build container with Rust 1.97.0. It has no
display, audio device, Windows host, third-party plugins or listener. **Nothing in this report was
observed on Windows.** Every native check is listed in [TARGET_PENDING.md](TARGET_PENDING.md).
Release readiness: **not ready** (native gates open).

## Source identity

| | |
|---|---|
| Checkout | `/home/user/SML-Pro-Tools`, branch `fable/recording-core` (not `main`) |
| Starting point | `cde8ae2` (= `origin/fable/recording-core`, 0 ahead / 0 behind at the start; clean tree) |
| Code commits | `9b06131` HF12+HF13 · `9ce3452` HF07 · `53b15b0` HF10 · `da2acf8` HF08 · `c8f46a0` HF09 · `30df4a9` HF11 |
| Documentation | the commit that adds this file (manual, Grok guide, references, evidence) |
| Candidate | see *Candidate* below. It is staged only: nothing was installed, pushed, merged or published. |

The package's screenshots (`image(5).png` … `image(20261013-070531).png`) were not in the package
and are not available here. HF08 and HF09 were decided from the real UI (renders) and the
tickets' text. The owner should confirm both (TARGET_PENDING).

## Ticket matrix

| Ticket | Result | Root cause / what changed | Native |
|---|---|---|---|
| **HF12** P0 rescan crash | **Fixed in source; PASS_PORTABLE** | Root cause (from the source): `engine.vst3_plugins/clap_plugins {rescan: true}` loaded every bundle **inside the app on the UI thread**, bypassing the isolated scanner. The control reply timed out after 90 s while loading continued in-process, where a plugin that access-violates ends the app. Fix: one scan service; jobs; hosts never scan in-process | PENDING_NATIVE |
| **HF13** FabFilter | **Diagnostics and a host-side fix; compatibility UNPROVEN (PARTIAL)** | Windows access violations were recorded as "exit code …" failures with no stage. Workers did not run `ExitDll` on Windows (`std::process::exit` skips C exit handlers there). Fix: stage trace, NTSTATUS decoding, architecture check, module exit run by the worker, selected Retry | PENDING_NATIVE |
| **HF07** Windows audio | **Honest negotiation and reporting; ASIO and WASAPI exclusive BLOCKED** | cpal 0.16 WASAPI opens shared, event-driven mode and reports an unlimited buffer range. A 128-frame request was accepted, then delivered at the engine period (480 frames at 48 kHz). Fix: WASAPI fixed buffers are not passed on; requested vs delivered (smallest–largest) shown; driver type row | PENDING_NATIVE / BLOCKED |
| **HF10** Clip drag | **Fixed; PASS_PORTABLE (real pointer path)** | Diagnosed with a real-pointer test. The move gesture started, but rows below the dragged one were not laid out yet when the drag looked them up, so a downward drag never changed track. The model also sent all clips to one track and removed a clip before validating its target | PENDING_NATIVE |
| **HF08** Bottom controls | **Implemented; target identified from renders (owner to confirm)** | The Edit toolbar was a horizontal scroll strip of about 2,300 points of groups (hover scrollbar; at 911×512 Record it stacked to 156 points). Now: a tools row on top, a one-row bottom bar, and labelled overflow menus | PENDING_NATIVE (owner confirmation) |
| **HF09** Faders | **Implemented (labelled interpretation); gain law unchanged** | New look and handling; `fader_db_to_pos` and `fader_pos_to_db` untouched | PENDING_NATIVE (look approval) |
| **HF11** Detached Mixer | **Implemented; PASS_PORTABLE as an embedded window** | An egui immediate viewport: same app, engine and session; geometry stored as a UI preference | PENDING_NATIVE (three displays) |

## HF12 — rescan timeout and host exit (P0)

**Baseline (INSPECTED_SOURCE at `cde8ae2`):**
- `crates/engine/src/cmd/vst3.rs` and `clap.rs` called `soundcraft_vst3_host::rescan()` /
  `soundcraft_clap_host::rescan()`.
- Those clear the registry and `scan_paths()` every bundle **in this process** (`registry.rs`
  `ensure_scanned`).
- The control channel runs `engine.execute` on the UI thread (`control.rs` → the app's single command entry point (`SoundApp::run`)) and
  answers `{"ok":false,"error":"timeout"}` after 90 s (`control_server.rs`). The scan kept
  running after that answer.
- The isolated scanner (`scan_isolated.rs`, worker processes) was used only by the start-up/UI
  scanner.
- So the owner's request mapped all 384 VST3 binaries into the DAW itself. A plugin that faults
  at load (the FabFilter workers access-violate when isolated) faults the DAW. That fits the
  report: a timeout, then the connection forcibly closed, and the process gone with no dialog.
- It was **not reproduced natively**; the exact causal sequence on Windows is unproven.

**Fix:**
- `soundcraft_mix::scan_jobs::ScanService`: one per process, installed by the app at start (not in
  Safe Open). The UI scanner, the start-up scan and the control channel all submit jobs to it.
- A submit returns at once with the job (`id`, `kind`, `state`, `total`, `completed`,
  `scanned_ok`, `quarantined`, `cached`, `current`, `requests`, `clap`, `vst3`, `elapsed_ms`).
- **Policy:**
  - one job runs; a request the running job covers joins it (`coalesced: true`);
  - otherwise one job waits, and further requests merge into it (the stronger kind wins);
  - up to 4 workers (half the hardware threads), each with a 20 s deadline;
  - cancellation stops new workers and kills running ones (only the service's own children);
    interrupted bundles are not recorded;
  - the cache is saved atomically (temp file + rename) every 8 bundles and at the end;
  - Retry forgets and rescans only the chosen paths.
- With the service installed, both hosts' `scan()`/`rescan()` never load a bundle in-process
  (`set_in_process_scan(false)`); the plugin list is adopted from the cache.
- **Legacy result shape:** `engine.vst3_plugins {rescan: true}` returns `{job, coalesced}` in the
  app instead of a list. Without `rescan` it returns the cached list as before. In
  `soundcraft-cli` (no service) a rescan stays in-process, as documented.
- **New commands:** `engine.plugin_scan_start {rescan?}`, `engine.plugin_scan_status {job?}`,
  `engine.plugin_scan_cancel {job?}`, `engine.plugin_scan_retry {paths | all}`,
  `engine.plugin_scan_quarantine`.
- **Recovery point:** the app writes one before any scan-starting command when the session
  has unsaved changes (`recovery_snapshot_now`). It goes into the same autosave store; the named
  session is never written and it stays modified.

**Tests (PASS_PORTABLE):**
- `scan_jobs::tests`: prompt return, fault containment (crash, hang, failure, garbage),
  coalescing and the single waiting job, cancel during a hung worker (killed in under 3 s,
  finished results kept, interrupted not quarantined), retry touching only the chosen bundle.
- `engine/tests/plugin_scan_jobs.rs`: the owner's control sequence. A legacy VST3 rescan returns
  a job in under 500 ms; the overlapping CLAP rescan joins it; quarantine diagnostics; explicit
  retry; cancel.
- `hotfix02.rs::hf12_a_scan_request_writes_a_recovery_point_first`.
- The pre-existing `isolated_scan_contains_faults_and_caches`, unchanged.

## HF13 — FabFilter scan compatibility and diagnostics

**Evidence held:**
- Owner reports: 13 FabFilter VST3 workers ended with exit code −1073741819 = `0xC0000005`
  (access violation), with empty stderr; Timeless 3 scanned.
- No binaries, versions or dumps are available here.

**Host-side findings (INSPECTED_SOURCE):**
1. The worker ends with `std::process::exit`. On Windows Rust calls `ExitProcess`, which does
   **not** run the C `atexit` handlers. The VST3 host relies on one to call each module's
   `ExitDll` before its DLL is unloaded (`ffi.rs`, `register_exit_hook`). The code's own comment
   records that some plugins crash in their static destructors without it.
2. So on Windows every scan worker unloaded plugins without `ExitDll`. On Linux, `exit` runs the
   handler, which hides the difference.
3. The sandbox worker (`--plugin-worker`) had the same pattern.
4. Whether this causes the FabFilter crashes is **not known**. The crash could equally be at load,
   entry, factory or class queries, which is why stage diagnostics were added.

**Changes:**
- **Stage trace:** workers announce each step on stderr before running it:
  - VST3: `locate_binary`, `load_library`, `module_entry InitDll`, `get_factory`,
    `factory_info`, `count_classes`, `query_factory2`, `class_info i/n`;
  - after output: `output_written`, `release_factory`, `module_exit`, `worker_exit`;
  - CLAP: the equivalents through `entry_deinit`.
- The host keeps the stderr tail (read to the end, so stage lines never block the pipe).
- **Classification:**
  - Windows NTSTATUS codes are decoded. Access violation, stack overrun, heap corruption and
    similar are *crashes*. Missing DLL, missing entry point and bad image are *unsupported* (a
    dependency or architecture problem, not a crash).
  - A crash after a complete plugin list is `UnloadCrashed`: still quarantined, with the count
    and stage.
  - Diagnostics stored per cache entry: last stage, exit code and meaning, output complete,
    classes reported, architecture (PE/ELF/Mach-O header), worker PID, elapsed time and scanner
    revision.
  - Old cache files still load; entries from older scanners are flagged `older_scanner` (worth a
    retry).
- **Architecture pre-check:** a binary for another architecture is reported without starting a
  worker. Unknown headers are tried, never refused.
- **Module exit:** the worker now calls `soundcraft_vst3_host::shutdown()` (runs `ExitDll` on
  the entry thread) and `soundcraft_clap_host::release_unused()` (`deinit`) before it returns.
  The sandbox worker calls `shutdown()` as well.
- **Selected Retry:** the insert menu's **Quarantined (n) › Retry** and
  `engine.plugin_scan_retry`. Unrelated quarantines and cached successes are untouched.
- Nothing was relaxed:
  - quarantine is still on;
  - no in-process fallback;
  - no metadata fabricated (an unload crash stays quarantined);
  - no plugin update suggested as the fix.

**Tests (PASS_PORTABLE):**
- `scan_isolated::classify_tests` (6): the owner's exact code, unload crash, dependency and
  architecture codes, ordinary failures and signals, old cache files, header architecture.
- `mix/tests/scan_worker_fixture.rs`: the real worker binary against the VST3 fixture bundle. It
  ends with `std::process::exit` like the app, lists 3 classes and ends at stage `vst3
  worker_exit` with exit code 0 (the fixture aborts if its module exit is skipped).

**Result:** PARTIAL. The FabFilter outcome is unknown until the owner retries on Windows.

## HF07 — Windows low-latency audio

**Backend audit (INSPECTED_SOURCE):**

| Item | Finding |
|---|---|
| WASAPI | cpal 0.16 (`Cargo.lock`) opens WASAPI **shared mode, event-driven** only (`host/wasapi/device.rs`: `AUDCLNT_SHAREMODE_SHARED`, `AUDCLNT_STREAMFLAGS_EVENTCALLBACK`). It reports a buffer range of 0–u32::MAX in software audio stacks. The callback period is the Windows engine period (usually 10 ms = 480 frames at 48 kHz). This explains the owner's "128 requested, 480 delivered". |
| Our negotiation | Accepted any buffer within the reported range, so 128 was "accepted" and passed as `BufferSize::Fixed(128)` with no effect: **silently not honoured**. |
| ASIO | cpal's `asio` feature is wired (`--features asio`, since RC07) but not built. It needs Steinberg's ASIO SDK (licence: Steinberg's own or GPLv3; the owner must accept it), LLVM/libclang and the MSVC toolchain. **BLOCKED** for this cross-build. |
| WASAPI exclusive | Not in cpal 0.16. It needs a new Windows audio backend: new unsafe FFI code, an architecture decision. **BLOCKED**. |

**Changes:**
- `device::buffer_control`: WASAPI is `EnginePeriod`. Negotiation opens with the driver default
  and records `applied_buffer: None` and `buffer_note`. Settable hosts still get exactly the
  request and still refuse sizes out of range.
- `device::driver_types`: compiled hosts, plus (on Windows) ASIO and WASAPI Exclusive listed as
  unavailable, each with its exact blocker.
- The player tracks the smallest and largest callback (lock-free `fetch_min`/`fetch_max` in the
  callback; no allocation).
- **Audio Setup, in order:** Driver type → output → input → sample rate → buffer.
  - **Driver panel…** opens the Windows Sound control panel for WASAPI.
  - The buffer choice is shown only where it can be applied; a saved WASAPI buffer gets a warning
    and **Use driver default**.
  - Delivered frames show as a single value or a range, with mode and rate, and an amber line when
    they differ from the request.
  - Durations show to 1/1000 ms (64 = 1.333 ms, 128 = 2.667 ms at 48 kHz) with "not round-trip
    latency".
  - The ledger adds "Buffer asked of the driver".
- Device changes still apply only while stopped; nothing changes the interface when the window
  opens.

**Tests (PASS_PORTABLE):** `device::tests::wasapi_shared_never_silently_accepts_a_fixed_buffer`,
`driver_types_state_what_this_build_can_use`, the existing negotiation tests unchanged, and
`hotfix02.rs::hf07_audio_setup_shows_driver_type_and_buffer_meaning`.

**Not met here:** 64/128-frame delivery on the SSL 2 (needs ASIO), 30-minute runs, xrun counts and
device loss. All are native; see TARGET_PENDING.

## HF10 — dragging clips between tracks

**Diagnosis:**
- A real-pointer test (`clip_drag.rs`, Smart tool, lower half, Kick → Snare) reproduced it: the
  `MoveClips` gesture started, but `to_track` stayed on the source track.
- `edit_window` cleared `edit_layout.rows` each frame and pushed each row just before drawing it,
  so a drag handled on the Kick row could not see the rows below it.
- A **tool and hit-testing defect, not a command defect**. Dragging upward worked.

**Model gaps found and fixed:**
- `edit.move_clips {track}` sent every selected clip to that one track.
- It removed a clip from its playlist before placing it, so a target without a playlist dropped
  the clip.
- A stereo clip on a mono track lost its right channel (the strip reads source channel `k mod n`).

**Changes:**
- Rows are laid out before any row draws.
- `move_targets` (each clip keeps its distance in tracks, counted over visible tracks that hold
  clips) and `move_target_problem` (track kind; clip channels ≤ track channels; mono on stereo
  allowed, as recorded mono already plays). Any invalid target refuses the whole move.
- **UI:**
  - ghost outlines at every destination, with the new start time;
  - red outline with the reason when refused; "Clip not moved: …" on release;
  - snapping measured from the grabbed clip's original start (it used whatever clip was under the
    pointer);
  - **Ctrl/Cmd while dragging = time lock** (vertical only, exact samples);
  - Escape cancels;
  - edge auto-scroll to reach tracks below the view;
  - the origin row keeps its events when scrolled away, so release always commits or cancels.
- One drag = one `edit.move_clips` = one undo step. Media is referenced, never rewritten. Fades,
  offsets and gain travel with the clip.

**Tests (PASS_PORTABLE, real pointer path):** down and up with one undo and redo; time lock with
a sideways move; grid snap; Escape (no revision change); refused Aux and stereo-to-mono (source
and track format unchanged) plus the command refusal; multi-track selection keeps relative
tracks; large tracks, zoom and a target below the view reached by edge scrolling; save and
reopen.

**Known limit (K24):** with the Arrow tool and an alternate tool set, Ctrl at *press* selects the
alternate tool (an existing gesture kept); press Ctrl after the drag starts.

## HF08 — no scrolling toolbar; controls at the bottom

**Target identified (INSPECTED_SOURCE + renders; screenshots unavailable):**
- The Edit toolbar (`toolbar.rs`) was the only horizontally scrolling row in the top area
  (`ScrollArea::horizontal` with ids `edit_toolbar`, `edit_toolbar_compact`,
  `edit_toolbar_stacked`).
- It held about 2,300 points of 52–64-point groups. On narrower windows they scrolled behind
  egui's hover scrollbar (light on the dark theme), and SHUFFLE and the audio tools were cut off.
- In the Record layout at 911×512 it stacked to two rows (156 points).

**New layout:**
- **Top (60 points):** edit modes, tools, audio tools, zoom.
- **Bottom bar (34 points, one row):**
  - locate (RTZ, rewind, fast forward, end);
  - **Stop / Play / Record + Loop** (the control bar's named chips);
  - counters (main, Start, End, Length; the sub counter on hover);
  - NUDGE (presets in its unit menu; displacement on hover);
  - Grid toggle and value;
  - output meter.
- Groups that do not fit go into **More tools… / More controls…** by priority. The transport is
  never moved there and becomes icon-only below 700 points. On a narrow row the menu button is
  "…", placed first, with the same accessible name.
- The dock tabs share the status line.
- Tempo, meter and count-off are no longer duplicated in the toolbar (they are in the control
  bar).
- Mix and Master keep the control-bar transport. The control bar's narrow-window transport
  hand-off (`transport_moved`) is gone.

**Measured editing space (renders, `evidence/compare-*.png`):**

| View | Track area before → after |
|---|---|
| Edit 1366×768 | 445 → 463 points (+18), nothing hidden, no overflow menu |
| Record 911×512 (1366×768 at 150 %) | 85 → 177 points |
| Edit 1920×1080 | +18 points, nothing hidden |

**Tests:**
- The toolbar unit tests were rewritten (RC09 intent kept): transport and Nudge on screen and not
  overlapping from 607 to 2560 points, light and dark; nothing in overflow from 1366 points; at
  611/340 points the overflow is used and the transport stays.
- `ui_refinement.rs::compact_record_toolbar_keeps_nudge_and_transport_reachable` was updated for
  the same reason, with its intent kept (documented in the test).

## HF09 — Logic-inspired faders

**This is an interpretation, not a pixel match.** No reference image was available.

- **Look:**
  - slim groove;
  - signless scale 6 3 **0** 3 6 10 15 20 30 40 60, labels never overlapping;
  - a unity line across the groove;
  - a larger cap (18–36 × 20–30 points, scaling with the strip) with a centre line, two-tone
    body and shadow;
  - hover, drag and focus edges;
  - the exact value beside the cap while hovered or dragged.
- **Handling:**
  - Kept: drag, Ctrl/Cmd fine drag (1/10), double-click or Alt-click to 0 dB, one undo per drag,
    the touch/automation path.
  - New: keyboard (↑/↓ 0.5 dB, Shift 0.1 dB, Home 0 dB; the vertical arrows are locked to the
    focused fader, so egui does not move focus), and a typed value in the dB readout
    (double-click; Enter = one undo; Escape cancels; "-inf" accepted; clamped to −144…+12).
  - Each key step or typed value is one undo step. The merge is closed for every non-drag change;
    while a fader has focus, the earlier logic would have merged them all.
  - Accessible as a slider "<track> volume".
- **Unchanged:** gain law, range, taper, smoothing, automation mapping.
- **Tests (PASS_PORTABLE):** law table and no write on hover; keyboard, typed value and undo;
  plain vs fine drag ratio (0.1) and one undo per drag; Console and Split use the same control.
- **Renders:** `evidence/after-mixer-*.png` at comfortable, compact and 960×540 at 200 %.

## HF11 — detached Mixer

- **Window › Detach Mixer / Dock Mixer**; commands `window.mix_detach {value?}`,
  `window.mix_dock`, `window.mix_window_status` (no existing command used these ids).
- **One engine:**
  - `ctx.show_viewport_immediate` draws the Mixer in a native window in the same frame, from the
    same `SoundApp`;
  - one engine, session and player; every change goes through the app's single command entry point (`SoundApp::run`), so it is applied once;
  - no second DSP graph and no event duplication;
  - on a backend without native viewports (the headless harness) it is an embedded window.
- **Content:** transport, a Dock button, and the full mixer (`mix_window::content`). The window
  handles the same shortcuts on its own input.
- **Main window:** detaching from Mix switches it to Edit; its Mix layout then shows a placeholder
  with Show Mixer window / Dock Mixer.
- **Geometry:** position, size, maximized state, and the display's size and scale are saved in
  `UiState.mixer_window` (`ui.json`).
- **Off-screen fallback:** a saved position that cannot be on a display reopens on the main
  display, with a status message (`usable_position`; egui reports no display names, so display
  identity is best-effort).
- Closing docks the Mixer; the transport keeps running.
- **Tests (PASS_PORTABLE):** `hotfix02.rs::hf11_*`. Edit and Mixer are visible at once; a mute in
  the Mixer is exactly one revision and one undo step; Dock keeps the transport playing;
  preferences round-trip; commands report state. Plus `usable_position` unit tests.
- **Not tested:** native windows, three displays, mixed DPI, a lost display, maximize behaviour.

## Gate and checks

| Check | Result |
|---|---|
| `cargo xtask ci` (fmt, clippy `-D warnings`, test, assets, layers, docs, wasm) | fmt, clippy, **test (766 passed, 0 failed)**, assets, layers: pass on `30df4a9` plus the documentation tree (`evidence/ci.log`). The docs step first **failed** on this report, which wrote a function name in backticks that the checker reads as a command id. I reworded it; then docs-check and wasm passed (`evidence/ci-remaining-steps.log`). No code changed between the runs. |
| Unit and integration tests added | 28 new tests: `scan_jobs` 3, `classify_tests` 6, `scan_worker_fixture` 1, `plugin_scan_jobs` 1, `device` 2, `toolbar` 2, `detached_mixer` 1, `clip_drag` 7, `hotfix02` 7, `ui_refinement` (rewritten) |
| Existing tests changed | `toolbar.rs` RC09 tests and `ui_refinement.rs` compact Record toolbar: they asserted the scrolling/stacked toolbar HF08 removes; rewritten with the same intent (transport and Nudge reachable, no overlap, tools in view). No other existing test was changed. |
| Evidence | `evidence/compare-edit-*.png`, `compare-record-*.png` (before `53b15b0` / after), `after-mixer-*.png`, `after-detached-mixer-embedded-1920x1080.png`, `render-scripts/`, `ci.log` |

Renders show layout only: not native DPI, font hinting, pointer feel, window-manager behaviour or
audio. The detached-Mixer render is the headless (embedded) form; on Windows it is a separate
window.

Build note: to fit the container's disk allowance, tests were built without debug info
(`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`). That changes nothing in the code or
the release candidate.

## Candidate (staged, not installed)

CANDIDATE_SECTION

## Open risks

1. HF12 is fixed at the root cause found in the source, but the owner's crash was not reproduced
   natively. A different trigger (for example a plugin crashing in the *app* when inserted)
   would still end the app: hosting is in-process unless sandboxed.
2. HF13: the FabFilter outcome is unknown. If the stage trace shows a crash before
   `output_written`, the next step is per-stage investigation with owner-local dumps.
3. HF07: low-latency monitoring on the SSL 2 needs ASIO (licence and build machine) or a new
   WASAPI exclusive backend: the owner's decision.
4. HF08 and HF09 were decided without the owner's screenshots.
5. HF11 multi-monitor behaviour depends on eframe/winit on Windows (untested). A mixed-DPI
   window move is handled by the window system.
6. Reference A/B remains open defect K1. SSL input/summing research is not in this pass.

## Next owner test

Follow [TARGET_PENDING.md](TARGET_PENDING.md) in this order:
1. Install side by side, then check identity (hash, version).
2. HF12 control-channel rescan in a disposable session.
3. HF13 FabFilter retry and the quarantine diagnostics.
4. HF07 Audio Setup with the SSL 2.
5. HF10 drag, HF08 bars, HF09 faders.
6. HF11 on three displays.
