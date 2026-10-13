# SML Studio user manual

Product: SML Studio (the SoundCraft engine), version 0.5.0. This is the one authoritative manual.
Documented source: commit `018793f7c2f3d601800b7ddbcd38b45c2c30a255` (branch `fable/recording-core`), whose program code is identical to the packaged build `b641b5ad938d47a9387f611cf717f9933c7cdb19`.
Verification status, applicable builds and history: [DOCS_MANIFEST.json](DOCS_MANIFEST.json), [DOCS_CHANGELOG.md](DOCS_CHANGELOG.md).
Every command named here exists in this build; parameters and chords for all of them: [COMMAND_REFERENCE.md](COMMAND_REFERENCE.md).

This manual describes what the software does **today**, checked against its source code and
automated tests. It does not describe planned features as working. Where something has not yet
been seen working on real hardware, the text says so. Your installed copy is compiled with this
file: the in-app assistant quotes its section numbers ("guide §12").

## 1. How to read this manual

**Evidence labels.** Each capability carries one of these labels:

| Label | Meaning |
|---|---|
| **Implemented** | Present in the source and covered by automated tests in a Linux build container (environment "P"). It has not necessarily been seen on your computer. |
| **Observed** | Seen working on a named machine with a named build. Recorded in [DOCS_MANIFEST.json](DOCS_MANIFEST.json) → `verification.observed`. At this revision **no observation on Windows exists**. |
| **Pending target check** | Implemented, but depends on hardware, a driver, a plugin, a phone or a listener that has not been checked. |
| **Not available** | Not in this build. The control may be visible but disabled, or absent. |
| **Planned** | Intended for a later version. Never usable now. |

**Builds.** The application reports only its version ("SoundCraft 0.5.0"), not the commit it was
built from. §3.4 shows how to identify the exact build you installed. If your build is not
`b641b5a`/`018793f`, check [DOCS_MANIFEST.json](DOCS_MANIFEST.json) → `known_builds` and the
changelog for differences. The previous edition of this manual, exactly as compiled into
`b641b5a`, is kept in [manual_archive/USER_GUIDE-0.5.0-b641b5a.md](manual_archive/USER_GUIDE-0.5.0-b641b5a.md);
§28 maps its section numbers to this edition.

**Names.** Menu paths are written `Menu › Item`; on-screen labels are in quotes or bold as they
appear; command ids are written like `mix.volume`. Every command can also be run from the
**Search** palette (Window › Pro Tools Search; type a label or id, Enter), which runs it as if
chosen from its menu: commands with a dialog open it, commands without one run with their
defaults. Several windows have **no menu-bar entry** and open only from Search: Recording,
Recover Autosave…, SML Remote, Assistant and Command Help (Command Help also with F1). This manual
writes them as **Search › Recording** (open Search, type "Recording", Enter). Key chords use Windows names (`Ctrl` where macOS uses `Cmd`). The default shortcut
profile is "Pro Tools style" (§21).

**Procedure layout.** Procedures give: purpose, before you start, steps, result, undo and
recovery, and limits.

**Engineering values** in walkthroughs are labelled *starting point*: reasonable first settings
for typical material, not rules. Trust your ears and your meters over any number here.

## 2. Capability status

| Area | Status in 0.5.0 | Notes |
|---|---|---|
| Recording to disk (dry, 32-bit float WAV), punch with selection, pre/post-roll, loop record, Count Off | Implemented; pending target check on your interface | §6 |
| Device selection, buffer request, effective callback display (WASAPI on Windows) | Implemented; pending target check | ASIO: not in the standard build (§4.6) |
| QuickPunch, TrackPunch, Destructive Record/Punch | Not available | Menu entries are disabled |
| Playlists, promote-to-main comping, take rating | Implemented | §7 |
| Eight audio tools, edit modes, clip gain, fades, strip silence, Bend | Implemented | §8–§10 |
| Nudge panel and keypad Nudge | Implemented; keypad and Windows-key limits | §12 |
| Console / Visual / Split mixer, width, snapshots A/B/C | Implemented | §13–§14 |
| Volume automation writing (Write/Touch/Latch), automation drawing | Implemented | Only volume is written live (§14) |
| Buses, FX returns, sends, Spill | Implemented | §15 |
| Sidechain into a plugin from a bus | Implemented (CLAP first aux port, VST3 first aux bus); no UI | Command only (§15.5); pending target check with real plugins |
| Summing / Tape / Master Tape slots | Implemented as plugin slots; **native analog summing/tape models: Planned** | §16 |
| CLAP and VST3 hosting, isolated scanning, quarantine | Implemented; pending target check with your plugins | §17 |
| VST3 editor windows on Windows | Implemented (Win32 window); **not yet observed on Windows** | §17.3 |
| Sandboxed (out-of-process) plugin instances | Implemented, command only | §17.7 |
| Built-in effects (EQ, compressor, reverb…) | **Not available** | No plugin ships with the app |
| SML plugins (Channel EQ, Gate/Expander, Compressor, Optical Leveler, Tune, Vocal Editor, Align) | **Not available**: separate projects | Listed as AWAITING_EXTERNAL_PLUGIN |
| Mastering meters (true peak, LUFS, LRA), level-match measurement | Implemented; checked against analytic and independent (FFmpeg) results, not certified | §18 |
| Reference playback (A/B listening) inside the app | **Not available** (button stores a setting; no audio plays) | Known issue K1 (§26) |
| Export: WAV/AIFF/FLAC, 16/24/32f, dither, rate conversion, tails, report | Implemented | No MP3/AAC export |
| Phone remote (plain HTTP on your network) | Implemented; pending target check with a real phone | No HTTPS, no phone microphone |
| Internal assistant: grounded answers, previewed plans | Implemented | §20 |
| Internal assistant: local language model | Implemented client; **no model ships; never observed with a real model** | Loopback servers only |
| Push-to-talk (enrolled phrases), spoken replies | Implemented; tested on synthetic speech only | Not dictation; quiet room needed |
| Wake word, free dictation, cloud assistant | Not available | |
| Shortcut profiles, themes, interface scale, layouts | Implemented | Known keyboard issues K5–K7 |
| Autosave generations, crash recovery, Safe Open | Implemented; power-loss behaviour pending target check | §23 |
| External control: CLI, local control channel, MCP | Implemented | §25 and [grok/GROK_OPERATOR_GUIDE.md](grok/GROK_OPERATOR_GUIDE.md) |

## 3. First launch, settings, versions and Safe Open

### 3.1 Start the application

**Purpose:** open SML Studio with or without a session.

**Steps:**
1. Start `soundcraft.exe` (portable zip: from the unzipped folder; installer builds: from the shortcut the installer created).
2. Optional arguments (from a shortcut or a terminal):
   `soundcraft [--demo] [--control <port>] [--no-audio] [--safe] [--reset-prefs] [session.scraft | audio files…]`.
   - `--demo` (or `--sample`): open the synthesised demo session.
   - A `.scraft` file opens it; `.mid`/`.midi`/`.smf` files are imported as MIDI; other files are
     imported as audio at the edit insertion point. Dropping files on the window does the same.
   - `--no-audio`: run without opening any audio device.
   - `--control <port>`: start the local control channel (§25.2). Off unless you pass it.
   - `--safe`, `--reset-prefs`: §3.3.

**Result:** the window title reads "<session name> — SML Studio". Without arguments you get an
empty "Untitled" session at 48 kHz.

**Limits:** on Windows the program has no console, so anything it prints (for example
`--version`) is not visible; use Help information in the app (§3.4).

