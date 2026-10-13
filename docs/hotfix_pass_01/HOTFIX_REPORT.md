# SML Studio — Hotfix Pass 01 report

Package: `SML_Studio_Hotfix_01_Package` (START_HERE.md, CLAUDE_HOTFIX_PROMPT.md, docs/HOTFIX_SPEC.md,
docs/SCREENSHOT_INDEX.md, docs/VALIDATION_AND_HANDOFF.md, five reference screenshots — all read and
viewed before layout changes; `tools/verify_package.py` passed).

Date: 2026-10-13. Environment: **P (portable)** — Linux x86_64 build container, Rust 1.97.0,
egui 0.36 / egui_kittest 0.36, offscreen wgpu renderer; no Windows host, display, audio device,
microphone or interface. **Nothing in this report was observed on Windows.**

## Source identity

| | |
|---|---|
| Branch | `fable/recording-core` (not `main`; nothing pushed or merged by this pass) |
| Starting HEAD | `3cd6372151f502a5b20ba063c8c37e3879f1980e` (documentation commit on top of `018793f7…`, the last reported source; the delivered package `b641b5a` has identical program code to `018793f`). Working tree clean at start. |
| HF01 + HF02 + HF04 | `6b0693b7ac8bbdbf9a800578c7139a456c4bba2c` |
| HF03 + HF05 + HF06 | `1d710899b03880ee75c6d0f42eac45c6ff2e1023` |
| Documentation, report, update script | `6474f4a`, `053d448` (report bundled), and the commit recording the build below |
| Candidate build source | `053d448fbff4d7cf8f1bdd3e305deded90590cb7` (clean tree) |
| Installed build on the owner's computer | **not reachable from this session**; unidentified (manual §3.4 explains how to hash it) |

## Status per acceptance ID

Legend: **reproduced** = root cause confirmed in the source (and, where possible, in a headless
render); **portable-tested** = automated UI/engine tests driving the real app with pointer and
keyboard events; **native-tested** = on Windows with the owner's display and devices.

