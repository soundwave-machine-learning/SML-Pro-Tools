# MCP server

SoundCraft speaks the [Model Context Protocol](https://modelcontextprotocol.io) over stdio, so
Claude and other agents can make music with it.

```sh
soundcraft-cli mcp                    # headless engine (no window, no audio device)
soundcraft-cli mcp --demo             # headless, starting from the demo session
soundcraft --control 7801 &           # the desktop app…
soundcraft-cli mcp --connect 7801     # …driven over MCP: same tools plus screenshots and clicks
```

With `--connect`, the bridge authenticates to the app's control channel (protocol 2) with
`SOUNDCRAFT_CONTROL_TOKEN` or the token file in the settings folder (see
[control-protocol.md](control-protocol.md)); without a token it reports where the file is. It never
re-sends a request after a timeout or a lost connection.

Claude Code:

```sh
claude mcp add soundcraft -- soundcraft-cli mcp --demo
```

## Tools

| Tool | Purpose |
|---|---|
| `list_commands` | Discover every command (id, label, menu path, shortcut, params doc, enabled). |
| `execute` | Run one command with JSON params. |
| `batch` | Run several commands in order (`keep_going` optional). |
| `inspect_session` | Verify work: tracks, clips, mixer, plugins, sends, automation, markers, selection; each `sources[]` entry includes `loaded` (decoded audio in the pool). |
| `new_session`, `open_session`, `save_session` | Session files (`.scraft`, audio in `Audio Files/`). `open_session` returns `missing`: the media files that could not be loaded (empty when all are found). |
| `import_audio` | WAV, AIFF, FLAC, MP3, OGG, AAC/M4A, ALAC, CAF onto a new or existing track. |
| `bounce_mix` | Render to WAV/AIFF/FLAC (and MP3/M4A on Windows); returns peak dBFS and integrated LUFS, and `master_muted` with a warning when the Master is muted. |
| `parity` | Feature-parity report. |
| `screenshot`, `ui_inspect`, `ui_set`, `menu_invoke`, `click`, `drag`, `key` | App only (with `--connect`). |

Resources: `soundcraft://session` (full session JSON), `soundcraft://commands`.

## Conventions

- Programmatic calls never open dialogs (except theme/shortcut import/export without a path); empty params act on the current selection. There is no confirmation: `new_session`/`open_session` discard unsaved changes (they are refused while a take is being recorded).
- Positions: samples, `{"seconds": x}`, or `"bars_beats:5|1|000"`-style strings.
- Tracks by name or id; clips by id.
- Editing and mixing commands are undoable through `edit.undo`; session, file, export, transport, setup and options commands are not (see the Undo column of [COMMAND_REFERENCE.md](COMMAND_REFERENCE.md) or `help.search`). Each call — including each call inside `batch` — is its own undo step.
- `serverInfo.version` is the CLI's version; with `--connect` it does not prove the app's version (ask the app: `help.limits` returns `build`).

The acceptance test `crates/automation/tests/agent_tasks.rs` drives a realistic session end to end
through MCP only (mixing, plugins, routing, editing with undo, markers, tempo, automation, bounce,
save and reopen).