### 3.2 Where settings and logs live
| What | Windows location |
|---|---|
| Settings folder | `%APPDATA%\SoundCraft` (override: environment variable `SOUNDCRAFT_CONFIG_DIR`) |
| Interface preferences (theme, shortcuts, layouts, monitor level, phone bind address) | `ui.json` |
| Assistant settings, enrolled voice phrases (features, not audio), model address | `assistant.json` |
| Recovery generations | `Autosave\<session>\` |
| Plugin presets | `Presets\` |
| Plugin scan cache | `plugin-scan-cache.json` |
| Log | `logs\soundcraft.log` (16 MiB, two older copies kept) |

macOS uses `~/Library/Application Support/SoundCraft`; Linux `~/.config/soundcraft`.
Audio device, buffer and input choices are **not** remembered between launches (§4.1).

### 3.3 Safe Open and resetting preferences

**Purpose:** start when a plugin or a preference stops the app from working.

**Steps:** start with `--safe` (hosted plugins are neither scanned nor loaded for that run) or
`--reset-prefs` (renames `ui.json` to `ui.json.backup-<unix time>`; rename it back to restore).

**Result:** in Safe Open every plugin slot passes audio through and keeps its stored state and
settings, so saving the session loses nothing. Restart without `--safe` to load plugins again.

**Limits:** the "SAFE OPEN" and "previous run did not close properly" status messages are
currently overwritten when saved preferences load (Known issue K9); Safe Open still takes effect.

### 3.4 Identify your build

**Purpose:** know exactly which build you run, so you read the matching documentation.

**Steps:**
1. In the app: menu **SML Studio › About SML Studio** shows "Version 0.5.0 — …". The Command Help
   window header shows "build 0.5.0".
2. The version alone does not identify the commit. Compute the hash of the program file. In
   PowerShell, in the folder that contains it:
   `Get-FileHash .\soundcraft.exe -Algorithm SHA256`
3. Compare it with [DOCS_MANIFEST.json](DOCS_MANIFEST.json) → `known_builds`. The portable zip
   delivered on 2026-10-12 (commit `b641b5a`) has
   `soundcraft.exe` SHA-256 `bb405c79a3a3c2d3d4a9bf9f0bb80e845901f80f794d891b30653f1d34878967` and
   `soundcraft-cli.exe` `7e8ca105f4a64e80bd531bb54c3951e0ed812ac8de2c247694352be99a67300b`.
   The zip also contains `MANIFEST.json` naming its commit.

**Result:** a matching hash means this manual applies exactly. A different hash means a different
build: read its own `MANIFEST.json` and the changelog.

## 4. Audio devices, sample rate, buffer and monitoring

### 4.1 Choose the output device and buffer

**Purpose:** play through your interface at a known buffer size.

**Before you start:** stop the transport.

**Steps:**
1. **Setup › Playback Engine...** (also **Setup › Hardware...**). The window "Playback Engine" opens.
2. Click **Scan output devices**. Pick the device in the combo ("default device" or
   "<host>: <device>"; on Windows the host is WASAPI).
3. Pick the buffer: "default buffer" or 32, 64, 128, 256, 512, 1024 frames.
4. Click **Apply (stopped only)**.

**Result:** "Audio device opened with the requested settings". The read-only rows show **Device
rate**, **Session rate**, **Requested buffer**, **Reported buffer range**, **Effective callback**
("N frames (x.xx ms)" once audio runs) and **Stream clock**. "Clock lock / driver latency" always
reads "Unknown (not reported by this backend; not assumed zero)".

**If it fails:** "Device not changed: …" with the reason (device not found, rate not supported,
buffer outside the driver's range, no 32-bit float format). The previous device stays open;
another device is never substituted silently.

**Limits:** the choice is not saved; each launch starts on the system default output. **Reconnect
audio device** reopens the *default* output, not the one you chose (Known issue K10).
Pending target check: actual buffer sizes on your interface.

### 4.2 Choose the recording input

**Steps:** in the same window click **Scan input devices** and pick the interface in the input
combo ("default input" or "<host>: <device>"). It opens immediately (stopped only) at the session
rate, the chosen buffer and every input channel the device offers.

**Result:** "Input device opened with the requested settings"; the **Input device** row shows
"<name> (<n> ch @ <rate> Hz, …)".

**If it fails:** "Input not changed: …" (for example "`X` has no 32-bit float input at 48000
Hz"); the current input stays. If the app ever reported "No audio input: …", Record answers
"Cannot record: no audio input device" until you pick an input here.

**Limits:** not saved between launches. If the input runs at a different rate than the session,
takes are resampled to the session rate and the status line says so; for bit-exact dry takes,
make the session rate match the device (§5.1).

### 4.3 Delay compensation
The **Delay compensation** checkbox in Playback Engine (also **Options › Delay Compensation**,
`options.delay_compensation`) aligns all channels to the slowest one, including plugin latency.
Leave it on for mixing. §17.6 covers latency reporting.

### 4.4 Monitoring controls (speaker path)

**Where:** **Search › Recording** (the Recording window) and the Record layout (menu-bar button **Record**).
- "Monitor (speaker path only — never in a bounce)": **Volume** slider (−60 to 0 dB), **Dim −20**,
  **Mono** (`monitor.volume`, `monitor.dim`, `monitor.mono`). They act after the mix on the
  output only, never in an export, and are remembered per computer in `ui.json`.
- **Headphones**: shown "Headphones: unavailable" unless I/O Setup has an extra output whose name
  contains "cue", "phone" or "hp". Even then a separate headphone level or cue mix is **not
  available** in this build.

### 4.5 Input monitoring and monitoring modes
- **Input monitoring** (`mix.input_monitor`, Shift+I; the "I" button in a track header; "Input
  monitor" in the Recording window) plays the track's live input through its channel. An armed
  track also monitors its input while stopped and while recording.
- **Monitoring mode** (`mix.monitor_mode`; Recording window "Monitoring:" **Tracking** /
  **Mixing** / **Hardware**; change while stopped). **Hardware** turns software input monitoring
  off so you can use your interface's own direct monitoring (plugins are then not on what you
  hear). **Tracking and Mixing currently behave the same** (they are labels; Known issue K3).
- To cut monitoring latency from latent plugins while tracking: `mix.tracking_deactivate`
  (Search: "Tracking: Deactivate Latent Processors"; stopped only), and `mix.tracking_restore`
  afterwards. Temporary, never saved, never applied to exports.
- Recording is always the dry input, whatever you monitor.

### 4.6 What is not available
ASIO is not in the standard build: it exists only as a build option that needs Steinberg's SDK
on a Windows build machine (repository file `docs/remote_continuation/WINDOWS_AUDIO_BACKENDS.md`).
Aggregate-device clock correction and measured driver latency are not available. "Low Latency
Monitoring" (`options.low_latency_monitoring`) stores a setting that nothing uses yet.

## 5. Sessions and project setup

### 5.1 Create a session

**Steps:** **File › New...** (Ctrl+N). The "New Session" dialog has **Name**, **Sample Rate**
(44.1 kHz up to 384 kHz) and **Start from the demo session**; click **Create**.

**Result:** an empty session at that rate. Session bit depth is set in **Setup › Session**
(Ctrl+2: "Timecode Rate", "Bit Depth" 16/24/32f); it labels the session but recordings and saved
media are always 32-bit float.

**Undo:** New, Open and Close are not undoable. When run programmatically they replace the open
session without asking; save first.

### 5.2 Save
- **File › Save Session As...** (Ctrl+Shift+S) the first time: choose a folder and name. The session
  file is `<name>.scraft`; audio that exists only in memory is written as 32-bit float WAV into
  `Audio Files\` beside it.
- **File › Save Session** (Ctrl+S) afterwards. If the session was never saved, Save reports
  "no path yet: pass `path` (Save As)".
- Saving writes a temporary file, flushes it to disk and then replaces the old file; a failed save
  leaves the previous file untouched and the session marked unsaved.
- **File › Save Session Copy In...** also re-points the open session to the copy's media (it
  behaves like Save As for media paths).
- **File › Save As Template...** writes the session without clips. Opening user templates from
  File › New is **not available** (only the blank and demo starting points).

### 5.3 Tracks
- **Track › New...** (Ctrl+Shift+N): count, format (mono, stereo, surround) and kind (audio, aux,
  master, MIDI, instrument, VCA, folder). Undoable.
- **Track › Duplicate...**, **Track › Delete** (both undoable; media files are never deleted).
- Import audio: **File › Import › Audio...** (Ctrl+Shift+I) or drop files on the window. WAV, AIFF,
  FLAC, MP3, OGG, AAC/M4A, ALAC and CAF can be imported.

## 6. Recording

### 6.1 Prepare a track

**Steps:**
1. Select the input the track records: in the Mix window the track's input selector ("no input",
   "In 1" … "In 8", or a bus) — command `track.input`. A mono track on "In 3" records input 3; a
   stereo track on "In 3" records inputs 3–4.
2. Arm it: the red record button in the track header, **● Arm** in the Recording window, or
   `mix.record_arm` (Shift+R). Undoable.

**If it fails:** at Record you get "Cannot record: `Vox` records input 3–3 but the input device has
2 channel(s). Change the track input or the input device." — the app never records a different
input instead. A one-input device can feed a stereo track (same signal both sides).

**Limits:** the selector always lists In 1–In 8 whatever the device has; an unset input shows as
"In 1".

### 6.2 Record a take

**Before you start:** an input device (§4.2), an armed track, the session saved if you want the
take beside it (otherwise it goes to the system temp folder `sml-recording`).

**Steps:** press **● Record** (Recording window or the Transport window) or Ctrl+Space. Press Record or
Stop again to finish.

**Result:** "Recorded N clip(s)". The capture file `<track>_<NNN>.wav` (all device inputs,
32-bit float, never overwriting an earlier file) is written continuously; the clip refers to a
separate copy saved into `Audio Files\` when you save. Recording is always **dry**: before clip
gain, inserts, slots and fader. Undo removes the clip (the capture file stays).

**Limits:** with nothing armed: "Record-enable a track first (the red button in its header)."
A take replaces whatever it covers on the active playlist (use playlists to keep alternatives,
§7). Pending target check on your interface.

### 6.3 Punch in with a selection, pre-roll and post-roll

**Steps:**
1. Make an edit selection over the phrase to replace (Range tool).
2. Turn on **Options › Pre/Post-Roll** (Ctrl+K). Amounts default to 2 s at 48 kHz (96 000 samples
   each); change them with `options.pre_roll` / `options.post_roll` (`{length: samples}` or
   `{length: {seconds: 1.5}}`). The expanded Transport window shows them; there is no field to
   type them in.
3. Record.

**Result:** playback starts at selection start − pre-roll, the take is trimmed to the selection,
playback stops at end + post-roll. One undo step.

**Limits:** existing clips inside the pre-roll and post-roll zones on that track can be removed by
the punch (Known issue K8); duplicate the playlist first (§7.1) when the surrounding material
matters. Without a selection, pre-roll audio stays in the take.

### 6.4 Count Off

**Steps:** tick **Count Off** in the Recording window and set its bars (1–16; default 2), or use
`options.countoff` and `setup.click_countoff {countoff_bars}`. The toolbar shows "N bars".

**Result:** recording starts that many whole bars before the record point with the click sounding
(even if the click is off); the clip starts exactly at the record point.

**Limits:** not applied to loop recording or when punching in while already playing.
**Setup › Click/Countoff...** shows the current values but opens no dialog.

### 6.5 Loop recording

**Steps:** turn on **Options › Loop Record** (Alt+L), make a selection, press Record; stop when done.

**Result:** "Loop-recorded N take(s)". The first pass lands on the active playlist; each later pass
goes to a new playlist named "<track>.NN", all at the loop start. A last pass shorter than one
eighth of the loop is dropped.

**Undo:** several undo steps (one per pass and playlist).

### 6.6 When something goes wrong while recording
| Message | Meaning and what to do |
|---|---|
| "Recording stopped: plugin latency changed during the take (alignment would be wrong). The take so far was kept; press Record to continue." | A plugin changed its latency (e.g. switched to lookahead). Keep plugins stable while tracking. |
| "Recording stopped: the input device is no longer available…" | Reconnect the interface; press Record again (the chosen input is reopened, never replaced). |
| "Playback stopped: … Reopen the device in Setup › Playback Engine." | The output vanished. The take so far is kept. |
| "Recording stopped early (WriteFailed / Overflow / DeviceLost): N frames kept in <path> — …" (shown at Stop) | Disk full or too slow. Capture stopped accepting audio earlier; the transport kept running until Stop. Everything written before the problem is kept. |
| "Recorded … — input ran at X Hz, resampled to the session's Y Hz" | Rate mismatch (§4.2). |

After a crash, use **Search › Recover Autosave…** → "Unfinished takes" (§23.2).

## 7. Takes, playlists and comping

### 7.1 Playlists
The playlist arrow in a track header offers **New...**, **Duplicate...**, **Delete Unused...** and
the list of playlists (`track.playlist_new`, `track.playlist_duplicate`,
`track.playlist_delete_unused`, `track.playlist_select`; all undoable). Duplicate the main playlist
before risky recording or editing; the duplicate is your backup.

### 7.2 Comp from alternate takes

**Purpose:** build one performance from the best phrases of several takes.

**Before you start:** takes on alternate playlists (loop recording does this, §6.5).

**Steps:**
1. Set the track view to **playlists** so alternate takes show as lanes under the main one.
2. Select a range over a phrase, then click the take lane you want (hover text: "Click a take (or
   the selection) to promote it to the main playlist"). Command:
   `track.playlist_promote {track, playlist, start, end}`.
3. Repeat for each phrase; add crossfades at the joins (§9.2).

**Result:** the phrase is copied into the main playlist at its original time; alternates are kept
unchanged; source files are never modified. Each promotion is one undo step.

**If it fails:** "that playlist is already active"; "give `start` and `end`".

### 7.3 Rate takes
**Clip › Rating** (none, 1–5; `clip.rating`). Stars show with View › Clip › Rating. The phone
remote can also rate takes. The listed chord Ctrl+Alt+0–5 does not work (Known issue K7).

## 8. Audio editing tools and edit modes

### 8.1 Edit modes
Toolbar cells **SHUFFLE**, **SLIP**, **SPOT**, **GRID** (`edit.mode`; F2 Slip, F3 Spot, F4 Grid in
the Pro Tools-style profile; F1 does **not** select Shuffle — it opens Command Help, Known issue
K6). Clicking GRID again switches to **REL GRID**. Default: Slip.
- **Slip**: clips move freely. **Grid**: a dragged clip's start snaps to the grid (Rel Grid snaps
  the same way in this build). **Spot**: dragging cannot change a clip's time, only its track.
  **Shuffle**: cut, clear, paste and erase close or open gaps (ripple).

### 8.2 The eight audio tools
The toolbar's audio tools group has **Arrow, Range, Split, Eraser, Paint, Mute, Bend, Listen**
in that order (`edit.tool`). Keys 1–8 select them only in the Studio One-style profile. Every
tool runs a shared command, so one gesture is one undo step, and the same edit is available from
Search, the CLI and the control channel. Escape or switching away during a sweep cancels it
without changing anything.

| Tool | What it does | Command | Limits |
|---|---|---|---|
| **Arrow** | Select and move clips. Within 6 px of a clip edge it trims; the top corners (upper part, within 14 px of an edge) drag fades, which are created as S-curves; the gain handle rides clip gain. With **Link** on (and a lane at least 40 px tall) the upper half over a clip selects a range; over empty lane space the whole height selects a range. | `edit.move_clips`, trims, `edit.tool_link` | Dragging moves only the selected clips, not their clip-group partners (§11). Locked clips are skipped. |
| **Range** | Drag a time selection across tracks. | `edit.select` | |
| **Split** | A line and time readout follow the pointer (snapped in Grid); click splits the clip, or every selected clip if the clicked one is selected. | `edit.split_at` | "no clip contains the split time (zero-length pieces are not created)"; a locked clip rejects the whole split. |
| **Eraser** | Click or sweep to remove clip instances. Ripples only in Shuffle. | `edit.erase_clips` | Media files are never deleted. A locked clip rejects the sweep. |
| **Paint** | Draws the shape chosen in the **Paint:** combo (freehand, line, parabola, square, triangle, saw, sine, transform; periodic shapes take a "cyc" count) into the shown automation lane, or into the clip-gain envelope of the clip under the pointer when no lane is shown. Pointer height covers −36…+36 dB. **Transform** draws a box and opens "Time shift (ms)", "Time scale", "Value offset", "Value scale", Apply/Cancel. | `automation.draw`, `clip.gain_draw` | Never writes automation when aimed at clip gain or the reverse. |
| **Mute** | Click or sweep to mute/unmute clips (clip mute, not channel mute). A sweep takes its target state from the first clip. | `edit.mute_clips` | |
| **Bend** | Warp anchors inside an audio clip (§10). | `clip.warp_set` | |
| **Listen** | Press and hold to audition the track from the pointer; release, Escape or focus loss stops it and restores the cursor. | `audition.start`, `audition.stop` | Refused while playing or recording ("Listen is unavailable while recording (a take is never stopped to audition)"). Changes no mute/solo/arm flag, writes nothing, never saved. Muted tracks stay silent. |

**Alt:** combo — an alternate tool used while Ctrl is held with Arrow active (`edit.alt_tool`).
The older tool group (Zoomer, Trim, Selector, Grabber, Scrubber, Pencil; F5–F10) is still present.

### 8.3 Clipboard, duplicate, repeat, separate, heal, consolidate
| Action | Menu and chord (Pro Tools style) | Command |
|---|---|---|
| Undo / Redo | Edit › Undo Ctrl+Z / Edit › Redo Ctrl+Shift+Z | `edit.undo`, `edit.redo` |
| Cut / Copy / Paste | Edit, Ctrl+X / C / V | `edit.cut`, `edit.copy`, `edit.paste` |
| Clear | Edit › Clear, Ctrl+B | `edit.clear` |
| Duplicate (pastes at the selection end) | Edit › Duplicate, Ctrl+D | `edit.duplicate` |
| Repeat (dialog "Repeat", 1–999 times) | Edit › Repeat..., Alt+R | `edit.repeat` |
| Separate at selection | Edit › Separate › At Selection, Ctrl+E | `edit.separate` |
| Heal separation | Edit › Heal Separation, Ctrl+H | `edit.heal` |
| Consolidate | Edit › Consolidate, Alt+Shift+3 | `edit.consolidate` |

Cut, copy and paste carry automation when Automation Follows Edit is on. Every undoable command
is one undo record, recorded only if something changed; a failed command restores the session.
**Window › Undo History** lists the steps. Undo history is not saved: opening, reopening or
recovering a session starts with an empty history.

## 9. Clip gain, fades and strip silence

### 9.1 Clip gain

**Purpose:** set a clip's level before the channel (independent of the fader and of automation).

**Ways:**
- The gain handle on a clip (shown when Clip Gain Info is on or the gain is not 0 dB, and the clip
  is wide enough): drag it to open a fader (−144…+36 dB; hold Ctrl for fine control). One drag is
  one undo step.
- `clip.gain {clips?, db | delta_db}` (clamped −144…+36 dB).
- `clip.gain_nudge_up` / `clip.gain_nudge_down` (Ctrl+Shift+Up / Down): ±1 dB per press.
- Paint tool on the clip (clip-gain envelope, §8.2) or `clip.gain_draw`.
- **Clip › Clip Gain › Render** (`clip.gain_render`) bakes it into new audio; **Clip › Clip Gain ›
  Bypass** (`clip.gain_bypass`); **Edit › Clear Special › Clip Gain** (`edit.clear_clip_gain`)
  resets to 0 dB and clears the envelope.

**Undo:** all undoable. Source files are never modified (Render creates new audio).

### 9.2 Fades and crossfades

**Steps:** select a range covering a clip start, a clip end or the boundary between two butted
clips, then **Edit › Fades › Create...** (Ctrl+F). The "Fades" dialog offers Linear, **Equal
Power** (default), S-Curve, Exponential and Logarithmic; OK applies.

**Result:** a fade, or a crossfade using the clips' handles (both sides use the same shape).

**If it fails:** "the selection must cover a clip start, end or the boundary between two clips".
Crossfades need clips that touch.

**Other commands:** `edit.fades_delete` (Edit › Fades › Delete); `edit.fade_to_start` and
`edit.fade_to_end` always use Equal Power.

### 9.3 Strip silence

**Steps:** select clips, **Edit › Strip Silence** (Ctrl+U). Dialog "Strip Silence": "Threshold dB"
(−96…0, default −48), "Min strip duration ms" (default 50), "Clip start pad ms" (default 5), "Clip
end pad ms" (default 20); **Strip**.

**Result:** regions whose peak (across channels) exceeds the threshold are kept as clips, padded;
silent gaps shorter than the minimum duration do not split. One undo step; media untouched.
*Starting point for a vocal:* threshold about 12–20 dB above the noise floor (measure it with the
assistant's Engineer mode, §20.4), pads of 10–30 ms so breaths and consonants are not clipped.

## 10. Bend (warp anchors)

**Purpose:** move a moment inside an audio clip in time, preserving pitch.

**Steps:**
1. Choose the **Bend** tool. Click inside an audio clip to add an anchor.
2. Drag the anchor to move that moment; Alt-click removes an anchor.

**Result:** the clip keeps playing its previous version until the new render is ready ("rendering…"
on the clip); renders run off the audio and interface threads. Command `clip.warp_set`; the
render is applied by `clip.warp_poll` (automatic in the app).

**Undo:** each anchor change is one undo step. **Clip › Clear Warp Anchors** (`clip.warp_clear`)
removes the map; **Clip › Remove Warp** (`clip.remove_warp`) restores the original material.
The original file is never modified.

**Limits:**
- Anchors at least 64 samples apart; each segment stretched 0.25–4×; the last anchor at least 64
  samples before the clip end.
- Segments are joined with a 480-sample raised-cosine crossfade at each anchor (10 ms at 48 kHz,
  about 10.9 ms at 44.1 kHz, 5 ms at 96 kHz; shorter for short segments). Where the two sides
  differ in phase the level can dip briefly; listen to every join.
- Stereo channels share one map. Locked clips and non-audio clips are refused.
- At most 4 renders wait at once: "render queue full (4 clips waiting); try again when a render finishes".
- Opening another session drops renders of the previous one (they never land in the new session).

## 11. Clip groups, edit groups and stereo editing

- **Clip groups:** **Clip › Group** (Ctrl+Alt+G; needs two or more clips), **Clip › Ungroup**
  (Ctrl+Alt+U), Regroup (Ctrl+Alt+R). In this build **Nudge moves every member of a group**, but
  dragging, Duplicate and other edits act only on the clips you selected.
- **Edit and mix groups:** **Track › Group...** (Ctrl+G) creates a group with "Edit group" and/or
  "Mix group". Selecting a track of an active edit group selects the whole group (commands accept
  `exact: true` to ignore groups). Mute and record-arm follow an active mix group whose attributes
  include them; solo does not follow groups.
- **Stereo editing:** a stereo track holds one clip with two channels; every edit applies to both
  channels together. There is no separate linked-mono-track editing mode.

## 12. Nudge

**Purpose:** move clips, clip contents, trims or the selection by an exact amount.

### 12.1 The NUDGE panel
In the Edit toolbar, separate from Grid. It stays visible (pinned at the right, next to the
transport) when the window is too narrow for the whole toolbar; the other groups then scroll.
- **◀ / ▶**: nudge earlier/later with the panel's target (`nudge.apply`).
- **Value** (0.001–100000) and **unit**: "samples", "ms", "s", "ticks" (960 per quarter note,
  through the tempo map), "frames".
- **Target**: "Selected clips", "Clip contents", "Left trim edge", "Right trim edge",
  "Selection/cursor only", "Selection start", "Selection end".
- Favourites **1 smp**, **1 ms**, **10 ms**.
- Readout of the exact displacement, e.g. "= 480.00 smp @ 48.0 kHz" for 10 ms (ticks: "N ticks
  (tempo map)").

Defaults: 1 ms, Selected clips. Setting the value: `nudge.set`; current state: `nudge.info`.

### 12.2 Targets in detail
| Target | Command (Edit › Nudge) | What moves |
|---|---|---|
| Selected clips | `nudge.clips` | The clips (and their clip-group members). With no clip selected: the selection/cursor. |
| Clip contents | `nudge.contents` | The audio inside the clip; the clip boundaries stay (content slip). Needs source handles. |
| Left / right trim edge | `nudge.trim_start` / `nudge.trim_end` | The clip start (with its clip-gain envelope) / end. |
| Selection/cursor only | `nudge.selection` | The edit selection or cursor, nothing else. |
| Selection start / end | `nudge.selection_start` / `nudge.selection_end` | One edge of the selection. |

### 12.3 Rounding and exactness
Each press advances a running count of steps from where the target started; the position is that
count × step, rounded to the nearest sample (ties to even). Ten 1 ms steps at 44.1 kHz therefore
move exactly 441 samples, and going back the same number of steps returns exactly to the start.
A step smaller than one sample may move nothing on a given press (the command reports
`below_one_sample`) until the accumulated amount crosses a sample. The running count restarts when
you change the value, the clip set, or move the target some other way, or when the sample rate,
tempo map or frame rate changes. Undo restores both the positions and the running count.

### 12.4 When a nudge is refused
The whole nudge is refused and nothing moves when: a clip is locked ("clip `x` is locked"); it
would move before the session start; it would run out of source handles; a clip would become
zero length; content or trim-start nudge is applied to a time-stretched clip; content nudge
targets a clip without audio; selection start would pass its end; or a clip target is used with
no clip selected ("select one or more clips first").

### 12.5 Keyboard
| Keys | Action |
|---|---|
| Num+ / Num− | Nudge selected clips later / earlier (the cursor when none) |
| Alt+Num± | Trim start |
| Ctrl+Num± | Trim end |
| Alt+Shift+Num± | Selection start |
| Ctrl+Shift+Num± | Selection end |
| Ctrl+Alt+Num± | Next / previous nudge value from the preset list (1, 10, 100 smp; 0.1, 1, 5, 10, 20, 50, 100 ms; 1 s; 240, 320, 360, 480, 960 ticks; 1 frame; a value not in the list jumps to 1 smp) |
| Shift+Num± | Deliberately unassigned |
| Alt+Left / Alt+Right | Studio One-style profile only: nudge clips |
| , / . | Nudge clips, only while Commands Keyboard Focus (the toolbar "a-z" button) is on |

**Limits:** nudge keys work only when the Edit window is the main view and no text field, dialog
or plugin window has keyboard focus. The application cannot tell keypad + / − from the main-row
keys (a toolkit limit); how the main-row keys behave depends on your keyboard layout and has not
been checked on your keyboard. Content nudge from the keyboard (the Windows-key chord) is **not
available** because the Windows key is not reported to the app — use the panel target "Clip
contents" and ◀ ▶. The panel's help text mentions remapping `nudge.apply`, but neither shortcut
table has a row for it (Known issue K7); rebinding needs an imported shortcut file.

## 13. Mixer views: Console, Visual and Split

### 13.1 Open and switch views
**Window › Mix** (Ctrl+= also lands here; it does not toggle back, Known issue K6) opens the Mix
window; **Window › Edit** returns. The Mix window header has **Console** (channel strips),
**Visual** (soundstage) and **Split** (both) — command `mix.view`, stored with the session. The
same header holds **Snapshot** A/B/C (§14.3), **Create Bus from Selection**, **Create FX Return**
(§15) and, outside Console view, **Filter channels**. All views edit the same mixer; switching
views never changes the sound.

### 13.2 Channel controls
| Control | Where | Command | Range / notes |
|---|---|---|---|
| Fader | Console strip; Visual node up/down | `mix.volume {tracks?, db | delta_db}` | −144…+12 dB |
| Input trim ("PREAMP") | Console strip | `mix.trim` | ±24 dB in the strip (command: −144…+24 dB) |
| Pan | Console knob; Visual node left/right | `mix.pan {tracks?, pan: -1..1, index?}` | `index` 1 = second panner of a stereo channel |
| Stereo width ("W n%") | Console "W" row; Visual node side handles | `mix.width {tracks?, width: 0..2}` | 0 % mono, 100 % untouched (exact pass-through), 200 % double side. Stereo channels only (mono tracks are skipped); not on Master. |
| Mute / Solo | strip buttons | `mix.mute` (Shift+M), `mix.solo` (Shift+S, `exclusive?`) | see §13.4 |
| Record / Input | strip buttons | `mix.record_arm` (Shift+R), `mix.input_monitor` (Shift+I) | §6 |

**Visual view:** drag a node — Shift locks an axis, Alt gives fine control, Esc cancels, double-click
resets to 0 dB / centre. The canvas shows +12…−60 dB; values outside are marked ▲/▼, never changed.
Master, Video, VCA and Folder tracks have no node. The channel list beside the canvas selects,
hides (view only) and edits values numerically; double-click a name to rename. A badge shows the
destination ("→ Vocal Bus +1 send"; muted sends not counted) and a red **CLIP** tag shows while a
meter clips.

**Undo:** fader, pan, width, mute and solo changes are undoable (a fader drag is one step).

### 13.3 Mix profile and pan law
`mix.profile` (Search: "Mix Profile"; no other control): **standard** (constant-power −3 dB pan
law) or **logic** with law `0db`, `-3db` (default) or `-3db_compensated`, stereo mode `balance`
or `stereo_pan`. These are SML's own published curves. New buses are neutral (unity, centred,
100 % width), so nested buses add no pan-law gain. New sessions sum buses and Main in 64-bit
floating point ("graph v2"); sessions from older builds keep their arithmetic until you run
`mix.graph_migrate` (undoable).

### 13.4 Mute and solo behaviour (graph v2)
- A muted channel still feeds its **pre-fader** sends (mute the send itself to stop them);
  post-fader sends stop.
- Soloing a track keeps everything downstream of it (its buses and returns) audible without
  letting unrelated tracks into a shared return; soloing a bus keeps the tracks that feed it;
  solo-safe channels stay audible.
- A VCA's mute mutes its members.
- **Muting the Master track has no effect** in this build (Known issue K2). Use the monitor
  controls (§4.4) or stop the transport.

## 14. Automation and mixer snapshots

### 14.1 Automation modes
`mix.automation_mode {tracks?, mode}` (the Automation window, **Window › Automation**, has mode
buttons and "Enabled parameters"): **off, read** (default), **touch, latch, touch/latch, write,
trim**. Every mode except Off plays existing automation.
**Live writing records volume only**, every 20 ms while playing, one undo step per pass:
Write records throughout; Touch while you hold the fader; Latch and Touch/Latch keep writing after
the first touch. Trim mode writes nothing in this build. Pan, mute, send and plugin automation are
drawn (§14.2), not written live.

### 14.2 Draw and edit automation
- Paint tool (§8.2) or `automation.draw` into the shown lane (volume, pan, mute, send level,
  plugin parameter lanes). One undo step per stroke.
- **Edit › Automation**: Thin (Ctrl+Alt+T, `automation.thin`), Write to Current (Ctrl+/,
  `automation.write_to_current`), Write to All Enabled (Ctrl+Alt+/), Copy to Send (Ctrl+Alt+H).

**Limits:** send pan and send mute lanes can be drawn but are **not played back** (Known issue K11).

### 14.3 Mixer snapshots A/B/C

**Purpose:** compare mix balances quickly.

**Steps:** set a balance, **Shift+click** a snapshot button (A, B or C) to store it. Click a
stored snapshot to recall it (stopped only: "stop playback to recall a snapshot…"). Commands
`snapshot.save`, `snapshot.recall`, `snapshot.info`.

**Stored:** volume, pan, width, mute, solo and visibility of every channel except Master.

**Not stored:** plugins, slots, routing, trims, automation.

**Careful:** clicking an **empty** slot (shown "A·") stores into it (Known issue K12). A track in
Read mode with volume automation follows its automation, not the recalled fader.

**Undo:** save and recall are undoable.

## 15. Buses, sends, FX returns and sidechains

### 15.1 Create a bus from selected tracks

**Steps:** select the tracks (e.g. Lead, Double L, Double R) → **Create Bus from Selection** in the
Mix window header (also Search; command `bus.create_from_selection {tracks?, name?, destination?}`).

**Result:** one step creates a stereo aux named "Bus" (rename it, §13.2) at unity and centre,
routes the tracks into it (their faders, pans, inserts and sends unchanged) and selects it. One
Undo removes all of it.

**If it fails:** tracks feeding different outputs: "…choose a destination (suggested: main)" — run
the command with `destination`; a Master, VCA or Folder track cannot be a source; a bus and one of
its own sources cannot be selected together; a routing cycle is refused.

### 15.2 Create an FX return

**Steps:** select the source tracks → **Create FX Return** (`bus.create_fx_return {tracks?, name?,
plugin?, destination?, pre_fader?}`).

**Result:** a return channel plus a send on each selected track, post-fader, starting at −∞ (−144
dB). Dry outputs are unchanged. Load your own reverb or delay on the return (§17.2) and raise the
sends. **If it fails:** "has no free send" (each track has 10 send slots).

### 15.3 Sends
`mix.send {track?, slot?, bus, level_db?, pre_fader?}` assigns a send; `mix.send_level {track?,
slot, db?, pan?, follow_pan?, mute?, pre_fader?}` adjusts it (level −144…+12 dB). Ten sends
(a–j) and ten inserts per channel.

### 15.4 Spill, delete, validate
- `bus.spill {track}` shows the bus and every channel feeding it (directly or by send) and hides the
  rest of the mixer view; run it without a track to exit. View only.
- `bus.delete {track, sends?}` reroutes the bus's sources to the bus's own output and removes or
  reassigns sends that targeted it (undoable). Nothing is deleted from your media.
- `bus.sources`, `bus.validate`: report feeders and routing problems.

### 15.5 Sidechain a plugin from a bus

**Purpose:** key a compressor or gate on another signal.

**Steps:**
1. Create or pick a bus that carries the key signal; send the key track to it (§15.3).
2. Run `mix.insert_sidechain {track, slot, bus}` for the plugin's insert slot. There is **no
   on-screen control** for this in this build, and Search cannot supply the parameters; use the
   control channel or MCP (§25), e.g.
   `soundcraft-cli app engine.execute '{"command":"mix.insert_sidechain","params":{"track":"Lead","slot":"a","bus":"Key"}}'` (insert slots are `a`–`j` or 0–9 from the top).
   Undoable.

**Result:** CLAP plugins receive the bus on their first auxiliary input port; VST3 plugins on
their first auxiliary input bus (if the plugin refuses to activate it, `engine.plugin_capabilities`
shows "(not routed)"). Audio Units have no sidechain.

**Timing:** on aux and master channels the key is sample-aligned; on audio/instrument tracks it is
one audio block late. `mix.latency_report` lists every sidechain with its lag.

**Pending target check:** real third-party sidechain plugins.

## 16. Signal order, Summing and Tape slots

### 16.1 Signal order
`mix.signal_flow {track}` prints the order for any channel (its Master output has errors, see K2).

**Audio, instrument and MIDI tracks**
1. Clips with clip gain and fades (or the live input when monitoring/armed)
2. Input trim and polarity
3. **Tape** slot (plugin, then slot trim)
4. Pre-fader inserts, in slot order (bypassed inserts keep their latency as a delay)
5. Stereo width (stereo channels)
6. Pre-fader send tap
7. Fader (volume, trim automation, VCA offset) and mute
8. Post-fader inserts
9. Delay-compensation alignment
10. Meter
11. Sends (pre-fader from the tap, post-fader from here), each with its own panner
12. Output panner → Main or a bus

**Buses (aux and folder):** bus sum (64-bit) → **Summing** slot → trim/polarity → **Master Tape**
if Pre → pre-fader inserts → width → pre-fader tap → fader → post-fader inserts → **Master Tape** if
Post (default on buses) → alignment → sends → output panner.

**Main (Master track):** Main sum → **Summing** slot → **Master Tape** if Pre (default on Main) →
**all Master inserts in slot order, before the fader** (the Post-fader setting is ignored on
Master) → fader (volume automation only) → **Master Tape** if Post → output. Main has no trim,
width, mute or sends. The click is added after Main; monitor volume/dim/mono act at the device
output only.

### 16.2 Summing and Tape slots

**Purpose:** dedicated places for a tape or summing plugin you own, separate from inserts.
- Each audio/instrument/MIDI track has a **Tape** slot; each bus and Main have a **Summing** slot and
  a **Master Tape** slot.
- Console "SLOTS" section: "Tape None"/"Tape <name>", "Sum Clean", "MTape▲" (Pre) / "MTape▼"
  (Post), "(byp)" when bypassed, red when the plugin is missing. Click for the menu: "Clean
  (identity)" / "None (identity)", your installed plugins, "Bypass (keeps latency compensation)",
  "Trim", "Pre (before inserts)" / "Post (after fader)", and "Native summing/tape models: not yet
  available".
- Command `mix.slot {track?, slot: tape|summing|master_tape, plugin?, bypass?, trim_db?: -24..24,
  position?: pre|post}` (undoable). Change the plugin or position while stopped.
- An empty slot is exact pass-through. A plugin in a bus's Summing slot processes the summed bus
  signal, not the individual tracks.
- During playback each configured slot shows two thin meters (input, output; −60…0 dBFS, red at
  0 dBFS) and the tooltip "In x dBFS → Out y dBFS" for gain staging.
- `mix.audition_bypass {summing?, tape?, inserts?, all?}` temporarily bypasses that processing
  (keeping its delay) for comparison; never saved, never applied to exports; no on-screen control.

**Not available:** native analog summing and tape models (Planned). No drive/noise/bias controls
exist.

## 17. Plugins

### 17.1 Scanning
At start-up, when nothing is recording, the app scans CLAP and VST3 plugins in separate worker
processes, one bundle at a time ("Scanning installed plugins…", **Cancel scan**). A plugin that
crashes, hangs (20 s), fails or returns bad data is **quarantined** with the reason and skipped
later; it never takes the app down. Results are cached by path, size and date, so only new or
changed plugins are rescanned. The insert menu ends with **Quarantined (n)** (with **Re-enable**)
and **Rescan plugins**. Audio Units (macOS) are not scanned in isolation. In Safe Open nothing is
scanned (§3.3).

### 17.2 Insert a plugin

**Steps:** in a Console strip, click an empty insert slot (right-click an occupied one to change
it) → **CLAP** or **VST3** (grouped by vendor) → the plugin. Command `mix.insert {track?, slot?,
plugin, params?}`.

**Result:** the plugin is inserted (undoable). Clicking an occupied slot opens its plugin window:
**Bypass**, **Presets ▾**, **Open Plugin Editor** / **Close Plugin Editor**, and generic parameter
controls.

**Other:** `mix.insert_bypass`, `mix.insert_move`, `mix.insert_remove`, `mix.insert_post`
(post-fader; stopped only; ignored on Master), `mix.insert_param(s)`.

**Limits:** no plugin ships with the app; the menus list only what you have installed.

### 17.3 Plugin editors
| Format | Editors |
|---|---|
| CLAP | Floating window on Windows, macOS and Linux (plugins that only support embedded editors are refused) |
| VST3 | macOS; Windows in a floating Win32 window that follows SoundCraft (closing it closes the editor) — **not yet observed on Windows**; Linux: "unsupported" |
| Audio Units | No editors |
| Sandboxed instances | No editor (generic controls only) |

### 17.4 Presets and state
- **Presets ▾** offers "<factory default>" and your preset files (parameter values only, stored in
  `Presets\`), plus **Save**.
- The plugin's full internal state is saved inside the session automatically.
- `track.save_preset` / `track.load_preset` store and load a whole channel strip.

### 17.5 Versions and missing plugins
- The plugin version at insert time is saved; `session.plugin_versions` reports `same`, `changed`,
  `unrecorded` or `missing` per slot.
- A plugin that is missing or fails to load keeps its id, parameters and state, and the slot passes
  audio through. `session.missing_plugins` lists them; the slot shows red.
- **Exports refuse** to run while a plugin is missing or failed (the result would not be the mix
  you hear) unless you pass `acknowledge_missing: true` (§18.4).

### 17.6 Latency and delay compensation
With delay compensation on, every channel is aligned to the slowest, including direct-to-Main
outputs. `mix.latency_report` lists strips, the worst latency, latent processors and sidechain
lags. Bypassing an insert or slot keeps its latency as a plain delay, so timing stays aligned.
For tracking: §4.5 (`mix.tracking_deactivate`).

### 17.7 Sandboxed plugins (opt-in, command only)

**Purpose:** contain a plugin you do not trust.

**Steps:** insert it as `sandbox:<plugin id>` with `mix.insert` (no menu entry).

**Result:** it runs in a worker process with exactly one audio block of added latency
(compensated). If it crashes or stops answering (1 s), the slot goes silent for that block, the
fault is recorded (`engine.host_faults`) and the worker restarts with its last state (up to 5
restarts) — except while recording, when it stays silent and flagged. Offline exports wait up to
3 s per block for it and **fail** rather than export silence.

**Limits:** in-process plugins (the default) have no crash containment: a crashing plugin can
close the app; autosave and take journals protect your work (§23).

### 17.8 What a plugin reports
`engine.plugin_capabilities {id?}` shows, per installed plugin, what the host read from a live
instance: audio inputs/outputs, aux/sidechain inputs, note input, parameter count and units,
editor availability on this platform, state support, declared latency and tail. It proves the
interface as hosted, not the sound. The SML plugins appear as AWAITING_EXTERNAL_PLUGIN.

## 18. Mastering and export

### 18.1 The Master layout
Menu-bar layout button **Master** (or `window.master`). It shows "MASTER", **Back to Mix**, the
Master strip (its inserts are the mastering chain, applied exactly once), "Program and reference",
and "Meters (offline over the program)" with **Measure**.

### 18.2 Program and reference
- **Program:** by default the session mix. `mastering.program {source}` can set an imported stereo
  file to play through the Master chain instead (offline bounce and meters use it).
- **Reference:** `mastering.reference {source, gain_db?}` registers an imported file as the
  reference; **Level match** (`mastering.level_match`) measures both integrated loudnesses and
  stores an audition gain; **Clear** removes it.
- **Not available: listening to the reference inside the app.** "A/B: listen to reference" stores a
  setting, but no reference audio plays in this build (Known issue K1). The on-screen hint
  "Clip List › right-click › Use as reference" names a menu item that does not exist; use the
  command. Compare by ear in another player at the level-matched gain (§24 walkthrough E).
- When a program file is set, Meters and Level match measure that file **without** the Master
  chain.

### 18.3 Meters
**Measure** (`mastering.meters {start?, end?}`) runs offline over the program and reports sample
peak, true peak (4× oversampled, BS.1770-4), momentary and short-term maxima, integrated loudness
(gated −70 LUFS absolute / −10 LU relative), loudness range (EBU Tech 3342), correlation and
mono-sum peak. They agree with the analytic EBU Tech 3341/3342 cases and with an independent
implementation (FFmpeg 6.1.1) on 54 of 56 comparisons; they are not certified.

### 18.4 Export the mix

**Dialog:** **File › Bounce Mix...** (Ctrl+Alt+B): **File**, **Format** (WAV, AIFF, FLAC),
**Bit Depth** (16, 24, 32f), **Normalize to -0.1 dBFS**.

**Command (all options):** `file.bounce_mix {path, format?, bit_depth?, start?, end?,
normalize?, dither?, rate?, tail?, fold_down?, metadata?, report?, overwrite?, acknowledge_missing?}`.
| Option | Behaviour |
|---|---|
| Range | `start` and `end` together; otherwise the selection, otherwise the whole session |
| `dither` | TPDF ±1 LSB on 16/24-bit, **on by default** (the dialog has no switch); never on 32f |
| `rate` | Sample-rate conversion (windowed sinc), 8 000–384 000 Hz |
| `tail` | Seconds (0–600) or `"auto"` (the longest tail any processor declares), so reverbs are not cut |
| `normalize` | Peak to −0.1 dBFS, before rate conversion |
| `report` | Writes `<file>.report.json`: settings, revision, plugin ids/versions, measured levels and a decoded-file check |
| `overwrite` | Required to replace an existing file (otherwise refused) |
| `acknowledge_missing` | Export anyway with a missing/failed plugin passed through |
FLAC cannot be 32f. No MP3/AAC export. The exporter adds no limiter or normaliser of its own
(normalize is off unless chosen). Reference audio, monitor controls and assistant speech never
reach an export. The parameter `source` is documented but ignored (Known issue K13).

**Verify:** check the report's `verified` block (header, frames, rate, channels) or reopen the file
and measure it (§24 E).

### 18.5 Stems and named exports
- `file.bounce_stems {dir, format?, bit_depth?, start?, end?}`: one file per unmuted audio,
  instrument or MIDI track, each rendered soloed ("<session> - <track>.<ext>"). Always dithered on
  integer depths; existing files are replaced without asking.
- `file.export_path {path, kind: raw|edited_dry|processed|full_mix, track?, start?, end?}`:
  **raw** (the original source of the first clip), **edited_dry** (comp, clip edits, clip gain,
  fades), **processed** (the channel's own output, before any bus and without the Master chain),
  **full_mix**. Always 32-bit float WAV; an existing file is replaced without asking.
- Summing processed stems is not guaranteed to recreate a mix with shared or nonlinear processing.
- **Long exports:** offline renders are limited to about 93 minutes of stereo at 48 kHz (untested
  beyond that).

## 19. SML Remote (phone)

### 19.1 Start the service and pair a phone

**Purpose:** transport, track and monitor control from a phone's web browser on your network.

**Steps:**
1. **Search › SML Remote** (window "SML Remote").
2. **Bind** (editable while stopped): `127.0.0.1:0` = this computer only; `0.0.0.0:8787` = every
   network interface on port 8787 (your LAN). Click **Start**. The window shows "Running at
   http://<address>/ (plain HTTP)"; with 0.0.0.0 use this computer's LAN address and the same port.
3. Click **Open pairing (2 min)**. A 6-digit code appears with its remaining seconds.
4. On the phone, type the address into the browser, enter the code, **Pair**.

**Result:** the phone appears under "Paired phones" ("name (id)" with **Revoke**). The phone page
has transport (Play, Stop, ● Rec, Loop, Pre-roll, Count-in, + Marker, Audition sel.), take rating
1–5, per-track Select, Arm, Mon, M, S and fader/pan, "Monitor (speaker path only, never export)"
with Dim, Mono and level, an **Ask** box (typed questions to the internal assistant, §20) with
"Run plan (one undo step)", and **Forget this phone**.

**Security:** pairing codes are single-use; five wrong codes close pairing; a paired phone holds a
24-hour cookie; requests from other web origins are refused; nothing listens unless you started
it. Stopping the service forgets every pairing.

**Limits:**
- Plain HTTP only: no encryption, no installable app, **no phone microphone** (browsers allow the
  microphone only on HTTPS pages), no QR code (type the address).
- The phone can only run an allow-listed set of actions (transport, markers, pre/post-roll, count
  off, selection, track volume/pan/mute/solo/arm/monitor, take rating and rename, monitor level,
  typed assistant requests). It cannot save, open, export or delete.
- The **Start with the app** checkbox is saved but does not start the service at launch (Known
  issue K4); press Start each session.
- Pending target check with a real phone and network.

### 19.2 Disconnects, stale edits and receipts
The desktop stays in charge. A phone's receipt means "accepted", not "happened at this sample".
Losing the phone (screen lock, closed browser, Wi-Fi drop, revoke) never stops playback or a
recording; reopening the page shows the current state. A Record request from before a reconnect
is refused ("stale record request: fetch /state again, then record"). If two phones edit the same
thing, the one working from an older state is told to refresh. Rate and queue limits answer
"too many requests" rather than queueing unbounded work.

## 20. The internal assistant

The **internal assistant** is part of SML Studio. It is not the same thing as an external AI
operator (such as a Grok bot) controlling the app from outside; that is §25 and the
[Grok operator guide](grok/GROK_OPERATOR_GUIDE.md).

### 20.1 Open it and ask
**Search › Assistant** opens the window "Assistant". Type into the field ("ask, or say what to do
(e.g. "mute Kick", "is the lead clipping?")") and press **Ask**. The status line says which answer
source and speech features are on and "nothing leaves this machine". The phone's **Ask** box and
the command `assistant.ask` use the same rules (but never the local model, §20.5).

### 20.2 Modes
- **Explain:** names the command, its menu location, the active profile's shortcut, parameters,
  status (including DISABLED entries) and known limits, with citations (command ids and "guide §n"
  of this manual as compiled into your build). A feature not in this build is reported unavailable.
- **Operate:** turns requests such as "mute Kick", "set the Vox fader to −6 dB", "rename Lead to
  Lead Vox" into a **plan** of validated commands shown before anything happens. **Run plan**
  executes it as one undo step. If the session changed since the plan was built ("the document
  changed since this plan was built: ask again") or any step is rejected, nothing is applied.
  Ambiguous names make it ask which track; missing tracks are refused; deleting files, sending,
  publishing or purchasing need your own authorization (it never plans them). Marker, clip and
  transcript text is treated as data, never as instructions.
- **Engineer:** measures the selected range (sample peak, 4× true peak, integrated loudness, noise
  floor, full-scale samples) and labels each statement measured / inference / preference:
  clipping locations, a gate threshold from the noise floor (using only a plugin you have), a
  vocal-bus plan, a balance between two tracks with a fader proposal to preview and undo. It never
  claims to have heard what it did not measure.

### 20.3 Spoken replies
Section "Voice: spoken replies and push-to-talk phrases": **Speak replies (monitor output only;
silent while recording)**, **voice pitch (Hz)**, **Stop speaking**. The first sentence of each answer
(up to 200 characters) is spoken in a built-in robotic voice on the monitor output only; never in a
bounce or a take. Command `assistant.voice`.

### 20.4 Push-to-talk with enrolled phrases

**Purpose:** say one of your own short command phrases instead of typing it.

**Steps:**
1. In the Voice section type a phrase ("phrase, e.g. mute kick"), hold **Hold to record phrase**
   ("Recording…") while saying it; release. Record each phrase once or more, in the room and with
   the microphone you will use (up to 64 phrases, 4 recordings each).
2. Hold **🎤 Hold to talk** ("🎤 Listening…") while saying a phrase; release.

**Result:** a recognised phrase appears as 🎤 text and is answered exactly like typed text (plans
still need Run plan). Anything not close to an enrolled phrase is rejected and nothing happens.

**Limits:** a phrase recogniser, **not dictation**; uses the default input device; needs a quiet
room (on synthetic test speech: reliable at 30 dB signal-to-noise, unreliable at 20 dB, failing at
10 dB); off while recording; no wake word or always-listening mode. Phrases are stored as numeric
features in `assistant.json`, never as audio. Pending target check with your microphone.

### 20.5 Optional local language model

**Purpose:** let a language model you run on this computer phrase answers and propose simple
actions.

**Before you start:** a local OpenAI-compatible server (for example llama.cpp `llama-server`,
Ollama, LM Studio). SML Studio ships no model.

**Steps:** open "Local language model (optional)" in the Assistant window: tick **Use the local
model**, set **Endpoint** (default `http://127.0.0.1:8080/v1/chat/completions`), **Model**,
**Time limit (s)** (1–600), **Apply**.

