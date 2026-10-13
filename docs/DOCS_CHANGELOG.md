# Documentation changelog

Newest first. Each entry: date, documentation revision, source commit described, what changed,
how it was verified. The verification state itself lives in [DOCS_MANIFEST.json](DOCS_MANIFEST.json).

## 2026-10-13 — revision 2026-10-13.5 — Hotfix Pass 02, source `30df4a9`

**Changed**
- [USER_GUIDE.md](USER_GUIDE.md):
  - §2 capability table: scan jobs, WASAPI shared-mode buffer, clip drag between tracks, bottom bar,
    faders, detached Mixer.
  - §3.5: the transport is in the Edit/Record bottom bar.
  - §4.1, §4.3, §4.6: driver type, buffer only where the driver takes it, requested vs delivered,
    callback spread, buffer duration is not latency; ASIO and WASAPI exclusive not in this build.
  - §6.4: Count Off shown in the control bar.
  - §8.4 (new): dragging clips to other tracks, time lock, refusals, `edit.move_clips {from}`.
  - §12.1: NUDGE in the bottom bar; favourites in the unit menu; displacement on hover.
  - §13.2: fader look and handling; §13.5 (new): the Mixer in its own window.
  - §17.1: scan jobs, quarantine diagnostics, Retry, recovery point before scanning.
  - §22: the Edit/Record rows (tools on top, bottom bar, overflow menus).
  - §25.2: timeouts do not cancel; scans return jobs.
  - §26: K22 (FabFilter), K23 (WASAPI buffer), K24 (Arrow + Ctrl); §27.
- [grok/GROK_OPERATOR_GUIDE.md](grok/GROK_OPERATOR_GUIDE.md) and
  [grok/GROK_BOT_INSTRUCTIONS.md](grok/GROK_BOT_INSTRUCTIONS.md): scan jobs and polling, never
  forcing quarantined plugins, `edit.move_clips {from}`, detaching the Mixer, the bottom bar, no
  ASIO/WASAPI exclusive and buffer ≠ latency.
- Generated references regenerated (`cargo xtask docs`): `engine.plugin_scan_*`,
  `window.mix_detach`, `window.mix_dock`, `window.mix_window_status`, new parameter docs of
  `engine.vst3_plugins`, `engine.clap_plugins` and `edit.move_clips`.
- New: [hotfix_pass_02/HOTFIX02_REPORT.md](hotfix_pass_02/HOTFIX02_REPORT.md),
  [hotfix_pass_02/TARGET_PENDING.md](hotfix_pass_02/TARGET_PENDING.md), evidence renders.

**Verified:** source review of every chapter docs-check flagged at `30df4a9`, portable tests; not
observed on Windows.

## 2026-10-13 — revision 2026-10-13.4 — UI refinement pass 01, source `3aa6554`

**Changed**
- [USER_GUIDE.md](USER_GUIDE.md):
  - §4.2 and §6: the input level bar text.
  - §13.1: Visual view labels, clusters of coincident channels, the detail card, and the
    Comfortable/Compact density (`ui.mix_density`).
  - §22: the stacked toolbar and the transport moving to the control bar.
  - Reference A/B is now described as an **open functional defect** (K1) in §2, §18.1, §18.2,
    §24 E and §26. There is no menu entry for choosing a reference.
- [grok/GROK_OPERATOR_GUIDE.md](grok/GROK_OPERATOR_GUIDE.md) and
  [grok/GROK_BOT_INSTRUCTIONS.md](grok/GROK_BOT_INSTRUCTIONS.md): cluster chips, `ui.mix_density`,
  the control-bar transport, and K1 never to be reported as working.
- New: [refinement_pass_01/REFINEMENT_REPORT.md](refinement_pass_01/REFINEMENT_REPORT.md) and
  [refinement_pass_01/REFERENCE_AB_DEFECT.md](refinement_pass_01/REFERENCE_AB_DEFECT.md).

**Verified by:** source review against `3aa6554`; `tests/ui_refinement.rs` and all UI suites;
before/after renders.
**Not verified:** anything on Windows; reference audio (open defect).

## 2026-10-13 — revision 2026-10-13.3 — interface polish pass 01, source `59038e2`

**Changed**
- New [STYLE_GUIDE.md](STYLE_GUIDE.md): semantic colour tokens, type scale, metrics, control
  states, workspace patterns, how to check a change.
- [USER_GUIDE.md](USER_GUIDE.md):
  - §3.5 control bar: segments, the recording status, the device status mark, icon buttons, the
    layout buttons moving there on narrow windows.
  - §6 Record layout panel sections; §8 reading the timeline (selection edges, selected and muted
    clips, playhead).
  - §13.1–13.2 strip order, state tags, inserts, snapshots.
  - §18.1 Master cards; §20.1 the assistant's "ANSWERS FROM" line.
  - §22 SML Studio Dark and Graphite.
- [grok/GROK_OPERATOR_GUIDE.md](grok/GROK_OPERATOR_GUIDE.md) on-screen section and
  [grok/GROK_BOT_INSTRUCTIONS.md](grok/GROK_BOT_INSTRUCTIONS.md): new control locations;
  accessible names unchanged.
- Report: [polish_pass_01/POLISH_REPORT.md](polish_pass_01/POLISH_REPORT.md).

