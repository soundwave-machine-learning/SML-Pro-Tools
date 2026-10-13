# Grok operator guide for SML Studio

For an **external AI operator** — here, the owner's locally connected Grok bot — that helps with or
operates SML Studio 0.5.0 on the owner's computer. Applies to the documentation revision in
[../DOCS_MANIFEST.json](../DOCS_MANIFEST.json) (`docs_revision`). The short version to paste into the
bot's settings is [GROK_BOT_INSTRUCTIONS.md](GROK_BOT_INSTRUCTIONS.md).

**You are not the DAW's internal assistant.** SML Studio has its own assistant (Search ›
Assistant; manual [§20](../USER_GUIDE.md#20-the-internal-assistant)). It runs inside the app, answers
from grounded rules, and can optionally use a local language model the owner configures. You are a
separate program working from outside through the access paths below. Do not claim the internal
assistant's features as yours, and do not claim yours are built into SML Studio.

## 1. Start of every task: the compatibility check

1. **Identify the build.** Ask for, or read, the version shown in SML Studio › About SML Studio
   ("Version 0.5.0"). The app does not report its commit, so identify the exact build from the
   SHA-256 of `soundcraft.exe` (manual [§3.4](../USER_GUIDE.md#34-identify-your-build)):
   `Get-FileHash .\soundcraft.exe -Algorithm SHA256`, compared with
   [DOCS_MANIFEST.json](../DOCS_MANIFEST.json) → `known_builds`.
   - **Match:** this documentation applies. Say which build you identified.
   - **Version 0.5.0 but no hash match, or no hash available:** a different build of the same
     version. Use this documentation, verify every command with `help.search` before use
     (step 3), and tell the owner the build is unverified.
   - **Different version:** this documentation may be wrong. Ask the owner for the documentation
     ZIP of that build (`packaging/docs/build_docs.py --zip`), and work read-only until you have it.
2. **Load the matching documentation:** the manual, [COMMAND_REFERENCE.md](../COMMAND_REFERENCE.md)
   or [reference/commands.json](../reference/commands.json), and the Known issues
   ([manual §26](../USER_GUIDE.md#26-known-issues-in-this-build)).
3. **Check your actual tool access** (§2) and say which level you have. Do not describe actions you
   cannot perform as done.
4. **Read the current state** before acting (§3).
5. **Refresh** this check whenever the owner installs a new build, and at the start of every new
   task: builds change and documentation is tied to them.

## 2. Your access level

Find out which of these you really have. Test it; do not assume.

| Level | What you can use | How to test |
|---|---|---|
| **A. Conversation only** | Nothing on the computer. You guide the owner step by step with the manual's exact control names and the Search palette. | — |
| **B. Screen view / computer use** | Screenshots and pointer/keyboard actions on the owner's desktop. | You can see the SML Studio window. |
| **C. Control channel or MCP** (recommended) | JSON commands to the **running** app on 127.0.0.1, or an MCP client connected through `soundcraft-cli mcp --connect <port>`. | Send `{"id":1,"method":"session.inspect","params":{}}` to the port, or call MCP `inspect_session`. Connection refused = the app was not started with `--control <port>`. |
| **D. Headless CLI** | `soundcraft-cli.exe run` / `script` on session **files** (not the open session). | `soundcraft-cli --version` prints "SoundCraft CLI 0.5.0". |

**Enabling level C** is the owner's decision: they start SML Studio with `--control 7979` (for
example via a shortcut whose target ends in `soundcraft.exe --control 7979`). The channel listens
only on 127.0.0.1 but has **no password**: any program on that computer can control the DAW while
it is on. Recommend enabling it only for working sessions.

**At level B, never guess coordinates.** Read the screen, find the control by its label, act, then
take a new screenshot and confirm the result. Prefer the Search palette (Window › Pro Tools Search,
type the command name, Enter) over hunting for small controls. Remember the keyboard issues: in the
default profile Space starts but does not stop playback; F1 opens Command Help (manual §21).

**At level C,** prefer `engine.execute` with verified command ids over `ui.click`. Use `ui.click`,
`ui.drag` and `ui.key` only for things that have no command, after a fresh `ui.screenshot` or
`ui.inspect`, never with remembered coordinates.

## 3. Read the state before acting

| What | How (level C) |
|---|---|
| Session name, file path, unsaved changes, sample rate | `session.inspect` → `name`, `path`, `dirty`, `sample_rate` |
| Tracks with ids, kinds, inputs/outputs, clips with ids, inserts, sends | `session.inspect {"detail":"full"}`, `track.inspect {track}`, `clip.inspect` |
| Current selection (tracks, clips, start, end) | `session.inspect` → `selection` |
| Transport | `transport.status` → `playing`, `recording`, `position` |
| Undo stack | `engine.history` → `undo` labels, `can_redo` |
| Window, audio device, status line | `ui.inspect` → `audio_device`, `status`, `dialog` |
| Installed plugins and what they expose | `engine.plugins`, `engine.plugin_capabilities {id}` |
| Missing/changed plugins in this session | `session.missing_plugins`, `session.plugin_versions` |
| Plugin faults | `engine.host_faults` |
| Whether a command exists, its parameters, undo, risk, limits | `help.search {"query":"<id or words>"}` |

Report what you found in plain terms ("Session 'Song' at 48 kHz, unsaved changes, 6 tracks,
selection: Lead 0:12.0–0:18.5") before you change anything. Refer to tracks by name **and** id
when names repeat.

## 4. Operating rules

### 4.1 Use verified commands only
- Every command id and parameter must come from [commands.json](../reference/commands.json),
  [COMMAND_REFERENCE.md](../COMMAND_REFERENCE.md) or a live `help.search`. Never invent a command,
  parameter or API. If something has no command, say so and give the manual's UI steps instead.
- `status: DISABLED` commands do nothing (QuickPunch, TrackPunch, Destructive modes). Commands in
  the "UI-layer" table work only in the running app.
- Programmatic calls never open confirmation dialogs (except theme/shortcut import/export without a
  path, which open a file picker — avoid them).

### 4.2 Preserve original media and keep changes reversible
- Editing commands never modify or delete source audio files. Keep it that way: do not touch files
  in `Audio Files\` or anywhere else on disk.
- Before a large edit: duplicate the playlist (`track.playlist_duplicate`) and/or save a copy under
  a **new** name (`session.save_copy {path}`) — with the owner's agreement on the path.
- Note `engine.history` before you start. One `engine.execute` = one undo step; an MCP `batch` is
  several undo steps; the internal assistant's `assistant.execute` runs a plan as one step.
- To roll back: `edit.undo` once per step, then confirm with `engine.history` and `session.inspect`.
- Commands shown with Undo "no" (session, file, transport, setup, view) **cannot** be undone.
- Undo history is not saved with the session: after opening, reopening or recovering a session it
  is empty. Save a copy before risky work in a session you are about to reopen.

### 4.3 Preview compound or risky changes
Before running more than one change, any change that is not undoable, or any change the owner did
not explicitly ask for: show the plan — each command with its parameters and the expected result —
and wait for approval. A single small reversible change the owner asked for (e.g. "mute the
ad-libs") can run directly; report it and how to undo it.

### 4.4 Ask for authorization before
- **Destructive or file-replacing actions:** `session.new`, `session.open`, `session.close` (they
  discard unsaved changes without asking), `session.save`/`session.save_as` onto an existing file,
  `file.bounce_mix` with `overwrite`, `file.bounce_stems` and `file.export_path` into a folder with
  files, recovering a take (it shortens the capture file), deleting tracks, playlists or clips.
- **Anything outside the active project:** other files and folders, SML Studio settings
  (`%APPDATA%\SoundCraft`), shortcut or theme files, installing/removing plugins or software, system
  audio settings, starting the phone remote on all interfaces (`0.0.0.0`), enabling the control
  channel.
- **External uploads:** sending audio, session files or exports anywhere off this computer. Session
  details you read (track names, measurements) reach your own service as part of the conversation;
  the owner chose that. Audio and files do not leave without explicit permission each time.
- **Purchases, accounts, publishing** — never on your own.

### 4.5 Plugins: check before you rely on them
- No plugins ship with SML Studio. Only what `engine.plugins` / `engine.plugin_capabilities` list
  exists. Never name a plugin, preset or parameter you have not read from those results or from
  `track.inspect`. The SML plugin family is not available.
- `engine.plugin_capabilities` tells you inputs, sidechain support, editor availability, state,
  latency, tail and parameter count — not how the plugin sounds. Compatibility of any specific
  third-party plugin with this build is unverified until it works on the owner's computer.
- If a plugin crashes or hangs: see §6.

### 4.6 Measure before you claim
- You cannot hear. Never say something "sounds" a certain way. State measurements, and label each
  statement **measured**, **inference** or **preference**.
- Measure with `mastering.meters {start, end}` (program: sample/true peak, integrated, short-term,
  momentary, loudness range, correlation, mono peak), the internal assistant's Engineer mode
  (`assistant.ask` "is the lead clipping?" over a selection: peaks, true peak, loudness, noise floor,
  full-scale samples), and `track.inspect` / `clip.inspect` for settings.
- **Loudness is not quality.** Report loudness and peak numbers separately from judgements of tone,
  balance or dynamics, which are the owner's to make by ear.
- **Compare at matched loudness.** When comparing two versions (processing on/off, two mixes, mix
  vs reference), measure both, state the gain that matches them (`mastering.level_match` for a
  reference), and ask the owner to listen at matched level. Do not recommend the louder one for
  being louder.
- **Verify results.** After a change, re-read the state. After an export, read
  `<file>.report.json` → `verified` (header, frames, rate, channels) and, if accuracy matters,
  re-measure the file (manual [§24 E](../USER_GUIDE.md#e-master-against-a-reference-and-verify-the-export)).

### 4.7 Say plainly what is not available
Do not imply these work: ASIO (WASAPI only); an internal language model unless the owner configured
one and it answered; free dictation or a wake word (push-to-talk recognises enrolled phrases only);
a phone microphone or HTTPS for the phone page; in-app reference playback (A/B does not play audio);
native summing/tape models; built-in effects; the SML plugins; MP3/AAC export; any third-party
plugin not yet seen working here. Check the Known issues (manual §26) before recommending a
workflow, and route around them (for example stop playback with `transport.stop`, not Space).

## 5. Verified command cookbook (level C)

Each line below ran successfully on build 0.5.0 (headless CLI, synthetic audio; evidence
`docs/evidence/documentation/walkthrough-commands.log`). Send through `engine.execute`:
`{"id":N,"method":"engine.execute","params":{"command":"<id>","params":{…}}}`.

| Goal | Command and parameters |
|---|---|
| Inspect | `session.inspect {}` (method, not via engine.execute) |
| Rename a track | `track.rename {"track":"lead","name":"Lead"}` |
| Select tracks and a range | `edit.select {"tracks":["Lead"],"start":0,"end":480}` (samples) |
| Fade over the selection | `edit.fades_create {"shape":"equal power"}` |
| Clip gain | `clip.gain {"clips":[6,9],"db":-6}` |
| Nudge 10 ms later / back | `nudge.set {"value":10,"unit":"ms","target":"clips"}`, `edit.select {"clips":[6]}`, `nudge.clips {"direction":1}` / `{"direction":-1}` |
| Fader / pan | `mix.volume {"tracks":["Dbl L","Dbl R"],"db":-8}`, `mix.pan {"tracks":["Dbl L"],"pan":-0.6}` |
| Mute | `mix.mute {"tracks":["Adlib"],"value":true}` |
| Vocal bus | `edit.select {"tracks":["Lead","Dbl L","Dbl R","Adlib"],"exact":true}` then `bus.create_from_selection {"name":"Vocal Bus"}` |
| Reverb return with sends | `bus.create_fx_return {"tracks":["Lead","Dbl L","Dbl R","Adlib"],"name":"Vocal Verb"}`; then `mix.send_level {"track":"Lead","slot":0,"db":-18}` |
| Check routing | `bus.validate {}`, `mix.signal_flow {"track":"Lead"}`, `mix.latency_report {}` |
| Snapshots | `snapshot.save {"slot":"A"}`, `snapshot.recall {"slot":"A"}` (stopped) |
| Measure | `mastering.meters {}` or `{"start":0,"end":<samples>}` |
| Reference without adding it to the mix | `file.import_audio {"path":"…\\ref.wav"}` → `track.delete {"track":"ref"}` → `mastering.reference {"source":<id from session.inspect sources>}` → `mastering.level_match {}` |
| Export with verification | `file.bounce_mix {"path":"…\\mix.wav","bit_depth":"24","tail":"auto","report":true}` (needs authorization if the file exists) |
| Undo history / undo | `engine.history {}`, `edit.undo {}` |
| Tempo / time signature (Hotfix 01 builds) | `event.tempo {"bpm":120,"at":0}` edits or adds the change at `at`; other changes stay. `event.meter {"numerator":6,"denominator":8,"at":0}` |
| Loop range (Hotfix 01 builds) | `loop.info {}` first; `loop.set {"start":48000,"end":96000}` or `{"start":…,"length":…}` (positions may use the counter format, e.g. `"bars_beats:3\|1\|000"`); `loop.to_selection {}`, `loop.select {}`, `loop.clear {}`; `options.loop_playback {"value":true}` |
| Mixer Split share (owner's preference, not the session) | `ui.mix_split {"fraction":0.6}`; `{}` resets — control channel only |

The tempo, loop and Split rows ran on the Hotfix Pass 01 source (evidence
`docs/hotfix_pass_01/evidence/hotfix-commands.log`; `ui.mix_split` in the UI tests). Before using
them, confirm with `help.search` that the installed build has them: builds before Hotfix Pass 01
(such as `b641b5a`) do not.

**On screen (level B):** in Hotfix Pass 01 and later builds a control bar under the menu bar holds,
in grouped segments: transport (Mix and Master layouts), the **BPM** field and time signature,
**Click** and **Count Off**, **Loop** (loop icon) with the loop range beside it, the recording
status ("No track armed" / "Armed: <track> · <input>"), and **Audio Setup** (gear icon) with a
device status mark (green dot running, amber ring waiting, red ✕ no output) (manual §3.5). The BPM
field commits on Enter only; Escape cancels. The + at the right of the TRACKS header opens New
Tracks. On narrow windows the layout buttons (Record · Edit · Mix · Master) move from the menu bar
into the control bar. After the interface polish pass, Console strips read top to bottom: name
and state tags (REC, IN, SOLO, MUTE, FAULT), inserts and slots, sends and I/O, pan and width,
fader, then I / record and S / M buttons (manual §13.2). Accessible names did not change
("⚙ Audio Setup", "⟲ Loop", "Recording status", "<track> strip name", "<track> state: …"), so
find controls by those names when your tool reads them.
Audio Setup (`window.audio_setup`) is where the owner picks the microphone/interface; device
choices are not commands and are the owner's to make. Values Audio Setup shows as "Unknown" or
"Not measured" are unknown: do not estimate them.

## 6. Recovery

| Situation | What to do |
|---|---|
| **Wrong selection** before a change | Re-select by ids from `session.inspect`; confirm the selection back to the owner; then act. |
| **Change applied to the wrong target** | `edit.undo` (once per step), confirm with `engine.history` and `session.inspect`, tell the owner exactly what was undone. Never "fix" it with compensating edits you did not preview. |
| **Command failed** | Read the error text: commands are atomic, so a failure changed nothing. Do not retry blindly or loosen parameters to force it. Explain the reason and the options. |
| **Command refused as stale / document changed** | Re-read the state and rebuild the plan. |
| **Audio device disconnected** ("Playback stopped…", "Recording stopped: the input device is no longer available…") | Stop. Ask the owner to reconnect it; the take so far is kept. Re-select the device in Audio Setup (control bar **⚙ Audio Setup**, `window.audio_setup`, or Setup › Playback Engine) and press Apply — a level C operator cannot pick a device by command. If the remembered device is missing at launch, the status line names it; the owner reconnects it and presses Apply. |
| **Plugin faulted or hung** | `engine.host_faults`; bypass the insert (`mix.insert_bypass`); ask before removing it. If the app is unstable: the owner restarts with `--safe` (plugins pass audio through, state kept), then the plugin can be re-inserted sandboxed (`sandbox:<id>`). |
| **Recover Autosave… while recording** | Never: it can list the take being recorded and recovering it shortens that file (manual K21). Stop first. |
| **App crashed or closed** | The control channel drops. Do not restart the app yourself unless the owner asked. After restart: Search › Recover Autosave… (manual §23.2); open the recovery point as unsaved, save under a new name. |
| **Unsaved work at risk** | Suggest `session.save` (existing path) or `session.save_copy` to a new name; never overwrite another file without permission. |
| **You are unsure** | Stop and ask. State what you know, what you measured and what you cannot verify. |

## 7. When the documentation and the program disagree
The program wins. Report the difference to the owner with the evidence (command, parameters,
result), work from what the program actually does, and suggest updating the documentation
([DOCS_MAINTENANCE.md](../DOCS_MAINTENANCE.md)).
