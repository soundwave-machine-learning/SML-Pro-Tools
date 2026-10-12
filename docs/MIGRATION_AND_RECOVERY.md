# Migration and recovery guide (implementation-derived)

What this build reads, writes and recovers. Derived from the code of this cycle; nothing here is
a promise about an untested platform.

## Session files
- Extension `.scraft`, JSON, `format_version` 1. A file with a higher version than this build
  understands is refused with "format" (never silently down-converted).
- Fields added in this cycle are optional and omitted when absent, so older builds of the same
  format version open these sessions and simply ignore them:
  `Insert.sidechain` (bus id), `Insert.plugin_version`, `Session.mastering` (program source,
  reference, revision note), `EditState` tool/paint fields, `Clip.warp` / `warp_rev`,
  `Session.listen` (never saved). A session saved by this build and reopened in an older one
  loses the sidechain routes and the mastering setup, not audio.
- Plugin ids are stable: `clap:<id>`, `vst3:<class id>`, `au:<type>:<subtype>:<maker>`;
  `sandbox:<id>` marks an out-of-process instance of the same plugin (older builds treat it as
  a missing plugin: pass-through placeholder that keeps its state).
- Removed upstream bundled effects (`eq_7band`, …) load as inactive missing placeholders
  (`session.missing_plugins`), never as silently substituted processing.

## Media
- Audio lives beside the session in `Audio Files/` (float WAV written by Save; imported files
  keep their format). Derived Bend renders are media too (`<clip>-Bend.wav`); when one is
  missing at reopen the clip is re-rendered from its original and anchors (`clip.warp_poll`
  applies it). Original material is never rewritten by any edit.
- Recorded takes are written through the bounded capture writer with a journal
  (`<take>.journal`): after a crash, `File › Recover Autosave…` lists unfinished takes and
  recovers every journaled frame, reporting the uncertain tail separately.

## Autosave
- Rolling generations under `Autosave/<session>/gen-N.scraft` with an index; each generation
  is validated before it is published; the previous one is kept until the new one commits.
  Recovery scans the index and any newer validated file. See USER_GUIDE §1a for the defaults.
- Hosted plugin state in a generation can be older than the edits (captured while stopped);
  the recovery window shows that age. A sandboxed plugin that crashed contributes its last
  captured state (`engine.host_faults` records when).

## Preferences and themes
- Preferences and themes are JSON in the settings folder. A broken theme file falls back to
  Dark at launch; `--reset-prefs` (Safe Open) sets the preferences aside with a restorable
  backup; `--safe` loads with hosted plugins disabled (placeholders keep their state).

## Moving a project
- Copy the `.scraft` with its `Audio Files/` folder (relative paths). Absolute paths are kept
  as written; missing media is reported at open (`missing media: …`) and never guessed.
- Export reports (`<export>.report.json`, Sprint 4) record the build, revision, plugin
  ids/versions and the measured levels needed to reproduce an export.

## Not covered / unverified
- Power-loss durability beyond the tested abrupt-kill model (E02); cross-platform path
  separators in sessions saved on Windows and opened on macOS/Linux (relative paths are
  forward-slash normalised on write; a native test is pending).
