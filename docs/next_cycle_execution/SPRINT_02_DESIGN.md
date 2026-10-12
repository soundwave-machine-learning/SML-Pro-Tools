# Sprint 2 design dossier — ownership, thread rules and interface decisions

Written 2026-10-12 before the Sprint 2 code, as the contract asks ("record actual ownership/thread
rules in the execution dossier"). Every item below is a decision about *this* repository; nothing
here is a claim that a test has passed. Statuses live in `REQUIREMENT_STATUS.csv`.

## Tool model (R11, EDIT_TOOLBAR_SPEC T01–T11)
- `soundcraft_model::Tool` gains the eight reference tools `Arrow, Range, Split, Eraser, Paint,
  Mute, Bend, Listen` (serde ids `arrow`…`listen`). The seven existing ids are unchanged, so
  saved sessions keep loading; `edit.tool` accepts all fifteen ids. The toolbar shows the
  reference row in inventory order and keeps the existing row.
- Tool state is session edit state (`EditState`), like `tool` already is: `tool_link` (Link
  Arrow/Range), `alt_tool` (held-modifier alternate), `paint_shape`, `paint_cycles`. New fields
  carry serde defaults; identities and ids of everything persisted are untouched.
- Gesture ownership (UI thread only): the effective tool is frozen at pointer-down into the
  `Gesture`; crossing hit zones cannot change the operation. Escape, focus loss and pointer
  release end a gesture; a sweep (Eraser, Mute) touches each clip id once and commits one
  command on release, so one gesture = one undo record. The alternate tool is active only while
  its modifier is held and Arrow is the selected tool; key release or focus loss clears it.
- Commands owned by the engine (`crates/engine/src/cmd/tools.rs`): `edit.split_at {at, clips?}`
  (eligible clips containing `at`; zero-length pieces rejected; locks reject atomically),
  `edit.erase_clips {clips}` (instances only; media preserved; locks atomic; no ripple unless
  Shuffle), `edit.mute_clips {clips, muted?}` (explicit target state), `clip.gain_draw` (the
  clip-gain envelope domain of `automation.draw`), `edit.tool_link`, `edit.alt_tool`,
  `edit.paint_shape`, `edit.paint_cycles`.

## Bend (T08, V11, E05)
- Edit map: `Clip.warp: Vec<WarpAnchor { src, dst }>` (samples, clip-relative; serde default
  empty) plus `Clip.warp_rev` (bumped by every anchor edit). Validation: strictly increasing
  `src` and `dst`, minimum spacing 64 samples, segment ratio within 0.25..4.0, anchors inside
  the clip; a violating edit is rejected whole. Grouped/stereo clips share one map (the group's
  members get the same anchors).
- Render service: `soundcraft_engine::warp::Renderer`. Commands `clip.warp_set {clip, anchors}`,
  `clip.warp_clear {clip}` mutate the map (undoable). The renderer runs on its own thread with a
  bounded queue (one active render per clip, one replaceable pending revision per clip,
  session-wide ceiling of 4 queued jobs); it stretches each segment with the in-house WSOLA
  (`soundcraft_dsp::offline::time_stretch`, E05 baseline) and never touches the audio thread.
  `clip.warp_poll` (UI/CLI, no undo record) applies finished results whose revision still matches
  the clip (stale results are dropped) by swapping the clip's content to a derived source in the
  pool; the original source is kept and `clip.remove_warp` restores it. Until a result lands the
  clip keeps its last valid render; `clip.warp_status` reports pending/error per clip.
- Undo of `warp_set` restores the previous anchors and content together (session snapshot undo).

## Listen (T09)
- `Session.audition_tracks` (serde skip, never saved) is a fixed 256-bit mask by track index so
  the realtime host copies it without allocating. Commands: `audition.start {tracks, at}` (only
  when the transport is stopped and not recording; snapshots the playhead and starts playback
  from `at` with the mask), `audition.stop` (stops, restores the playhead only if no other
  command ran meanwhile — the engine clears `audition.cursor_owned` on any other successful
  command), `audition.status`. Transport play/record first terminate audition. The mix engine
  treats a masked-out source like a solo-excluded one (same `soloed_out` path), so buses/returns
  needed by the audited tracks stay open and explicit mutes still win. No automation write, no
  record, no undo entry; the state is never serialized.

## Shortcut profiles (R13)
- `crates/ui-egui/src/shortcut_profiles.rs`: `Profile { ProTools, StudioOne, Custom }` persisted
  in `UiState.shortcut_profile`; `Custom` = a base profile plus `UiState.shortcut_overrides`
  (command id → binding, empty string = unbound). The Pro Tools table is the registry's
  `shortcut` strings plus the context keys documented in `shortcuts.rs`; the Studio One table is
  a static list in the same file. Resolution builds one `(modifiers, key) → id` map per profile
  change (cached in `SoundApp`); the registry strings are never edited by a profile.
- Conflicts: duplicate chords inside the active table, chords the OS reserves (Windows: Alt+F4,
  Alt+Tab, Win+*, Ctrl+Alt+Del; macOS: Cmd+Tab, Cmd+Space, Cmd+H/Q system chords) and chords
  egui cannot deliver (Super/Win, keypad location) are listed by the inspector with a visible
  alternate; none is bound silently. Text entry, dialogs, plugin editors and shortcut capture
  keep precedence (`handle` returns early).
- Inventory: `docs/evidence/next_cycle/sprint_02/shortcut-inventory.csv`. The reference manuals
  (S02, S03, S05) are unreachable from this environment (proxy 403), so every Studio One row and
  every Pro Tools row not already carried by the registry is `pending native verification`;
  rows with a working handler are `implemented-and-mapped`; the rest say why.

## Themes (R20)
- `theme::ThemeData` (serde): semantic colour tokens as `[r,g,b,a]`, waveform/meter palette,
  typography scale within 0.8..1.5, panel surface tokens. `Tokens::from_data` validates bounds
  and returns the first error; `theme::apply_data` installs a validated palette into a process
  `RwLock` (the UI thread owns it; painters read a copy per frame). Built-ins: Dark, Light, High
  Contrast. Import/export are JSON files through the host's file pickers; an invalid file reports
  the error and keeps the current theme; an invalid saved theme falls back to Dark at launch.
  Density, scaling and workspace placement stay separate preferences. No scripts, no network
  resources, nothing applied to plugin editors.

## Layouts and naming (R18)
- `MainWindow { Edit, Mix, Record, Master }` (UI prefs only). Record = Edit with the Recording
  panel docked and the track list hidden; Master = navigation shell showing the main strip and
  meters with a visible "incomplete until Sprint 4" label. Window title and About say "SML
  Studio"; session format, command ids, automation ids and plugin state keys are unchanged.

## Menu speed (R19, E04)
- Menu tree and command-palette entries are built once per process (`OnceLock`) instead of per
  frame/keystroke; plugin menus read the scan cache (R07) and never scan while a menu opens.
  Portable measurement: `crates/ui-egui/tests/menu_timing.rs` (egui_kittest, software GPU) at
  the W1 workload (16 tracks, 2000 clips), 100 repetitions, cold and warm recorded separately
  to `docs/evidence/next_cycle/sprint_02/menu-timing.csv`. Native W1/W2 distributions are a
  target gate (TARGET_PENDING).

## Remote (R21) and monitor (R37)
- `crates/remote`: hand-written HTTP/1.1 on `std::net::TcpListener` (no HTTP/TLS crates in the
  tree; HTTPS is therefore not provided and the configuration is labelled browser-only HTTP).
  Opt-in, explicit bind address (loopback by default), pairing code with a 120 s TTL and 5
  attempts, random session tokens in an HttpOnly cookie (never in URLs or logs), per-token
  revocation, role allowlist of command ids, Origin check (must match the Host the page was
  served from), rate limit and bounded queue (429 on overflow). Commands travel as R09 envelopes
  (client `Remote`, request id, expected revision) and are executed on the UI thread by
  `SoundApp::poll_remote`; the server thread never touches the engine. `transport.record` must
  carry the nonce of the snapshot the client last fetched, so a reconnect (new snapshot, new
  nonce) can never replay an old Record. Meters are coalesced to ≤ 10 Hz on a separate endpoint.
  Disconnects change nothing in the engine (the transport keeps running).
- Monitor: `MonitorControl` atomics (volume, dim, mono) on the playback output stage only, after
  fold-down; offline bounces never see them. Commands `monitor.volume/dim/mono/cue`; `cue`
  reports unavailable unless a dedicated cue output path exists in I/O setup.

## Knowledge (R22)
- `help.search {query, limit?}` (query command) over `envelope::knowledge` with the build
  version, status (`DisabledNotImplemented` for B04 toggles), risk and the active profile's
  binding supplied by the UI; `window.help` is the searchable window. Nonexistent tools return
  no entries (no invented commands).
