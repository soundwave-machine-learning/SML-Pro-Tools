# Target-machine gauntlet (H1–H4, CL-T6/T7, owner review)

Everything here is **NOT EXECUTED**: the build ran in a Linux container with no audio interface,
no Windows host, no owner plugins and no owner. This is the exact pending procedure. Record real
results under `docs/evidence/sprint_04/` (or a new dated folder) — never fill in expected values.

## 0. Artifact under test
- Candidate: `sml-recording-0.5.0-windows-x64-gnu-unsigned.zip` (cross-built with mingw-w64 from
  the commit printed in `docs/AUTONOMOUS_BUILD_FINAL_REPORT.md`; sha256 in
  `docs/evidence/sprint_04/artifact-manifest.json`). Unsigned (SmartScreen will warn).
  Audio host compiled: **WASAPI only** (cpal `asio` feature not enabled — needs the Steinberg ASIO
  SDK on a Windows build host; see §6).
- Preferred for release: build natively on Windows with `packaging/windows/package.ps1`
  (MSVC toolchain) — not runnable in this container.

## 1. H4 portable candidate smoke (≈30 min)
1. Extract to a fresh folder (no installer). Launch `soundcraft.exe`.
2. Setup › Playback Engine: *Scan output devices*, choose the interface, 48 kHz, 128 frames,
   *Apply*. Record: requested/reported/effective buffer, device name, any error text.
3. Same window: *Scan input devices*, pick the interface (opens at the session rate and the
   chosen buffer with all its inputs; on failure the message names why and the old input stays).
   Record the input line it then shows (device, channels, rate, requested buffer).
4. Window › Recording: select an audio track, *Arm*, speak; check the input meter and that a WAV
   appears in `<session>/Audio Files/` after stopping. Save, quit, reopen, play the take.
5. Load one owner VST3 on an insert and in a Tape slot; open/close its editor; save/reopen.
   During playback check the Tape slot's in/out meters move and differ by the slot trim.
6. Export: Full Mix and `file.export_path` Edited Dry/Processed for the vocal.
7. Screens: 1366×768 and 1920×1080 at 100/125/150 % scaling — Mix (Console/Visual/Split),
   Edit (NUDGE group), Recording window: controls readable, reachable, no overlap. Save images
   with resolution/scale in the filename; record hashes. On small/scaled screens the Console
   scrolls vertically to the faders and the Edit toolbar scrolls sideways to NUDGE/transport —
   check both are reachable and usable.
Evidence: `package-smoke-windows.md`, images.

## 2. H1 recording / clock (≈60 min)
- Same physical interface for input and output. 44.1 and 48 kHz, 64/128/256 frames where
  offered. Verify correct physical input channel (set the track Input to e.g. "In 3"; also
  confirm Record refuses with a named message when the input does not exist on the device), levels, dry file (insert a plugin on the
  track; the file must not contain it), no dropped/duplicated frames over a 10-minute take using
  an encoded test signal (e.g. a counting ramp or MLS from a second device), stop/finalize,
  crash test (kill the process mid-take; reopen; the take recovers up to the last checkpoint),
  unplug test (pull the interface's USB mid-take: recording must stop with a named message and
  keep the frames before the unplug; replug and press Record). With separate input/output
  devices, also unplug the output during a take: playback stops with a message, the take is kept.
- Evidence: `hardware-recording.md`, `recording-alignment.csv`.

## 3. CL-T6 latency and calibration (≈60 min)
- Overdub offset: line-out → line-in loopback (monitoring and phantom power off), ≥100 spaced
  impulses over 60 s, per rate/buffer, repeated after reopening the stream. Report raw offsets,
  median, spread, outliers, uncertainty. Enter the residual as `record.offset_samples` only if
  stable; a known-delay fixture (`test` delay provider is not shipped — use a plugin with known
  latency) must shift the measurement by its declared samples.
- Monitoring delay (separate topology): split one source to the DAW input and to an independent
  reference recorder channel; record the DAW's monitored output on the recorder alongside the
  reference; measure input-to-output. Proposed target ≤10 ms at a stable 64/128 setting (stretch
  ≤5 ms). Do not substitute the loopback number.
- Evidence: `rtl-matrix.csv` (topology, rate, buffer, n, median, spread, uncertainty).

## 4. CL-T7 / H3 stability and response (≥2 × 30 min + stress)
- Workload: 16 playback + 2 recording tracks, Mix window in Split view.
  (a) all slots empty; (b) declared owner plugins at fixed settings (list name/version/settings).
- Continuous capture, no transport changes; move faders/pans/width during the run. Zero xruns
  required; callback p99 < 0.8·(N/Fs); log every deadline miss. Separate short stress test:
  start/stop/seek/arm.
- UI: p95 event→visible ≤50 ms (Console↔Visual cross-view), frame time p95 ≤25 ms, no stall
  >100 ms; report measurement resolution. N09: 100 nudge key actions during playback on 16
  tracks / 2000 clips; VM-T7: 60 s continuous node drag + automation.
- Headless reference (this container, NOT native): the first profile showed **13 deadline
  misses at 64 frames**, caused by rayon scheduling of light strips (fixed: adaptive strips). Six
  later runs still show 11–18 misses / 270 000 callbacks at 64 frames in every config, with
  1.5–2.7 ms maxima against a p99 ≈ 50–65 µs. That is consistent with VM pauses but **unproven**:
  check 64 frames natively with both workloads. Workers are used with ≥4 plugin-hosting strips,
  so check workload (b) specifically.
- Evidence: `callback-profile.csv`, `response-profile.csv` (native), stress log.

## 5. H2 owner plugins and N07/N10, owner review
- Named owner VST3 EQ and dynamics (and a pitch plugin for V12 if available): 20 editor
  open/close cycles, automation of one parameter, save/reopen/render, bypass with latency.
  Evidence: `owner-plugin-matrix.csv` (name, vendor, version, format, result, notes).
- N07: physical numeric keypad vs main-row +/-, all modifiers, held repeat, text-field focus.
  Known limit: the UI toolkit cannot yet distinguish keypad from main-row keys and has no Win
  modifier, so Win+Num± (content nudge) is unavailable from the keyboard.
- N10 (optional claim gate): compare nudge behaviour with a licensed Pro Tools.
- Owner walkthrough (V-E2E/VM-T8/B-T7): three takes → comp → breath gain → crossfade → Nudge →
  Create Bus from Selection ("Vocal Bus", ≤3 interactions after selection, excluding typing) →
  FX return → Visual balance/width → Console change visible in Visual → A/B/C → save/reopen →
  named exports. Record observations; no invented approval.

## 6. Enabling ASIO (blocked here)
cpal's `asio` feature requires the ASIO SDK (`CPAL_ASIO_DIR`) and LLVM/clang on the Windows
build host; then add `features = ["asio"]` to the playback crate's Windows cpal dependency and
rebuild with MSVC. Verify the interface's own ASIO driver appears as host "ASIO" in the Playback
Engine scan. Not attempted here (no SDK, no Windows host, licensing to be accepted by the owner).
