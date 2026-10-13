# Hotfix Pass 02 — native checks still pending

Everything in this pass was implemented and tested in **environment P**: a Linux x86_64 build
container with Rust 1.97.0, no display, no audio device, no Windows host, no plugins and no
listener. Nothing below has been observed on Windows. A cross-built Windows ZIP is **not** a
Windows test. Each item names what to do on the owner's machine and what counts as a pass.

Evidence states used here (from the package's VALIDATION_PLAN): **PASS_PORTABLE** (passed in
the container), **INSPECTED_SOURCE** (established by reading the source), **PENDING_NATIVE**
(needs the owner's Windows machine), **BLOCKED** (needs something this pass could not provide).

Before any test: save and close SML Studio, keep the installed build, back up
`%APPDATA%\SoundCraft`, extract the candidate into a new folder, and use a copy of a session (or
a new one), never the only copy of real work.

## HF12 — rescan through the control channel (P0)

| Check | State | How |
|---|---|---|
| Root cause (in-process rescan on the UI thread) | INSPECTED_SOURCE | `crates/engine/src/cmd/vst3.rs` at `cde8ae2` called `soundcraft_vst3_host::rescan()`, which loads every bundle in the app. |
| Jobs return at once; faults contained; coalescing; cancellation; retry; cache survives interruption | PASS_PORTABLE | `crates/mix/src/scan_jobs.rs` tests, `crates/engine/tests/plugin_scan_jobs.rs`, fault-injecting worker scripts. |
| Recovery point before a scan | PASS_PORTABLE | `crates/ui-egui/tests/hotfix02.rs::hf12_a_scan_request_writes_a_recovery_point_first`. |
| The owner's exact sequence on the real library (384 VST3, 1 CLAP) | PENDING_NATIVE | Start `soundcraft.exe --control 7979` with an empty, disposable session. Send `engine.execute engine.vst3_plugins {"rescan": true}`: expect `{job, coalesced}` within a second. On a **new** connection send `engine.clap_plugins {"rescan": true}`: expect the same job id with `coalesced: true`. Poll `engine.plugin_scan_status` until `done`. Pass: the app stays open and responsive throughout, the job finishes, quarantined entries have reasons. Record total time and the count of scanned/quarantined. |
| The same from the UI (insert menu › Rescan plugins; Cancel scan mid-way) | PENDING_NATIVE | Pass: the menu shows progress "n of N plugin files"; Cancel ends it within a few seconds; the app never exits. |
| Recovery after a real crash | PENDING_NATIVE | Not to be provoked on real work. If the app ever exits during a scan: restart, File › Recover Autosave… lists the point written before the scan. |

## HF13 — FabFilter scan compatibility

| Check | State | How |
|---|---|---|
| Exit code `-1073741819` was recorded as an ordinary failure; it is an access violation | INSPECTED_SOURCE + PASS_PORTABLE | classifier tests in `crates/mix/src/scan_isolated.rs`. |
| The scan worker did not run `ExitDll` on Windows (`std::process::exit` skips C exit handlers there) | INSPECTED_SOURCE | `apps/soundcraft/src/main.rs` (worker path) and `crates/vst3-host/src/ffi.rs` (`register_exit_hook` / `atexit`). Whether this caused the FabFilter crashes is **not known**. |
| Worker stage diagnostics, module exit run by the worker, architecture check | PASS_PORTABLE | `crates/mix/tests/scan_worker_fixture.rs` (real worker, VST3 fixture bundle). |
| The 13 FabFilter plugins and Timeless 3 | PENDING_NATIVE | After the HF12 run: `engine.plugin_scan_quarantine {}` → for each FabFilter entry note `reason`, `diagnostics.last_stage`, `exit_meaning`, `architecture`, `older_scanner`. Then `engine.plugin_scan_retry {"all": true}` and read the list again. Pass per plugin: listed in `engine.vst3_plugins {}` with its classes. Otherwise PARTIAL with the stage it reached. Record each file's version (Properties › Details) and SHA-256 locally; do not upload the binaries. |
| Insert, process, save, reopen and editor for each plugin that scans | PENDING_NATIVE | In a disposable session: insert, play, adjust a parameter, save, close, reopen, open the editor. |

## HF07 — Windows audio

| Check | State | How |
|---|---|---|
| Root cause of 128 → 480 frames (WASAPI shared mode; request silently accepted) | INSPECTED_SOURCE | `cpal-0.16.0/src/host/wasapi/device.rs`: `AUDCLNT_SHAREMODE_SHARED`, event callback; buffer range reported as 0–u32::MAX. |
| No silent acceptance; requested vs delivered; driver types listed honestly | PASS_PORTABLE | `crates/playback/src/device.rs` tests, `hotfix02.rs::hf07_*`. |
| Audio Setup on the SSL 2 (WASAPI) | PENDING_NATIVE | Driver type shows "WASAPI — Windows audio, shared mode", buffer control "set by Windows (shared mode)", Buffer (delivered) shows the actual frames (expected 480 at 48 kHz) and the smallest/largest. **Driver panel…** opens the Sound control panel. |
| 30-minute playback and 30-minute recording, xruns and dropouts | PENDING_NATIVE | A representative session; note callback spread, any dropouts heard or logged, CPU. |
| Device loss and reopen; saved device missing at launch | PENDING_NATIVE | Unplug the SSL 2 while stopped, then while playing; reconnect; Apply. |
| ASIO 64/128 frames | BLOCKED | Needs the owner to accept Steinberg's ASIO SDK licence (Steinberg's own or GPLv3) and a Windows build machine with MSVC and LLVM (`cargo build --release -p soundcraft --features asio`). The GNU cross-build cannot provide it. |
| WASAPI exclusive mode | BLOCKED | cpal 0.16 has no exclusive mode; needs a separate Windows audio backend (new unsafe FFI code; an architecture decision for the owner). |
| Loopback round-trip latency | BLOCKED | No calibration feature exists; buffer duration is not latency. |