**Result:** answers come from the model, citing the same sources; the model receives the session's
track list and the command help rows relevant to your question. It may only **propose** mute,
solo, volume (−144…+12 dB), pan, record-arm and rename actions (at most 8), each validated and
shown as a plan you run yourself. A slow answer shows **Cancel (N s)**; if the server is down or
slow, the grounded rules answer and say why.

**Limits:** only `http://` addresses of this computer (localhost or a loopback IP) are accepted.
The model is used only from the desktop window, never by `assistant.ask` through the phone, the
control channel or MCP. **Not observed with a real model**: no model was available when this build
was tested. If you point it at a local program that forwards to an online service, your session
details leave this computer — that is your decision to make, not a default.

## 21. Shortcut profiles

**Where:** **Setup › Keyboard Shortcuts...** (window "Keyboard Shortcuts").
- Profiles: **Pro Tools style** (default; the registry's chords plus F-keys, Enter for a marker and
  single-key Commands Keyboard Focus), **Studio One style** (1–8 tools, D duplicate, E/W zoom,
  Alt+arrows nudge…; every row marked "unverified" because that product's manual was not
  available to check), **Custom** (a profile chosen in "based on" plus your overrides).
- Click a chord cell and press the new chord ("Press the new chord for … (Escape cancels)") to
  reassign; that moves you to Custom. **Unbind**, **Reset**, **Import…**, **Export…** (JSON format
  `sml-shortcuts-1`), **Search**.