**Verified by:** source review against `59038e2`; `soundcraft-ui-egui` tests including
`tests/interface_polish.rs`; full `cargo xtask ci` (731 passed); headless before/after renders.
**Not verified:** anything on Windows or at the owner's display scale.

## 2026-10-13 — revision 2026-10-13.2 — Hotfix Pass 01, source `1d71089`

**Changed**
- [USER_GUIDE.md](USER_GUIDE.md): new §3.5 (control bar: BPM, time signature, Click, Count Off,
  Loop, Audio Setup and the device summary) and §6.7 (loop playback and the loop range, `loop.*`);
  §4.1–4.3 rewritten for the Audio Setup window (drafts with Apply/Cancel, remembered choices, a
  missing saved device, the selected track's input, the latency ledger); §4.5 monitoring choice;
  §5.3 the + Add Track button and the New Tracks layout; §6.1 the input selector lists the
  device's channels and links to Audio Setup; §6.5 Loop Record uses the selection; §13.1 the Split
  divider; walkthrough A step 1; capability table; K10 partly fixed.
- [grok/GROK_OPERATOR_GUIDE.md](grok/GROK_OPERATOR_GUIDE.md): tempo, time signature, loop and
  Split commands in the cookbook, the control bar for screen-access operators, device recovery
  through Audio Setup. [grok/GROK_BOT_INSTRUCTIONS.md](grok/GROK_BOT_INSTRUCTIONS.md) to match.
- Generated references regenerated (`loop.*`, `window.audio_setup`, `ui.mix_split`).
- Report: [hotfix_pass_01/HOTFIX_REPORT.md](hotfix_pass_01/HOTFIX_REPORT.md).

**Verified by:** source review against `1d71089`; UI tests `mix_split.rs`, `add_track.rs`,
`control_bar.rs`; headless CLI command check
(`docs/hotfix_pass_01/evidence/hotfix-commands.log`); offscreen renders; `cargo xtask docs-check`.
**Not verified:** any of it on Windows, with the owner's microphone/interface, or on the installed
copy (the installation could not be reached from the build container).

## 2026-10-13 — revision 2026-10-13.1 — source `018793f` (program code identical to build `b641b5a`)

**Changed**
- [USER_GUIDE.md](USER_GUIDE.md) rebuilt in place as the single authoritative manual: 28 chapters
  with procedures (purpose, prerequisites, controls, steps, result, undo/recovery, limits), a
  capability status table with evidence labels, a dedicated Nudge chapter, the full signal order,
  five professional walkthroughs (A–E), an external-control chapter, 21 known issues and a section
  map from the previous edition.
- Corrected claims of the previous edition: VST3 sidechain is routed; Bend joins are crossfaded
  (480 samples, not "10 ms" at every rate); the Master layout is a working workspace; Keyboard
  Shortcuts is under Setup; the Search menu item is Window › Pro Tools Search; Recording, Recover
  Autosave…, SML Remote, Assistant and Command Help are reached through Search; reference audio
  never plays; dither defaults to on; Tracking/Mixing monitoring modes are identical; take recovery
  shortens the capture file; the recovery window lists only the newest generation; the assistant
  accepts any loopback address; the phone forgets pairings when the service stops.
- New generated references from the registry: [COMMAND_REFERENCE.md](COMMAND_REFERENCE.md)
  (replacing the hand-run table), [reference/commands.json](reference/commands.json),
  [reference/SHORTCUTS.md](reference/SHORTCUTS.md) (with shadowed chords),
  [reference/CAPABILITIES.md](reference/CAPABILITIES.md).
- New: [README.md](README.md) (index), [grok/GROK_OPERATOR_GUIDE.md](grok/GROK_OPERATOR_GUIDE.md),
  [grok/GROK_BOT_INSTRUCTIONS.md](grok/GROK_BOT_INSTRUCTIONS.md), [DOCS_MANIFEST.json](DOCS_MANIFEST.json),
  [DOCS_MAINTENANCE.md](DOCS_MAINTENANCE.md), this changelog, the archived previous edition.
- Tooling: `cargo xtask docs` / `cargo xtask docs-check` (part of `cargo xtask ci`); packaging
  scripts include the documentation snapshot (Windows cross-build zip, Linux packages, Windows
  portable zip from `package.ps1` — the last not executed here, no PowerShell; not yet in the MSI);
  `packaging/docs/build_docs.py` builds the HTML/PDF companions and the Grok ZIP.
- Note: the manual is compiled into the app, so builds made from this commit on have new
  executable hashes; build `b641b5a` keeps the archived edition for its in-app assistant.

**Verified by:** source review against `018793f` (four cited audits), generated reference,
headless CLI runs of walkthroughs B, C and E on synthetic audio
(`docs/evidence/documentation/walkthrough-commands.log`), `cargo xtask docs-check`.
**Not verified:** anything on Windows, with an audio interface, microphone, third-party plugin,
phone or by listening (see manifest `verification.pending`). The installed build was not
accessible and is recorded as unverified.

## Earlier
The manual was written incrementally during the 2026-10 build cycles; that edition is archived
as [manual_archive/USER_GUIDE-0.5.0-b641b5a.md](manual_archive/USER_GUIDE-0.5.0-b641b5a.md).
