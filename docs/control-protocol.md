# Control protocol

SoundCraft can be driven completely by other programs: scripts, test harnesses and AI agents.
Every menu item, button, mouse gesture and keystroke maps to a **command** (or to synthetic
input), and every command is reachable from:

| Surface | How |
|---|---|
| Desktop app control channel | `soundcraft --control <port>` or env `SOUNDCRAFT_CONTROL_PORT` (`0` = pick a free port, printed as `SOUNDCRAFT_CONTROL_PORT=…` on stdout; a Windows release build has no visible console, so use a fixed port there) |
| CLI against a running app | `soundcraft-cli app --port <port> <method or command id> [JSON]` |
| Headless CLI | `soundcraft-cli run --demo --cmd 'mix.volume={"track":"Kick","db":-6}' --bounce out.wav` |
| MCP (headless) | `soundcraft-cli mcp [--demo]` |
| MCP (bridged to the app) | `soundcraft-cli mcp --connect <port>` |
| Headless authenticated server (SDK tests, scripts) | `soundcraft-cli serve [--port P] [--demo]` (prints `SOUNDCRAFT_CONTROL_PORT=…`; `ui.*` methods need the desktop app) |
| Python / JavaScript SDKs | `sdk/python/sml_studio.py`, `sdk/js/sml-studio.mjs` (local files; see `sdk/README.md`) |

## Isolated automation profiles

Set `SOUNDCRAFT_CONFIG_DIR` to a fresh directory to isolate UI preferences, `Autosave`, `Presets`, and `logs`.
The nonempty value is used as the complete settings directory. Unset retains the normal platform paths;
an explicit empty value disables the settings directory instead of falling back to a personal profile. Save synthetic sessions to explicit scratch paths. Do not use `SOUNDCRAFT_NO_PREFS`
when testing preference persistence across restarts.

Run UI acceptance with `--no-audio`. This disables the output player; it does not disable recording
input opened by arm/monitor commands. An automated UI smoke run must never arm a track, enable input
monitoring, or start recording. Control input injects egui events; it does not establish operating-system
input delivery or screen-reader acceptance. Native screenshots use the presented viewport frame.

## Wire format (protocol 2)

Newline-delimited JSON over TCP on `127.0.0.1`, UTF-8, at most 1 MiB per line. The schema is
`sdk/schema/sml-control-v2.schema.json`. The **first** message of every connection must be `auth`:

```json
{"id": 1, "method": "auth", "params": {"token": "<token>", "client": "my-script", "protocol": 2}}
{"id": 1, "ok": true, "result": {"protocol": 2, "app_version": "0.5.0", "authenticated": true}}
```

Then one request per line, one reply per line:

```json
{"id": 1, "method": "engine.execute", "params": {"command": "track.new", "params": {"count": 2, "format": "Stereo"}}}
{"id": 1, "ok": true, "result": {"tracks": [12, 13]}}
```

Errors: `{"id": 1, "ok": false, "code": "error", "error": "bad parameters for `mix.volume`: no track named `Nope`"}`.

| `code` | Meaning |
|---|---|
| `auth_required` | The first message was not `auth`. The text says where the token file is (migration from protocol 1). The connection is closed. |
| `unauthorized` | Missing, wrong or revoked token. The connection is closed. |
| `protocol` | The client asked for another protocol version. |
| `busy` | Too many connections (8). |
| `bad_request` | Not JSON, or a line over 1 MiB. |
| `timeout` | No reply within 90 s. The request was **not** cancelled: check the state before retrying. |
| `closed` | The app is closing. |
| `error` | The request ran and was refused (the text says why). |

**Token.** The app creates the token file `control-token` in its settings folder at start if it
does not exist (Windows `%APPDATA%\SoundCraft\control-token`; elsewhere the platform settings
folder, or `SOUNDCRAFT_CONFIG_DIR`; mode 0600 on Unix). Format `smlc_` + 64 hex digits (32 random
bytes from the OS). It is compared in constant time and never logged; the app shows it redacted
(`smlc_…` + last four). Manage it with **Setup › External Control...** (Copy, Rotate, Revoke) or
`soundcraft-cli token path | show | rotate | revoke`. Rotation affects new connections only;
revocation refuses every new connection until a token exists again. Clients (CLI `app`, MCP
`--connect`, SDKs) read `SOUNDCRAFT_CONTROL_TOKEN`, else the token file.

**Security and limits.** The channel binds `127.0.0.1` only. Authentication must finish within
10 s; an idle connection closes after 30 minutes; at most 8 connections. `id` is echoed;
`command` is an alias of `engine.execute` and `document.inspect` of `session.inspect`.

**No replay.** A client must never re-send a request automatically once it was written to the
socket: it may already have run. After a timeout or a lost connection, inspect the state
(`session.inspect`, `engine.history`) and decide. The CLI, MCP bridge and SDKs follow this rule.

**While recording**, `session.new`, `session.open`, `session.close` and `session.revert` (and
recovery from Recover Autosave) are refused with "a take is being recorded…"; nothing changes.

**Migrating from protocol 1 (before Hotfix 03):** add the `auth` message as the first line. A
protocol 1 client receives `auth_required` with the token file's location and is disconnected.

## Methods

