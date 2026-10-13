# Documentation changelog

Newest first. Each entry: date, documentation revision, source commit described, what changed,
how it was verified. The verification state itself lives in [DOCS_MANIFEST.json](DOCS_MANIFEST.json).

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