- The conflict inspector lists duplicate chords, chords the operating system reserves and chords
  this build cannot deliver (keypad location and the Windows key).
- Search and Command Help show the active profile's binding.
- Full tables, including which chords are **shadowed** (declared but never received because an
  earlier row has the same chord): [reference/SHORTCUTS.md](reference/SHORTCUTS.md).

**Known keyboard issues in this build (K5–K7):** in Pro Tools style, **Space starts playback but
does not stop it** (use the Stop button, the Transport window, or switch to Studio One style where
Space toggles); **F1 opens Command Help** rather than Shuffle (click SHUFFLE); **Ctrl+=** only goes
to the Mix window; **Ctrl+Alt+L** runs "Move selection left", not Loop. In Studio One style **N**
(snap) and **Alt+X** (split) report errors. Clip rating chords Ctrl+Alt+0–5 do nothing.

## 22. Themes, scaling and layouts

- **Window › UI Customization** or **Setup › Appearance** (window "Appearance"): **Interface
  scale** (75–150 %), **Reduce motion**, **Theme** (Dark, Light, High contrast, System, Custom),
  **Auto-update configuration**, **Reset**, **Theme Editor…**. Interface scale and Reduce motion are
  not remembered after a restart.
- **Theme Editor:** colour per token, "Typography scale" (0.8–1.5), **Import…**, **Export…**,
  **Reset** (back to Dark). A theme must keep text readable (3:1 contrast) and record/solo/mute/clip
  states distinguishable, or it is refused with the reason. A broken saved theme never stops
  launch (you get Dark). Record, clip and overload states are also shown by shape or label.