| ID | Reproduced | Implemented | Portable-tested | Native-tested | Installed |
|---|---|---|---|---|---|
| HF01 Split mixer + divider | yes (fixed 45 % split, no divider, overlay scrollbars made the Console look cut off) | yes | yes | **pending** | **pending** |
| HF02 + Add Track | yes (no visible add control beside TRACKS; only Track › New… / Ctrl+Shift+N) | yes | yes | **pending** | **pending** |
| HF03 BPM / time signature | yes (BPM was a read-only label inside the Edit toolbar, scrolled away in compact widths; nothing in Mix/Master) | yes | yes | **pending** | **pending** |
| HF04 New Tracks Name field | yes (Name was the last item of one fixed-width row, squeezed to a few characters) | yes | yes | **pending** | **pending** |
| HF05 Loop | yes (loop playback looped the edit selection only; no stored range, no lane, no numeric entry) | yes | yes | **pending** | **pending** |
| HF06 Audio Setup | yes (Playback Engine window: no input-channel context, choices not remembered, reconnect reopened the default) | yes | yes (no device in the container: honest "not open / Unknown" states, failure paths) | **pending** (needs the owner's microphone/interface) | **pending** |

No requirement was dropped. Partial items are listed under *Gaps*.

## What changed

### HF01 — Split mixer (commit `6b0693b`)
- `crates/ui-egui/src/mix_window.rs`: Split view lays Visual and Console into bounded panes with
  their own clipping and scrolling; a full-width divider ("Mixer split divider", resize cursor)
  between them. Drag to resize; each pane keeps a minimum (Visual 140, Console 160 points; tiny
  windows share proportionally); double-click or right-click › Reset split restores 45 %. The
  Console scrollbar is always visible.
- The share is a per-computer preference (`ui.json` `mix_split`, clamped 0.1–0.9, invalid or
  missing values fall back to 0.45) and `ui.mix_split {fraction}` / `{}`; it is never written to
  the session and never changes audio.

### HF02 + HF04 — Add Track and the New Tracks dialog (commit `6b0693b`)
- `crates/ui-egui/src/panels.rs`: a **+** button at the right edge of the TRACKS header
  (accessible name "Add Track", tooltip with the active profile's chord) opens the **existing**
  New Tracks dialog through the same menu path as Track › New….
- `crates/ui-egui/src/dialogs.rs`: the approved dialog is kept (count, format, kind, timebase,
  Create/Cancel); **Name** moved to its own full-width row (≥ 280 points, window min width 460).
  Two safety fixes found while testing: Master Fader is disabled when the session already has a
  Main (tooltip says why), and choosing Aux Input while Mono switches the format to Stereo.

### HF03 + HF05 + HF06 — the control bar, loop locators, Audio Setup (commit `1d71089`)
- `crates/ui-egui/src/control_bar.rs` (new): a top panel under the menu bar in every layout,
  wrapping onto a second row when narrow. **⚙ Audio Setup** + device summary; Stop/Play/Record +
  position in Mix and Master (the layouts without an Edit toolbar); **BPM** field; time signature
  popover; Click; Count Off; **⟲ Loop**; **Loop: … ▾** settings. It keeps no tempo, loop or device
  state of its own — every control runs an existing command.
  - BPM edits the tempo change in effect at the playhead (`event.tempo {bpm, at}`), 5–1000, Enter
    = one undo step, Escape / click-away cancel, disabled while recording, **MAP** badge when the
    map has more than one change (the map is edited point by point, never flattened).
  - Time signature: popover with Apply/Cancel → `event.meter` at the signature in effect.
- `crates/engine/src/cmd/loop_range.rs` (new) + `crates/model/src/session.rs`: stored loop
  locators (`EditState.loop_range`, serde default — older sessions open with none; the session
  format version is unchanged so older builds still open new files and ignore the field).
  Commands `loop.set {start,end | start,length}`, `loop.to_selection`, `loop.clear`,
  `loop.select`, `loop.info`; validated (start ≥ 0, end > start — checked before the `Range`
  constructor, which would otherwise silently swap an inverted pair); undoable; refused during a
  take.
- `crates/ui-egui/src/lib.rs`: Loop Playback loops the stored range, or the selection when none
  is stored (previous behaviour). **Loop Record is unchanged**: it records over the edit selection;
  `loop.select` / Select Loop Range copies the locators into the selection for that.
- `crates/ui-egui/src/edit_window.rs`: a **Loop** ruler lane — draw a range, drag either handle,
  drag the middle to move it, snap in Grid, Escape cancels, right-click menu, locked while
  recording. The drag uses the press position, so a fast drag on a handle moves the handle (found
  by the tests).
- `crates/ui-egui/src/windows.rs`: the Playback Engine window becomes **Audio Setup** (same window
  for Setup › Playback Engine…/Hardware…, the control bar, Search, and a strip's input menu).
  Output device, input device (microphone / interface), sample rate (session vs device), buffer
  requested vs delivered, monitoring (Software vs Hardware/direct, with the limits of what the app
  can control), the selected track's input ("In k", stereo "In k (inputs k–k+1)", channels the
  device lacks disabled, a warning when the track's input is not on the device), an input meter
  only for an armed/input-monitored track (real signal; "input: no signal" otherwise; no test
  signal), and **Advanced / Latency**: requested, driver-reported range, delivered callback,
  calculated output buffering (labelled an estimate), stream clock, input buffer, driver latency
  **Unknown**, plugin/host path delay (including sandbox blocks), measured round trip **Not
  measured**. Device choices are drafts until **Apply** (stopped only; output first, input only if
  the output succeeded); **Cancel** or closing discards them. Applied choices are remembered in
  `ui.json` `audio`; at launch the saved output is reopened, a missing one is named in the status
  line with the default kept and Apply retrying it; the microphone is not opened at launch. The
  window opens below the control bar so it never hides it.
- `crates/ui-egui/src/mix_window.rs`: a strip's input menu lists the open device's channels
  (In 1–8 with a note before it opens) and has **Audio Setup…**; strip I/O selectors gained
  accessible names ("Kick input", "Kick output").
- `crates/ui-egui/src/menus.rs`: UI commands `window.audio_setup {track?, value?}` and
  `ui.mix_split`. `apps/soundcraft/src/main.rs`: restore the remembered devices after preferences
  load (not with `--no-audio`).

Not changed: DSP, routing, plugin hosting, capture, the session format version, existing
commands' semantics, Nudge. No driver, SDK, gain/phantom/cue API, stock plugin or beat feature
was added.

## Tests

New UI tests drive the real application (`SoundApp::logic` + `ui`) with pointer/keyboard events:

| File | Tests | Covers |
|---|---|---|
| `crates/ui-egui/tests/mix_split.rs` | 4 | divider drag both ways with minimums at 911×512, 1366×768, 1920×1080 and 960×540 @ 200 %; double-click and menu reset; session unchanged by layout moves; Console scrolls inside its pane; window resize reflow; clamping of saved values; preference round trip |
| `crates/ui-egui/tests/add_track.rs` | 5 | + visible in Edit/Mix/Record × three sizes × demo/empty; Name ≥ 250 points inside the dialog with all approved controls; long Unicode name created once on Enter, Escape changes nothing, typing triggers no shortcuts; batch of 3 Aux = one undo, no rerouting, Master disabled, save/reopen; Ctrl+Shift+N and the menu open the same dialog |
| `crates/ui-egui/tests/control_bar.rs` | 11 | BPM visible/enabled in all four layouts at four sizes; Enter commits once (undoable, clips keep sample positions), Escape and click-away cancel, out-of-range refused, disabled while recording; tempo-map point edit keeps other changes; time-signature Apply/Cancel; Loop toggle never starts the transport; loop settings Apply, invalid input refused, Set Loop to Selection, draft discarded on close; Loop lane Escape cancel, end-handle drag (one undo), move keeps length, new range, selection untouched, context menu Select/Clear, locked while recording; playback stays inside the stored range and falls back to the selection; Audio Setup in one click from all four layouts and from a strip input (focused on that track) and Search; honest states without a device, no meter for an unarmed track, real meter state when armed, track input change is one undo; drafts Apply/Cancel/close, Apply refused while playing, a device that cannot open changes nothing and is not remembered; missing saved device named at launch with Apply retry; old preference files load |
| `crates/engine/src/cmd/loop_range.rs` | 2 | locator validation, separation from the selection, undo, recording lock; save/reopen and older files without locators |
| `crates/ui-egui/tests/clip_menu.rs` | changed | finds the Kick lane and the Delete item where they are drawn instead of fixed pixels (the new bar moved them down 26 points); the assertion is unchanged |

Headless CLI command check: `evidence/hotfix-commands.script.txt` → `evidence/hotfix-commands.log`
(tempo/meter/loop commands, undo, save and reopen keep the loop range).

Gate: see *Gate results* below.

## Evidence (`docs/hotfix_pass_01/evidence/`)

Headless offscreen renders of the real app (demo session, wgpu software path, no audio device —
hence "No audio output"). They show layout only; they do **not** prove native drag, keyboard or
device behaviour.

| File | Shows |
|---|---|
| `hf01-mix-split-911x512.png`, `hf01-mix-split-1920x1080.png` | Split view with the divider (• • •), Console scrollbar |
| `hf02-edit-911x512.png` | + beside TRACKS; control bar wrapped onto two rows |
| `hf03-hf05-edit-1920x1080.png` | BPM with MAP badge (two tempo changes), Loop on, Loop lane with a stored range |
| `hf03-record-911x512.png`, `hf03-master-911x512.png` | control bar in Record and Master (with Stop/Play/Record in Master) |
| `hf03-edit-960x540-200pct.png` | 200 % scale |
| `hf06-audio-setup-1366x768.png`, `hf06-audio-setup-911x512.png` | Audio Setup below the control bar, scrolling inside a small window |
| `hotfix-commands.*` | CLI command check |

No before/after **native** screenshots: the Windows machine was not accessible.

## Gaps and pending items

1. **Native (Windows) checks — all six IDs.** Open the candidate at the owner's display scale and
   at 1366×768 @ 150 %: drag the Split divider and restart (share kept); + Add Track; type a BPM
   and press Enter / Escape; Loop lane drags; Audio Setup with the owner's microphone/interface:
   Scan, choose, Apply, arm a track and see its real input on the meter, restart and confirm the
   device is remembered; unplug the interface and restart to see the missing-device message.
2. **Installation pending.** This session cannot reach the owner's computer, so nothing was
   installed. The candidate zip, its manifest, checksums and `Update-SMLStudioCandidate.ps1` are
   delivered for the owner to run (see *Build and delivery*).
3. **Update script not run on Windows.** It was exercised with PowerShell 7.4.6 on Linux against
   the real zip (see *Build and delivery*); Windows PowerShell 5.1 and Windows file locking are
   untested — run it with `-WhatIf` first.
4. Remaining from K10: **Reopen the system default output** (Advanced) still opens the default,
   not the remembered device; no recording-offset control.
5. Not in scope / observed in renders, pre-existing: at 911 points wide the layout buttons overlap
   the end of the menu bar ("Window" is cut to "Win"); the Edit toolbar in compact widths scrolls
   horizontally (Nudge stays reachable by scrolling). Not changed in this pass.
6. Driver latency stays **Unknown** and round trip **Not measured** by design (no backend report,
   no calibration in this build).

## Gate results

Command: `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only cargo xtask ci` (fmt, clippy
`-D warnings`, `cargo test --workspace`, assets, layers, docs-check, wasm) on the code of
`1d71089` plus this commit's documentation.

| Step | Result | Log |
|---|---|---|
| fmt, clippy, test | pass — 76 test binaries, **726 passed, 0 failed** | `evidence/ci-attempt1-assets.log` |
| assets | first run **failed**: the new evidence PNGs had no `ATTRIBUTION.md` row; row added (directory `docs/hotfix_pass_01/evidence/`), then pass | `evidence/ci-attempt1-assets.log`, `evidence/ci-remaining-steps.log` |
| layers, docs-check, wasm, fmt --check | pass | `evidence/ci-remaining-steps.log` |

Only `ATTRIBUTION.md` changed between the two runs, so the test step was not repeated. Earlier in
the pass, `cargo test -p soundcraft-ui-egui` (all UI suites) and the engine/model suites passed
after each group; `clip_menu.rs` was the only existing test affected by the new layout (above).

## Build and delivery

Built with the repository's script: `OUT=dist/hotfix01-windows bash packaging/windows/cross-build.sh`
(GNU cross-build from Linux, `x86_64-pc-windows-gnu`, rustc 1.97.0, release, `--locked`; WASAPI;
no ASIO, unsigned, no installer). It includes the documentation bundle (manual, references, Grok
guide, this report; `docs_revision` 2026-10-13.2). Not executed on Windows.

| Artifact | SHA-256 |
|---|---|
| `soundcraft-0.5.0-windows-x64-gnu-portable.zip` | `9189ff1636d3a2604a910a17952348a245df6bbdbc9a9c43dd45eb10ce38a789` |
| `soundcraft.exe` (inside) | `192521f6211b3260bea1dc7a922810f3060758ea27183e7f3dd704dd94d6d3d4` |
| `soundcraft-cli.exe` (inside) | `57d06c7d362092e4f5b91271be799d2770f9de0a752875814d1d6841e37669b7` |
| `MANIFEST.json` (beside and inside the zip; source commit `053d448…`, `dirty: false`) | `c626274f84ffabe6f5f6b86b25462beec1b97acf4e486e9e0d43e8a3b7c9da04` (the copy beside the zip) |
| `Update-SMLStudioCandidate.ps1` (`packaging/windows/`, delivered beside the zip) | `6b47db77e03c9788bb92aee4b6442b610d4c9f0e05ea75df9d26e83370ec0bde` |

**Update script** (`packaging/windows/Update-SMLStudioCandidate.ps1`): checks the zip's SHA-256;
records the old build; refuses while `soundcraft.exe`/`soundcraft-cli.exe` runs from the folder
(never stops it); unpacks to a staging folder and checks every file against the zip's
`MANIFEST.json`; renames the old folder to `<folder>.backup-<timestamp>` and moves the new build in
(restoring the old one if anything fails); carries over anything in the old folder that did not
come from a package (for example sessions saved there); copies `%APPDATA%\SoundCraft` top-level
settings files into the backup without changing the originals; writes a log; `-Rollback` restores
the backup. Test on Linux with PowerShell 7.4.6 (wrong checksum, app running, `-WhatIf`, update
keeping a session folder and settings, rollback to the identical old binary):
`evidence/update-script-test.log`.

**Installation status: pending.** The owner's Windows computer is not reachable from this
session; nothing was installed and no claim is made that the installed copy changed. The zip,
its `.sha256`, `MANIFEST.json` and the update script were attached to the conversation.

**Exact next action (owner, on Windows):**
1. Put the zip, its `.sha256` and `Update-SMLStudioCandidate.ps1` in one folder. Save your
   session and close SML Studio.
2. In PowerShell in that folder:
   `powershell -ExecutionPolicy Bypass -File .\Update-SMLStudioCandidate.ps1 -InstallDir "<the folder you run soundcraft.exe from>" -WhatIf`,
   then the same without `-WhatIf`.
3. Start SML Studio; `Get-FileHash .\soundcraft.exe` should print `192521f6…94d6d3d4`. Then the
   native checks in *Gaps* item 1. To go back: the `-Rollback` line printed by the script.