## HF10 — clip dragging between tracks

| Check | State | How |
|---|---|---|
| Diagnosis (rows below the dragged one missing during the drag) and fix | PASS_PORTABLE | `crates/ui-egui/tests/clip_drag.rs` (7 tests through the real pointer path). |
| With a mouse on Windows | PENDING_NATIVE | Drag a clip from track 1 to track 3 and back (Smart tool, lower half); Ctrl-drag diagonally (time kept); onto an Aux (refused, red outline); Escape mid-drag; drag to the bottom edge to reach a track below the view; undo/redo; save, close, reopen. |

## HF08 — bottom bar, no scrolling toolbar

| Check | State | How |
|---|---|---|
| Target identified | INSPECTED_SOURCE + renders | The Edit toolbar's horizontal scroll strip; the owner's screenshots were not available to confirm. |
| No scroll areas; overflow menus; transport/Nudge reachable at 640–2560 points | PASS_PORTABLE | `toolbar.rs` unit tests, `ui_refinement.rs`, before/after renders in `evidence/`. |
| Owner confirms it is the bar they meant, at 100/150/200 % on their displays | PENDING_NATIVE | If a different row was meant, say which (one screenshot is enough). |

## HF09 — faders

| Check | State | How |
|---|---|---|
| Gain law, range and automation mapping unchanged; drag, fine, reset, keys, typed value, one undo each | PASS_PORTABLE | `hotfix02.rs::hf09_*`. |
| Look and feel compared with Logic by the owner | PENDING_NATIVE | Reference approval: the look is an interpretation, not a copy. |

## HF11 — detached Mixer

| Check | State | How |
|---|---|---|
| One session, one command per change, dock/undock, preferences | PASS_PORTABLE | `hotfix02.rs::hf11_*` (embedded window in the headless harness). |
| Native window on the owner's three displays | PENDING_NATIVE | Window › Detach Mixer; move the Mixer to display 2 and maximise; Edit on display 1; play and record; move a fader in each window; automation in Touch on one fader (applied once); restart the app (reopens detached on display 2); different scaling per display; disconnect display 2 while closed and restart (opens on the main display with a status message); close the Mixer window (docks, playback continues); quit. |

## Regression on Windows

Launch, Audio Setup, record a take with the SSL 2, Hotfix Pass 01 controls, the refinement pass
items still pending from earlier reports, and reference A/B (still the open defect K1).