- **Layouts:** the menu-bar buttons **Record · Edit · Mix · Master** switch the four layouts of one
  window (`window.record`, `window.edit`, `window.mix`, `window.master`). Record docks the
  recording essentials beside the timeline; Master is the mastering workspace (§18). Switching
  changes nothing in the session.
- **Window › Configurations:** **New Configuration…**, numbered recall buttons, ✕ to delete, Update
  Active Configuration (overwrites the last one). Stored in `ui.json`.
- **Window › Reset Panel Layout** restores the default docking.
- When the window is narrow (for example 1366×768 at 150 % scaling), the Edit toolbar keeps the
  transport and Nudge pinned on the right and scrolls the other groups, tools first.

## 23. Autosave, recovery and troubleshooting

### 23.1 Autosave
Every 60 s a modified session is copied into a rolling store of recovery generations (20, up to
256 MiB) under `Autosave\<session>\` in the settings folder. Each generation is written to a
temporary file, flushed, re-read and checked before it replaces anything; the previous one is
never touched first. It runs on its own thread, so recording never waits. Plugin state is captured
only while stopped; the Recording window shows "Autosave: generation N committed at HH:MM:SS"
and how old the plugin state is. Autosave is separate from Save.

### 23.2 Recover after a crash

**Steps:** **Search › Recover Autosave…** (window "Recover Autosave").
- "Recovery points": one row per session with its newest valid generation; **Open as unsaved
  session** opens it unsaved ("Recovered X (unsaved — choose File › Save As)"). Save it under a
  new name.
- "Unfinished takes": capture files in the temp capture folder and in the open session's `Audio
  Files\`. **Recover to a new track** keeps every journaled frame and imports the take at the
  session start (all device channels).

**Careful:** do not use this window while recording (K21). Recovering a take shortens the capture file in place to its last durable checkpoint
and rewrites its header (the window's footnote says otherwise; Known issue K14). Copy the WAV
first if you want the untouched file. Only the newest generation per session is offered; older
generations stay in `Autosave\<session>\gen-NNNNNN.scraft` (opening one directly with File › Open
Session... has not been verified).

**Limits:** tested failure model is an abrupt process kill (at most about one second of a take
lost). Power-loss behaviour is pending a target check.

### 23.3 Troubleshooting
| Symptom | What to do |
|---|---|
| App will not start, or a plugin misbehaves | Start with `--safe` (§3.3); remove or quarantine the plugin; restart normally. |
| Interface preferences broken | `--reset-prefs` (§3.3). |
| "Device not changed" / "Input not changed" | Read the reason; pick a supported rate/buffer; another program may hold the device. |
| No sound | Check the output device (§4.1), Master fader, monitor Volume/Dim, solo states, and that the track output goes to Main. |
| Space does not stop playback | Known issue K5; click Stop. |
| A window you expect is not in the menus | Open it from Search (§1). |
| Export refused: missing plugin | Install or re-enable the plugin, or export with `acknowledge_missing` knowing it will be passed through. |
| Export refused: file exists | Choose another name or pass `overwrite: true`. |
| Plugin hangs or crashes repeatedly | Insert it as `sandbox:<id>` (§17.7), check `engine.host_faults`. |
| Something changed that you did not intend | Edit › Undo; Window › Undo History; for whole-session problems, reopen the last save or a recovery generation. |
| Check the session's media | `session.audio_health` (lists missing or unreadable audio; no signal analysis). |
| Logs | `%APPDATA%\SoundCraft\logs\soundcraft.log`. |

## 24. Professional walkthroughs

Each walkthrough uses only controls and commands that exist in this build. The command
equivalents of walkthroughs B, C and E were run end to end on synthetic audio with the headless
CLI (evidence: `docs/evidence/documentation/walkthrough-commands.log`). The parts that need a
microphone, an interface, your plugins or your ears are **pending target check** and are marked.
Values marked *starting point* are first settings to adjust by ear, not rules.

### A. Record and comp a lead vocal

**Before you start:** interface connected; microphone on input 1; session created and saved
(§5.1–5.2) so takes land in its `Audio Files\` folder.
1. **Device:** Setup › Playback Engine... → Scan output devices / Scan input devices → your
   interface; buffer *starting point* 128 frames for tracking; Apply (stopped only). Read
   **Effective callback** to see what the driver gave you (§4.1–4.2).
2. **Track:** Track › New... → 1 mono audio track; rename it "Lead" (double-click its name).
   Input "In 1" (§6.1). Arm it (red button).
3. **Monitoring:** either your interface's direct monitoring with monitoring mode **Hardware**, or
   software monitoring with **Input monitor** on (§4.5). Keep latent plugins off the lead while
   tracking (`mix.tracking_deactivate`).
4. **Level:** set the preamp so loud phrases peak around −12 to −6 dBFS on the track meter
   (*starting point*; leaves headroom; the take is 32-bit float, so low peaks lose nothing).
5. **Protect the main take:** playlist arrow › Duplicate... (§7.1).
6. **Loop-record the section:** select the verse with the Range tool; Options › Loop Record
   (Alt+L); Count Off on (Recording window, *starting point* 1–2 bars); Record; let it run 3–5
   passes; Stop. Each pass becomes a playlist "Lead.NN" (§6.5).
7. **Audition and rate:** set the track view to playlists; solo the track; select a phrase and play
   it from each lane (or hold the **Listen** tool over it); rate takes with Clip › Rating.
8. **Comp:** for each phrase, select its range and click the best take's lane (promote, §7.2).
9. **Joins:** select across each join → Edit › Fades › Create... → Equal Power (*starting point*:
   5–20 ms for vocal joins; longer in sustained vowels). Listen to every join.
10. **Save** (Ctrl+S).

**Result:** a comped lead on the main playlist; every take kept on its own playlist; source files
untouched.

**Undo/recovery:** each promotion and fade is one undo step; a crash → Search › Recover Autosave… (§23.2).

**Limits:** pending target check on your interface and microphone. Punching into a gap with
pre/post-roll can clear clips in the roll zones (K8); duplicate the playlist first.

### B. Edit and mix a lead with doubles and ad-libs

**Before you start:** Lead comped (A); doubles "Dbl L", "Dbl R" and "Adlib" recorded or imported
(File › Import › Audio...).
1. **Clean the doubles and ad-libs:** select their clips → Edit › Strip Silence (Ctrl+U)
   (*starting point*: threshold 12–20 dB above the noise floor, start pad 10–20 ms, end pad
   20–50 ms). Add short fades at clip edges (Edit › Fades › Create...).
2. **Breaths:** split around loud breaths (Split tool) and lower them with clip gain (*starting
   point* −6 to −10 dB) rather than deleting them; or draw a dip with the Paint tool on the clip.
3. **Timing:** align a double to the lead with Nudge: target "Selected clips", 1–10 ms steps
   (Num+ / Num− or ◀ ▶), comparing transients by eye and ear; for single words, Bend anchors
   (§10). Nudge "Clip contents" slides the audio inside a clip without moving its edges.
4. **Keep parts together:** Clip › Group the clips of one phrase; Nudge then moves the whole group
   (dragging moves only selected clips, §11).
5. **Balance (*starting point*):** doubles 6–10 dB under the lead, panned left/right (±60–100 %);
   ad-libs off-centre and lower than the lead. Commands: `mix.volume`, `mix.pan`.
6. **Compare balances:** Shift+click snapshot **A**; change the balance; Shift+click **B**; click A
   and B to compare (stopped). When comparing processing on/off, match loudness first (Engineer
   mode or meters) so louder does not sound "better".
7. **Save.**

**Result:** checked headless: import, rename, fades, clip gain, Nudge 10 ms (480 samples at
48 kHz) and back, volume and pan, snapshots A/B and recall all succeeded as single undo steps
(Window › Undo History lists them).

**Limits:** listening judgements are yours; nothing here has been listened to.

### C. Build a vocal bus and effects sends
1. **Vocal bus:** select Lead, Dbl L, Dbl R, Adlib → **Create Bus from Selection** → rename the new
   "Bus" to "Vocal Bus". One undo step; the tracks now feed it.
2. **Bus processing:** on Vocal Bus insert your own compressor and EQ (§17.2). *Starting point*
   for glue compression: 2:1, slow attack (10–30 ms), auto/medium release, 1–3 dB of gain
   reduction. Optional: a summing plugin you own in the bus's **Summing** slot (§16.2).
3. **Reverb return:** select the vocal tracks → **Create FX Return** → rename it "Vocal Verb";
   insert your reverb on it at 100 % wet. Sends start at −∞; raise them with the send controls or
   `mix.send_level` (*starting point*: lead −18 to −12 dB, ad-libs a little higher).
4. **Delay return:** repeat step 3 for a delay ("Vocal Delay"), *starting point* a quarter- or
   eighth-note delay, filtered, sends lower than the reverb.
5. **Optional ducking:** send the lead to a bus "Duck Key"; on the delay return's compressor run
   `mix.insert_sidechain {track: "Vocal Delay", slot, bus: "Duck Key"}` (§15.5) so the delay dips
   while the lead sings. No on-screen control; pending target check with your plugin.
6. **Check routing:** `bus.validate` (expects `{"ok": true}`), `mix.signal_flow {track: "Lead"}`,
   `mix.latency_report`.

**Result:** checked headless: bus creation rerouted all four tracks; the FX return added one send
per track at −∞; validation passed.

**Mute/solo:** soloing the lead keeps Vocal Bus and the returns audible without letting other
tracks into the reverb; muting a track still feeds its pre-fader sends (§13.4).

### D. Mix with your own plugins

**Before you start:** your CLAP or VST3 plugins installed in their standard folders.
1. **Scan:** start SML Studio; the scan runs in worker processes. If a plugin is quarantined, read
   the reason under the insert menu's **Quarantined (n)**; **Re-enable** only if you trust it.
2. **Check what the host sees:** `engine.plugin_capabilities {id}` (inputs, sidechain, editor,
   state, latency, tail).
3. **Insert** on a channel (§17.2); click the slot to open its window; **Open Plugin Editor**
   (VST3 editor windows on Windows: pending target check).
4. **Tape and summing plugins:** load them into the Tape / Summing / Master Tape slots (§16.2).
   Use the slot's in/out meters to stage gain (*starting point* for analog-modelled plugins:
   peaks around −18 to −12 dBFS into the slot; follow the plugin's own guidance).
5. **Compare fairly:** bypass keeps the plugin's latency, so timing does not shift; match the
   loudness of bypassed and active states (adjust the plugin's output) before judging.
6. **Presets:** Presets ▾ › Save for parameter presets; the full plugin state is saved with the
   session automatically.
7. **Moving the session to another computer:** `session.plugin_versions` and
   `session.missing_plugins` show what changed or is missing; missing plugins pass audio through
   and block exports until acknowledged.
8. **A plugin that crashes or hangs:** restart with `--safe`, then insert it as
   `sandbox:<id>` (§17.7), or replace it.

**Limits:** no plugins ship with SML Studio; compatibility of any specific plugin is pending a
target check. The SML plugin family is not available yet.

### E. Master against a reference and verify the export
1. **Master chain:** **Master** layout → insert your mastering plugins on the Master track (all
   Master inserts run before the Master fader, §16.1).
2. **Register the reference without it entering the mix:** File › Import › Audio... the reference;
   note its source id (`session.inspect` → `sources`); delete the reference's new **track** (the
   source stays in the session); run `mastering.reference {source: <id>}`.
3. **Measure:** **Measure** (or `mastering.meters`) for the program; `mastering.level_match` gives
   `program_lufs`, `reference_lufs` and the gain that would match the reference to the program.
4. **Listen at matched loudness, outside the app:** in-app reference playback is **not available**
   (K1). Export your master (step 6), then play it and the reference in another player with the
   reference turned down (or up) by the level-match gain. Judge tone, depth, dynamics and
   translation at equal loudness: louder is not better, and loudness is not quality.
5. **Targets (*starting point*, check your distributor's current specification):** many streaming
   services normalise to around −14 LUFS integrated; a true-peak ceiling of −1 dBTP leaves room for
   lossy encoding. Keep a dynamic master unless the genre demands otherwise.
6. **Export:** `file.bounce_mix {path, bit_depth: "24", tail: "auto", report: true}` (or File ›
   Bounce Mix... for the basic settings; dither is on by default for 16/24-bit). For a 44.1 kHz
   delivery add `rate: 44100`; for 16-bit, `bit_depth: "16"`.
7. **Verify:** open `<file>.report.json`: `verified.header_ok` true, `decoded_frames`,
   `decoded_rate` and `decoded_channels` as expected, levels recorded. Then measure the file
   itself: new session → import the export → delete its track → `mastering.program {source}` →
   `mastering.meters {start: 0, end: <length in samples>}` (an empty session needs the explicit
   range). The integrated loudness should match the report within dither noise.

**Result:** checked headless: reference registered and level-matched (−1.24 dB in the test), the
mix exported with `header_ok: true`, and the exported file re-measured at −9.963 LUFS against
−9.963 LUFS in the session.

**Limits:** meters are checked against analytic and independent results, not certified;
listening checks are yours.

## 25. External control: CLI, control channel and MCP

These are the documented ways for another program (a script, an automation tool or an external
AI operator) to drive SML Studio. They run the same commands as the menus. The
[Grok operator guide](grok/GROK_OPERATOR_GUIDE.md) gives the rules an AI operator must follow.

### 25.1 Headless CLI (`soundcraft-cli.exe`)
Works on session **files**, without the app window. `run [--in FILE | --demo] --cmd ID[=JSON]…
[--bounce] [--save] [--inspect]`, `script FILE [--in FILE | --demo] [--keep-going]` (one `ID JSON`
per line, `#` comments), `commands [FILTER]`, `describe ID`, `info FILE`, `--version`. Transport
and UI commands do nothing headless. Example:
`soundcraft-cli run --in song.scraft --cmd 'mix.volume={"tracks":["Lead"],"db":-3}' --save`.