| Method | Params | What it does |
|---|---|---|
| `engine.execute` (alias `command`) | `{command, params}` | Run a command programmatically. Never opens a dialog — except `ui.theme_import`, `ui.theme_export`, `ui.shortcuts_import` and `ui.shortcuts_export` called without `path`/`text`, which open a native file picker. Empty params use defaults or the current selection. No confirmation: `session.new/open/close` discard unsaved changes (refused while recording). |
| `engine.commands` | `{filter?}` | Every command: id, label, menu path, shortcut, params doc, enabled + reason. Includes UI-layer commands (`window.*`); those rows ignore `filter` and always report enabled with params `{}`. Undo/risk metadata: `help.search`. |
| `engine.parity` | `{}` | Parity report against the incumbent's menu catalog. |
| `session.inspect` | `{detail?: "summary"|"full"}` | The document: tracks, clips (with times), inserts (+params when full), sends, routing, automation, markers, groups, busses, sources (`loaded` true when decoded audio is in the pool), selection, transport, undo label. |
| `ui.inspect` | `{}` | UI state (`window`, panels, dialogs), window size, playing/position, audio device, and `edit_layout` (timeline rect and every track row's rect, for clicking). |
| `ui.set` | any `UiState` fields | e.g. `{"window": "Mix", "narrow_mix": true}`. |
| `ui.menu.list` | `{}` | Every catalog menu path and the command that implements it (or null). |
| `ui.menu.invoke` | `{path}` or `{id}` | Like clicking the menu item: may open its dialog (e.g. `"Track > New..."`). |
| `ui.click` | `{x, y, button?, count?, cmd?, shift?, alt?, ctrl?}` | Synthetic click (window points). |
| `ui.drag` | `{x, y, to_x, to_y, steps?}` | Synthetic drag with the primary button. |
| `ui.move` | `{x, y}` | Move the pointer. |
| `ui.key` | `{key, cmd?, shift?, alt?, ctrl?}` | Key press (`Space`, `Enter`, `A`, `F7`, `=`…). |
| `ui.text` | `{text}` | Type text into the focused field. |
| `ui.screenshot` | `{path?}` | PNG of the window (base64 in `png_base64` when no path). |
| `app.quit` | `{}` | Quit. |

## Positions and references

- Tracks: id (number) or name (`"Kick"`).
- Clips: id (from `session.inspect`).
- Positions: samples (`96000`), seconds (`{"seconds": 2.5}`), or a time string in any timebase,
  optionally prefixed: `"bars_beats:5|1|000"`, `"min_secs:0:12.500"`, `"timecode:00:00:10:00"`,
  `"samples:48000"`.
- Ranges: `start` + `end` (or `start` + `length`); omitted = the current edit selection.

## Examples

```sh
soundcraft --demo --control 7979 &        # creates the token file if needed        # a fixed port; with 0, read the port from the app's stdout or log
P=7979
soundcraft-cli app --port $P mix.solo '{"tracks":["Bass"]}'
soundcraft-cli app --port $P transport.play
soundcraft-cli app --port $P ui.screenshot '{"path":"/tmp/shot.png"}'
soundcraft-cli app --port $P ui.menu.invoke '{"path":"Track > New..."}'
```

`soundcraft-cli app` wraps a bare command id in `engine.execute`, except ids that begin with `ui.`,
`engine.`, `session.` or `app.`, which are sent as methods; send those commands explicitly:
`soundcraft-cli app --port $P engine.execute '{"command":"session.save","params":{"path":"/tmp/x.scraft"}}'`.

Offscreen (no window): `cargo run -p soundcraft-ui-egui --example ui_shot -- out.png script.jsonl`,
where each script line is a request (`{"method": …, "params": …}`), `{"shot": "path.png"}` or
`{"steps": n}`. Use `--system-theme light|dark|none` to supply the initial OS appearance, and
script events such as `{"system_theme": "dark"}` to change it on subsequent frames.

Appearance defaults to Dark. `ui.theme {"mode":"system"}` follows the appearance supplied by
the native or web integration at launch and on later frames, while the saved choice stays System.
When the integration cannot report an appearance, System uses Dark. Manual Light and Dark stay
fixed when the OS appearance changes. An offscreen render without `--system-theme` supplies no
OS appearance and therefore resolves System to Dark.

## Panel docking

Tracks and Groups, Clips, MIDI Editor, Memory Locations, and Undo History use movable panels. Commands target the active Edit or Mix view; floating panels stay inside the current viewport. Edit and Mix retain separate arrangements; Clips and MIDI Editor belong to Edit. Transport, plug-in, and specialist utility windows retain their existing hosts. The document/session view is protected. Layouts, active tabs, panel visibility, and floating rectangles are saved with the app's existing UI preferences. These operations do not edit document contents.

Panel IDs: `tracks`, `clips`, `midi`, `memory`, `history`. The protected center is `canvas`.

`ui.dock` accepts `operation` and `panel`. Operations are `open`, `close`, `activate`, `float` (with finite positive `rect: [x, y, width, height]`), or `move` (with `target` and `zone: center|left|right|top|bottom`). Center groups with the target; side zones split beside it. `ui.dock.reset` restores the default arrangement. Invalid moves return an error without changing the layout.

Invoke these UI commands through `engine.execute`, for example `{"command":"ui.dock","params":{"operation":"float","panel":"tracks","rect":[100,100,320,440]}}`.

Advanced `ui.dock` operations use the same validated transaction as pointer gestures:
`moveFloating` updates `rect`, `resizeSplit` accepts a boolean child `path` and
`size` (`{"Ratio":0.5}`, `{"FixedFirst":280}`, or `{"FixedSecond":280}`),
`setStackOpen` takes `open`, and `resizeStack` takes a point `height` or null.
A center `move` can include `before` (panel ID or null to append) for tab order.
The equivalent `{"action": ...}` form accepts a serialized shared docking action;
this is also the form emitted by the renderer. Protected-canvas and app-specific panel
rules apply equally to both forms.