### 25.2 Control channel (drives the running app)
- **Enable:** start the app with `--control <port>`, for example a shortcut target
  `soundcraft.exe --control 7979` (or set `SOUNDCRAFT_CONTROL_PORT`). There is no menu setting. Use
  a fixed port on Windows (with port 0 the chosen port is printed to a console Windows does not show).
- **Security:** listens on 127.0.0.1 only, **without a password**: any program on this computer can
  control SML Studio while it is enabled. Enable it only when you want an operator to work.
- **Protocol:** one JSON object per line, e.g.
  `{"id":1,"method":"engine.execute","params":{"command":"mix.mute","params":{"tracks":["Lead"],"value":true}}}`.
  Methods: `engine.execute`, `engine.commands`, `session.inspect`, `ui.inspect`, `ui.menu.list`,
  `ui.menu.invoke`, `ui.screenshot`, `ui.click`, `ui.drag`, `ui.key`, `ui.text`, `ui.move`,
  `ui.set`, `engine.parity`, `app.quit` ([reference/CAPABILITIES.md](reference/CAPABILITIES.md)).
  Replies time out after 90 s. Programmatic calls open no dialog, except theme/shortcut
  import/export without a path.
- **CLI helper:** `soundcraft-cli app --port 7979 METHOD [JSON]`. A bare command id is wrapped in
  `engine.execute`, except ids beginning `ui.`, `engine.`, `session.` or `app.` — pass those inside
  `engine.execute` explicitly.

### 25.3 MCP server
`soundcraft-cli mcp --connect 7979` bridges an MCP client to the running app; `soundcraft-cli mcp
[--demo]` runs its own headless engine. Tools: `list_commands`, `execute`, `batch` (each call is
its own undo step), `inspect_session`, `new_session`, `open_session`, `save_session`,
`import_audio`, `bounce_mix`, `parity`, `screenshot`, `ui_inspect`, `ui_set`, `menu_invoke`,
`click`, `drag`, `key`. Details: [mcp.md](mcp.md).

### 25.4 What every external caller must know
- Commands run exactly as asked, **without confirmation dialogs**: `session.new`, `session.open`
  and `session.close` discard unsaved changes; `session.save`/`session.save_as` replace an
  existing file; `file.export_path` and `file.bounce_stems` replace files; only `file.bounce_mix`
  refuses an existing file without `overwrite`.
- Undo: `engine.history` lists the undo stack; `edit.undo` steps back. Commands marked "no" under
  Undo in [COMMAND_REFERENCE.md](COMMAND_REFERENCE.md) cannot be undone.
- `assistant.ask` from outside always uses the grounded rules, never the local model.

## 26. Known issues in this build

Found by reviewing the source of this build; most have not been observed on Windows. They are
documented so that you and any operator can work around them. Workarounds in brackets.

| # | Issue |
|---|---|
| K1 | Mastering reference audio never plays: "A/B: listen to reference" only stores a setting. Level match and meters do work. [Compare in another player at the level-matched gain, §24 E.] The hint "Clip List › right-click › Use as reference" names a menu item that does not exist. |
| K2 | Master track: mute has no effect; all Master inserts run before the fader even if set Post; `mix.signal_flow` for Master lists stages Main does not have. |
| K3 | Monitoring modes Tracking and Mixing behave identically (only Hardware changes processing); "Low Latency Monitoring" does nothing. |
| K4 | SML Remote "Start with the app" does not start the service at launch. [Press Start.] |
| K5 | Pro Tools-style profile: Space starts playback but does not stop it. [Stop button / Transport window, or Studio One style.] |
| K6 | Pro Tools-style profile: F1 opens Command Help (not Shuffle); Ctrl+= only goes to Mix; Ctrl+Alt+L runs Move Selection Left (not Loop). |
| K7 | Studio One-style N (snap) and Alt+X (split) report errors; clip rating chords Ctrl+Alt+0–5 do nothing; the Nudge help mentions remapping `nudge.apply`, which has no row in the Keyboard Shortcuts window. |
| K8 | Punch recording with pre/post-roll can remove existing clips in the roll zones (code reading; not reproduced). [Duplicate the playlist first.] |
| K9 | The "SAFE OPEN" and "previous run did not close properly" status messages are overwritten when preferences load. Safe Open itself works. |
| K10 | Playback Engine: "Reconnect audio device" reopens the default output, not your choice; device/buffer/input choices are not remembered; no control sets a recording offset. |
| K11 | Send pan and send mute automation can be drawn but is not played back. |
| K12 | Clicking an empty snapshot slot stores the current mix into it. |
| K13 | `file.bounce_mix` ignores its documented `source` parameter; the Bounce Mix dialog has no dither, rate, tail, report or overwrite controls (use the command). |
| K14 | Recover Autosave: recovering a take shortens the capture file in place (the window says nothing is overwritten); only the newest generation per session is listed. [Copy the WAV first.] |
| K15 | Recording, Recover Autosave…, SML Remote, Assistant and Command Help have no menu-bar entry. [Search.] |
| K16 | Programmatic New/Open/Close discard unsaved changes without asking; Save/Save As replace existing files; stems and named exports replace files. |
| K17 | Interface scale and Reduce motion are not remembered after a restart. |
| K18 | With a mastering program file set, Meters and Level match measure the raw file without the Master chain. |
| K19 | Grid and Rel Grid snap a dragged clip the same way. |
| K20 | Long exports: offline renders stop at about 93 minutes of stereo at 48 kHz (untested beyond). |
| K21 | While a take is being recorded, Recover Autosave… can list that take as "unfinished"; recovering it would shorten the file being written. [Never use Recover Autosave… while recording.] |

## 27. Not available in this build

ASIO in the standard build (WASAPI on Windows); aggregate-device clock correction and measured
driver latency; QuickPunch, TrackPunch, Destructive Record/Punch; a separate cue/headphone mix;
native analog summing and tape models; built-in effects and instruments; the SML plugin family
(Channel EQ, Gate/Expander, Compressor, Optical Leveler, Tune, Vocal Editor, Align); ARA and
note-level pitch editing; Audio Unit editors and sidechains; in-app reference playback;
MP3/AAC export; user session templates in File › New; crash containment for plugins hosted
in-process (opt-in sandbox only); HTTPS, an installable app, a microphone and QR pairing for the
phone remote; free dictation, wake word and an always-listening assistant; a bundled language
model; keypad-vs-main-row key distinction and Windows-key chords. Help › Known Limits
(`help.limits`, via Search) lists the stated limits; a command that is not in
[COMMAND_REFERENCE.md](COMMAND_REFERENCE.md) does not exist in this build.

## 28. Section map from the previous edition

The 0.5.0 build `b641b5a` contains the previous edition
([manual_archive/USER_GUIDE-0.5.0-b641b5a.md](manual_archive/USER_GUIDE-0.5.0-b641b5a.md)); its
assistant cites those numbers.

| Previous § | Title | This edition |
|---|---|---|
| 1 | Recording dry vocals | §6 |
| 1a | Autosave and recovery | §23 |
| 1b | Plugin scanning (isolated) | §17.1 |
| 1c | Safe Open | §3.3 |
| 2 | Nudge | §12 |
| 3 | Vocal comping and editing walkthrough | §7, §24 A–B |
| 3a | Audio editing tools | §8, §10 |
| 3b | Shortcut profiles | §21 |
| 3c | Themes and layouts | §22 |
| 3d | SML Remote (phone browser) | §19 |
| 3e | Monitor volume, dim, mono and headphones | §4.4 |
| 4 | Buses and FX returns | §15 |
| 5 | Dedicated slots (Summing / Tape) | §16 |
| 6 | Mix profile, pan and width | §13.2–13.3 |
| 7 | Console, Visual and Split | §13 |
| 8 | Monitoring and exports | §4.5, §18.5 |
| 9 | Audio device, latency and tracking | §4 |
| 10 | Plugins: what to expect | §17 |
| 10a | Assistant (Explain, Operate, Engineer) | §20 |
| 10b | Mastering workspace and export | §18 |
| 11 | Not supported in this build | §27 |

Corrections to the previous edition made here: VST3 sidechain is routed (first aux bus); Bend
joins are crossfaded (480 samples); the Master layout is a working mastering workspace; Keyboard
Shortcuts is under Setup; Search is Window › Pro Tools Search; several windows are reached only
through Search; the reference never plays; dither defaults to on; monitoring modes, snapshot,
recovery and keyboard details as listed in §26.
