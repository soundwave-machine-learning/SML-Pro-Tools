# SML Recording — user guide

Working draft written during the four-sprint build. Sections describe what the software does
today; anything not yet validated on the target machine is marked.

## 1. Recording dry vocals
- Record-enable a track (red button), press Record. Each take is captured **dry** — exactly the
  samples your interface delivers, before any clip processing, insert, slot or fader — straight
  to a new 32-bit float WAV beside the session (`Audio Files/`), or to the system temp folder for
  an unsaved session. Takes never overwrite earlier files.
- If the disk cannot keep up or fails, or the input interface disappears (unplugged, driver
  error), recording stops and the status bar says so; every frame captured before the problem
  is kept. After an interface loss, press Record again once it is back — the input is reopened
  (a specifically chosen interface is never replaced by another one).
- If the **output** device disappears, playback (and a running recording) stops with a message;
  the take is kept. Reopen the device in Setup › Playback Engine. After a crash, the take file can be recovered to the last
  durable checkpoint (at most about one second is lost).
- Device/rate/buffer: choose the input interface in **Setup › Playback Engine** (*Scan input
  devices*, §9). The exact device/rate/buffer is opened, or you get an error — another device is
  never used silently. ASIO is not compiled in this build.
- Input channels: a track records the input its *Input* selector names ("In 3", "In 1-2"). If
  the input device lacks that channel, Record refuses to start and says which track and channel;
  it never records (or monitors) a different input instead. A one-input device can feed a stereo track (the
  same signal on both sides).
- **Not implemented in this build:** QuickPunch, TrackPunch, Destructive Record and
  DestructivePunch. Their Options entries are present but disabled and say so on hover; they
  change nothing. Recording is always non-destructive (every take is a new file). Punch-in with
  a selection, pre/post-roll, loop recording and Count Off are the recording modes that exist.
- **Count Off** (toolbar, beside Meter/Tempo; bars in Setup › Click/Countoff): Record starts the
  transport that many whole bars before the record point (selection start, or the playhead)
  and the click sounds during those bars even when the click is off; the take starts exactly
  at the record point (the count-off audio stays in the file, outside the clip). Not applied
  to loop recording or to punching in while already playing.
- If the input device runs at a different rate than the session, the take is resampled to the
  session rate and the status line says so (the capture file keeps the device rate). For
  bit-exact dry takes, open the input at the session rate (§9).
- *Not yet validated on a real Windows interface (H1).*

## 1a. Autosave and recovery
- Every 60 s a modified session is snapshotted into a rolling store of recovery generations
  (default 20, 256 MiB budget) under the settings folder's `Autosave/<session>/`. Each generation
  is written to a temporary file, flushed, re-read and validated, renamed into place, and only
  then published in the store's index; the previous generation is never touched before the new
  one is committed. Writing happens on a worker thread — recording never waits for it.
- Hosted plugin state is captured into the snapshot only while the transport is stopped; the
  Recording window and `File › Recover Autosave…` show how old that capture is.
- `File › Recover Autosave…` lists every recovery point (session, generation, time) and every
  unfinished take found beside the session or in the temp capture folder. Opening a recovery
  point gives an unsaved session (choose File › Save As); recovering a take keeps every
  journaled frame and reports what was after the last durable checkpoint. Nothing in that
  window deletes or overwrites a file.
- If the program did not close properly, the next start says so in the status bar.
- Limits: an abrupt process kill is the tested failure model; after a power loss the newest
  generation may be missing but an older committed one survives. The explicit File › Save is
  separate from autosave.

## 1b. Plugin scanning (isolated)
- At start the app scans for CLAP/VST3 plugins in separate worker processes, one bundle at a
  time. A plugin that crashes or hangs (20 s) while loading is **quarantined** with the reason and
  skipped on later scans; it never takes the app down. Results are cached by file size and
  modification time, so only new or changed plugins are scanned again. Rescan and re-enable are
  in Setup › Plugins (quarantine list). Hosting for playback still runs the plugin inside the
  app (see §10 for the risks); only scanning is isolated.

## 1c. Safe Open
- If the app will not start or a plugin misbehaves: start it with `--safe`. Hosted plugins are
  not loaded or scanned for that run; their slots pass audio through and keep their state and
  settings, so the session is unchanged — restart without `--safe` to load them again.
- `--reset-prefs` additionally sets the UI/theme preferences aside as a timestamped backup
  (`ui.json.backup-<time>` in the settings folder); rename it back to restore.
- After an unclean exit the status bar suggests `--safe`.

## 2. Nudge
The **NUDGE** group in the edit toolbar is separate from Grid:
- value + unit (samples, ms, s, ticks, timecode frames), favourites **1 smp / 1 ms / 10 ms**;
- target: Selected clips, Clip contents, Left/Right trim edge, Selection/cursor only,
  Selection start/end; ◀ ▶ arrows nudge with the panel target;
- readout of the exact displacement in samples at the session rate (e.g. 10 ms = 480 samples at
  48 kHz).

Keyboard (SML reference preset, Windows names): `Num+/Num-` move selected clips (cursor when
none); `Alt+Num±` trim start; `Ctrl+Num±` trim end; `Alt+Shift+Num±` selection start;
`Ctrl+Shift+Num±` selection end; `Ctrl+Alt+Num±` next/previous nudge value. `Shift+Num±` is
deliberately unassigned. Content nudge (Win+Num±) is not available from the keyboard in this
build because the UI toolkit does not report the Windows key — use the panel target. The UI
toolkit also cannot yet tell keypad +/- from the main-row keys; nudge keys never act while a
text field, dialog or plugin window has focus. Comma/period nudge only with Edit Keyboard Focus.

Precision: steps accumulate from a logical anchor, so ten 1 ms steps at 44.1 kHz move exactly
441 samples and going back the same number of steps returns exactly to the start. Below-sample
values may move nothing until the accumulated amount crosses a sample (the command reports
this). A nudge that would move a locked clip, go before the session start, run out of source
handles or make a zero-length clip is rejected as a whole — nothing moves. Undo restores both
positions and the accumulation state.

## 3. Vocal comping and editing walkthrough
1. Record three takes (loop recording puts each pass on its own playlist, all at the loop start).
2. Promote phrases from alternate playlists into the main one (`track.playlist_promote` — the
   phrase keeps its original timing; alternates are preserved).
3. Clear breaths or ride them with clip gain (source stage, independent of the fader).
4. Create a crossfade at a join (equal-power available).
5. Nudge a double by 10 ms, compare, undo.
6. Group lead and double clips to move them together.
7. Save and reopen: comp, playlists and positions come back identically; source files are never
   modified by editing.

Pitch correction uses your own VST3 pitch plugin (not yet validated with a named plugin).

## 3a. Audio editing tools (Arrow, Range, Split, Eraser, Paint, Mute, Bend, Listen)
The Edit toolbar's AUDIO TOOLS group holds the eight tools in that order (labelled buttons; keys
1–8 under the Studio One-style shortcut profile). Every tool runs a shared command, so the same
edit is available from search, the CLI and the remote, and one gesture is one undo step.
- **Arrow**: select and move clips; the clip edges trim, the top corners drag fades, the gain
  handle rides clip gain. With **Link** on, the upper half of a tall track selects a time range
  instead (compact tracks keep Range on its own button). **Alt** assigns an alternate tool that
  applies while Ctrl (Cmd on macOS) is held with Arrow active; releasing the key or losing focus
  drops it.
- **Range**: drag a time selection across one or more tracks.
- **Split**: a preview line and time readout follow the pointer; click splits the clip under it
  (the whole clip selection when the clicked clip is part of it). A click on a boundary makes
  no zero-length piece.
- **Eraser**: click or sweep to remove clip instances; media files are never touched. Shuffle
  mode ripples, every other mode does not. Escape during a sweep cancels it.
- **Paint**: draws the chosen shape (Freehand, Line, Parabola, Square, Triangle, Saw, Sine,
  Transform; cycles for the periodic ones) into the shown automation lane, or into the clip-gain
  envelope of the clip under the pointer when the track shows its waveform. Mixer automation and
  clip gain are separate data; Paint never writes one when you aim at the other. Transform
  drags a box and opens numeric time/value shift and scale controls.
- **Mute**: click or sweep to mute/unmute clips (clip mute, not channel mute). A sweep takes
  its target state from the first clip and touches each clip once.
- **Bend**: click inside an audio clip to add a warp anchor, drag the anchor to move that moment
  in time; Alt-click removes it. The clip keeps playing its last render until the new
  pitch-preserving render (in-house WSOLA, off the UI and audio threads) is ready; "rendering…"
  and any error show on the clip. Limits: anchors at least 64 samples apart, each segment
  stretched by 0.25–4×, segments are joined with a 10 ms crossfade at each anchor (an anchor inside a
  sustained note no longer clicks; where the two sides differ in phase the level can dip
  briefly), stereo channels share one map. **Remove Warp** (Clip menu) restores the
  original material; the original file is never modified.
- **Listen**: press and hold to audition the track from the pointer; release, Escape or focus
  loss stops it and restores the cursor (unless another command moved it meanwhile). Listen is
  refused while the transport runs or records, never stops a take, changes no mute/solo/arm
  flag, writes no automation and is never saved. Explicit mutes still win.

## 3b. Shortcut profiles
Window › Keyboard Shortcuts: choose **Pro Tools style** (the registry's chords, F1–F10 modes
and tools, Enter marker, single-key Commands focus), **Studio One style** (1–8 tools, D
duplicate, E/W zoom, N snap, Alt+arrows nudge, M/S/R track states…) or **Custom** (a base
profile plus your overrides). The switch is explicit and reversible; conflicting maps are never
merged. Click a chord cell, press the new chord to reassign (this moves you to Custom); Unbind,
Reset, Import… and Export… (JSON) are in the window; the conflict inspector lists duplicate
chords, chords the OS reserves (shown with the alternate) and chords this build cannot deliver
(keypad location and the Windows key — a toolkit limitation; the Nudge panel and search always
work). Search (Window › Search) and Command Help show the binding of the active profile. The
Studio One chords are best-known defaults: the vendor manual was not reachable from the build
environment, so they are listed as "unverified" until checked on your machine.

## 3c. Themes and layouts
Appearance (Window › UI Customization) offers Dark, Light, High contrast, System and Custom.
Themes are data: a JSON file with colour tokens, a base and a typography scale; the Theme Editor
previews live, Import…/Export… move files, Reset returns to Dark. A theme must keep text
readable (3:1 contrast on every surface) and record/solo/mute/clip states distinguishable, or it
is refused with the reason; a broken saved theme never stops launch (you get Dark and a notice).
Record, clip and overload states are also shapes/labels, not colour alone. The menu bar's
Record · Edit · Mix · Master switch the four layouts of one application; Record docks the
recording essentials beside the timeline, Master is a navigation shell until Sprint 4 adds
mastering (it says so on screen). Switching layouts changes nothing in the session.

## 3d. SML Remote (phone browser)
Window › SML Remote: choose the interface (127.0.0.1 = this computer only; 0.0.0.0:port = your
LAN), Start, then Open pairing: type the address into the phone's browser and enter the 6-digit
code within two minutes. The phone gets transport, pre-roll/count-in, markers, loop, track
select/arm/monitor/fader/pan/mute/solo, take rating, monitor dim/mono/volume and meters. The
desktop stays authoritative: a receipt means "accepted", not "happened at this sample". A
reconnect reloads the state and can never replay an old Record; disconnecting the phone leaves a
recording running; two phones cannot overwrite each other's stale edits (the later one is told
to refresh). Revoke any phone from the window. Limits: plain HTTP only — no installable app,
no microphone, no encryption; no QR image (type the address). Nothing is exposed beyond the
interface you bind, and the service never starts unless you start it (or tick "Start with the
app").

## 3e. Monitor volume, dim, mono and headphones
Window › Recording (and the Record layout) has Monitor volume, Dim (−20 dB) and Mono. They act on
the speaker output only, after the mix, and never reach a bounce or export. "Headphones" appears
only when I/O Setup has a dedicated cue path; without one it is shown unavailable — the Main
fader is never relabelled as headphones. Hardware (direct) monitoring is labelled separately
from software monitoring.

## 4. Buses and FX returns
- **Create Bus from Selection** (Mix toolbar, or command search): select Lead, Double L, Double R
  → Create Bus from Selection → name it "Vocal Bus". One step creates the bus channel, routes the
  selected tracks into it (their faders, pans, inserts and sends are kept) and selects it; one
  Undo removes all of it. If the tracks fed different outputs you are asked for a destination.
  Selecting a bus together with one of its own sources is refused (it would double the signal).
- **Create FX Return**: adds a return channel plus a send on each selected track (post-fader,
  starting at -∞; the dry outputs are unchanged). An empty return only duplicates the dry signal
  once you raise the sends — load your own reverb on it.
- A bus is a real summing channel (inserts, fader, mute/solo, sends). Organizational folders and
  VCAs are not buses. Deleting a bus reroutes its sources to the bus's own output and removes or
  reassigns sends that targeted it; nothing is deleted from your media.
- **Spill** (`bus.spill`): shows the bus and every channel that feeds it (direct or by send),
  hiding the rest of the mixer view; exiting restores your previous visibility. View only.
- Mute/solo (new sessions): a muted channel still feeds its *pre-fader* sends (mute the send
  itself to stop them); soloing a track keeps its buses/returns audible without letting other,
  unrelated tracks into a shared reverb; soloing a bus keeps the tracks feeding it; solo-safe
  channels stay audible.

## 5. Dedicated slots (Summing / Tape)
- Each audio track has a **Tape** slot (before your inserts); each bus and Main have a **Summing**
  slot (on the summed signal) and a **Master Tape** slot (Post — after the fader — by default on
  buses, Pre on Main so a limiter can follow).
- Empty slots are exact pass-through ("Clean"/"None"). You can load your own installed VST3/CLAP
  plugin into a slot; it processes the one bus/track signal it sits on — a plugin in a bus's
  Summing slot does not see the individual tracks.
- Slot bypass bypasses the plugin and its trim but keeps its delay, so timing stays aligned.
  Change a slot's plugin or Pre/Post position while stopped.
- During playback each slot row shows two thin meters at its right edge — slot **input** and
  **output** — and its tooltip gives both in dBFS, so you can gain-stage into and out of a slot
  plugin. No meters are drawn when the slot is empty-and-unconfigured or deactivated for tracking.
- Native analog summing and tape models are **not included yet** (deferred to a later pass); no
  drive/noise/bias controls are shown because none exist.
- Audition bypass (`mix.audition_bypass`) temporarily silences all Summing, Tape and/or insert
  processing for comparison. It is never saved and never affects exports.

## 6. Mix profile, pan and width
- Standard profile keeps the existing pan law. **Logic-style** profile (`mix.profile`) uses SML's
  published laws (0 dB, -3 dB [default], -3 dB compensated), mono Pan, stereo Balance (no
  crossfeed) or dual stereo Pan. These are SML's own curves, not a copy of another product.
- New buses are neutral (unity, centred, 100 % width) so nested buses never add pan-law gain.
- **Stereo Width** (Console "W" row or the Visual node handles — the same control): 0 % = mono,
  100 % = untouched, 200 % = double side. Width is placed after inserts and before the sends/fader.
- New sessions sum buses and Main in 64-bit floating point; older sessions keep their original
  arithmetic until you choose `mix.graph_migrate`.

## 7. Console, Visual and Split
The Mix window header switches **Console** (channel strips), **Visual** (soundstage: left/right =
pan, up/down = fader, side handles = width) and **Split** (both). Both views edit the same mixer;
opening or closing a view never changes the sound. Drag a node (Shift locks an axis, Alt is fine
control, Esc cancels, double-click resets to 0 dB/centre). The channel list beside the canvas
selects, hides (view only) and edits values numerically; double-click a name to rename the
channel. Under each name a badge shows where it goes (e.g. "→ Vocal Bus +1 send"; muted sends are
not counted) and a red **CLIP** tag appears while its meter shows a clip. Snapshots **A/B/C** store gain, pan,
width, mute, solo and visibility (click to recall while stopped, Shift+click to save). Plugins,
slots, routing, trims and automation are not part of snapshots.

## 8. Monitoring and exports
- Monitoring modes (`mix.monitor_mode`, change while stopped): **Tracking** (small buffer; see
  `mix.latency_report` for latency contributors), **Mixing** (full processing and compensation),
  **Hardware Monitor** (software input monitoring off; use your interface's direct monitoring —
  plugins are not on that path). Recording is always dry.
- Named exports (`file.export_path`): **Raw Source** (the original media), **Edited Dry** (comp,
  clip edits, clip gain and fades only), **Processed Track** (the channel's own final output incl.
  Tape, inserts, width, fader and pan, before any bus), **Full Mix** (Main after everything, normal
  saved state). Summing processed stems is not guaranteed to recreate a mix with shared or
  nonlinear processing.
- Exports refuse to run while a missing plugin would be silently skipped, unless you acknowledge it.

## 9. Audio device, latency and tracking
- **Setup › Playback Engine**: *Scan output devices*, pick host/device and buffer (32–1024 or
  default) and *Apply* while stopped. If the device cannot run exactly those settings you get an
  error and the previous device stays open — nothing is substituted silently. The window shows the
  requested buffer, the range the driver reports, the callback size actually delivered and the
  stream clock. Clock lock and driver latency show *Unknown* unless measured.
- **Recording input**: in the same window, *Scan input devices* and pick your interface. It opens
  immediately (stopped only) at the session rate and the buffer chosen above, with every input
  channel the device offers; if it cannot, you get an error and the current input stays as it was.
  "default input" uses the system default device.
- **Take placement**: when the audio backend provides stream timestamps, each take is placed from
  them (playback presentation time vs. capture time) plus any measured residual / manual offset
  (`record.offset_samples`); otherwise from the transport position. The recorded file never changes.
- **Plugin latency changed during a take** (e.g. you switched a plugin to a lookahead mode):
  recording stops with a message; everything captured so far is kept; press Record to continue.
- **Tracking deactivation** (`mix.tracking_deactivate`, stopped): takes latent plugins out of the
  live monitoring path to reduce delay; `mix.tracking_restore` brings them back. This is temporary,
  never saved, and exports always use your normal mix.
- **Post-fader inserts**: right-click an insert › *Post-fader* (stopped) to run it after the fader.

## 10. Plugins: what to expect

- **Formats.** CLAP and VST3 are hosted in-process; Audio Units on macOS. Nothing is bundled:
  the plugin list is whatever the isolated scanner found (§1b). `engine.plugin_capabilities`
  shows, per installed plugin, what this host actually read from a live instance: audio
  ports/buses, sidechain inputs, note input, parameter count and units, editor availability on
  this platform, state support, declared latency and tail. That record proves the interface
  as hosted here, never the plugin's sound.
- **Sidechain.** An insert can take a bus as its external sidechain (`mix.insert_sidechain
  {track, slot, bus}`; send the key signal to that bus). On aux and master strips the detector
  is sample-accurate; on audio/instrument tracks it hears the bus one audio block late (the bus
  is summed in the same pass) — `mix.latency_report` lists every sidechain with its lag. CLAP
  plugins get the bus on their first aux input port; VST3 aux buses are not activated yet
  (recorded in the capability matrix as "not routed").
- **Bypass keeps timing.** Bypassing an insert keeps its latency as a plain delay, so the mix
  stays aligned (the same rule as the dedicated slots).
- **Versions.** The plugin version at insert time is saved with the session;
  `session.plugin_versions` reports `same`, `changed`, `unrecorded` or `missing` per slot.
- **Exports.** `file.bounce_mix` / `file.bounce_stems` refuse when a plugin is missing *or
  failed to load* (the export would not be the mix you hear) unless you pass
  `acknowledge_missing: true`. `tail: seconds | "auto"` renders past the range end so reverbs
  and delays are not cut (`auto` = the longest tail any processor declares).
- **Sandboxed mode (opt-in).** Insert `sandbox:<plugin id>` instead of `<plugin id>` to run
  that plugin in a worker process. It adds exactly one audio block of latency (compensated),
  has no native editor (generic parameters only), and contains crashes and hangs: the slot
  goes silent for the current block, the fault is listed by `engine.host_faults`, and the
  worker is restarted with its last captured state — except while recording, when it stays
  silent and flagged (a crashed processor is never resumed into a take unnoticed). In-process
  plugins have no such containment. In an offline bounce a sandboxed instance waits for its
  worker (up to 3 s per block) instead of substituting silence, so an export is never a
  degraded result; a worker that stays silent faults the export. Measured in
  `docs/evidence/next_cycle/sprint_03/`.
- **Editors.** CLAP editors open as floating windows; VST3 editors are hosted on macOS only
  (other platforms report "unsupported"). Sandboxed instances have no editor.
- **Not hosted.** VST3 64-bit processing and host-created IMessage; CLAP tail extension (tail
  reported as 0); AU capabilities are not probed. The seven SML plugins (Channel EQ,
  Gate/Expander, Compressor, Optical Leveler, Tune, Vocal Editor, Align) are separate projects:
  until an artifact is supplied they are AWAITING_EXTERNAL_PLUGIN in the matrix, and the Vocal
  Editor's integration mechanism (insert vs clip-linked) is undetermined.

## 10a. Assistant (Explain, Operate, Engineer)

- **Where.** Help › Assistant (desktop) and the phone page's Ask box. By default answers come
  from grounded rules over the command registry, this guide and measurements, and cite the
  command ids / guide sections they rest on (`assistant.provider` reports what is on).
- **Local language model (optional, desktop).** SoundCraft ships no model. If you run a local
  OpenAI-compatible server (llama.cpp `llama-server`, Ollama, LM Studio), turn it on under
  *Local language model* with its address; only `127.0.0.1`, `::1` or `localhost` are accepted,
  so nothing leaves the machine. The model receives the session's track list and the command
  help for your question, answers in its own words and may only *propose* mute, solo, volume,
  pan, record-arm and rename actions; each proposal is checked against the current tracks and
  ranges and shown as a plan you run yourself. A slow answer shows its elapsed time and can be
  cancelled; if the server is down or times out, the grounded rules answer and say why.
  Unsupported features and destructive requests are still refused by the rules.
- **Explain** names the command, its menu location, the active profile's shortcut, parameters,
  status (including DISABLED rows) and known limits. A tool that is not in this build is
  reported as unavailable (ARA, wake word, QuickPunch, the SML plugins until supplied).
- **Operate** turns "mute Kick", "set the Vox fader to −6 dB", "rename Lead to Lead Vox" into a
  previewed plan of validated commands at the current revision. *Run plan* executes it as one
  undo step; if the document changed meanwhile, or any step is rejected, nothing is applied.
  Two tracks with the same name make it ask which; a missing track is refused; deleting files,
  sending, publishing or purchasing need your own authorization (the assistant never plans
  them). Marker, clip and transcript text is quoted as data, never followed as instructions.
- **Engineer** measures the selected range (sample peak, 4× true peak, integrated loudness,
  noise floor, full-scale samples) and labels every statement measured / inference /
  preference: clipping locations, a gate threshold from the noise floor (using only a plugin
  you have registered), a vocal-bus plan, a level balance between two tracks with a fader
  proposal to preview and undo. It never claims to have heard material it did not measure.
- **Speech (desktop).** *Hold to talk* records from the default input while held and recognises
  one of the phrases you enrolled under *Voice* (record each phrase once or more, in the room
  and with the microphone you will use). It is a command recogniser, not dictation: anything
  not close to an enrolled phrase is rejected and nothing happens. A recognised phrase appears
  as 🎤 text and is answered exactly like typed text (plans still need *Run plan*). It needs a
  quiet room: on synthetic test speech it was reliable at 30 dB signal-to-noise and failed at
  10 dB. Push-to-talk is off while recording. *Speak replies* reads the first sentence of each
  answer in a built-in robotic voice on the monitor output only (never a bounce or a take),
  silent while recording; *Stop speaking* cancels it. Enrolled phrases are stored as features
  in `assistant.json`, never as audio. The phone page is typed only (a phone microphone needs
  an HTTPS page). There is no always-listening mode.

## 10b. Mastering workspace and export

- **Layout.** The Master layout shows the real Master strip: its user inserts are the mastering
  chain, applied exactly once (the export path is the main mix). `mastering.program` can set an
  imported stereo file as the program instead of the mix.
- **Reference.** `mastering.reference {source}` plays a monitor-only reference; *A/B* mutes the
  program on the speakers (not in any export); *Level match* measures both integrated
  loudnesses and sets an audition gain. Nothing of the reference can reach a bounce.
- **Meters.** *Measure* runs the standard meters offline over the program: sample peak, true
  peak (BS.1770-4, 4×), momentary/short-term maxima, integrated (−70 absolute / −10 relative
  gate), loudness range (EBU Tech 3342), correlation and mono-sum peak. They are checked against
  the analytic EBU Tech 3341/3342 signals (`e07-meters.csv`); that is conformance evidence for
  those cases, not a certification.
- **Export.** File › Bounce Mix… writes exactly the settings you choose: format, bit depth
  (TPDF dither only on integer depths and only when chosen), sample rate (windowed-sinc
  conversion when it differs), tail, metadata; it refuses an existing file unless `overwrite`,
  a missing or failed plugin unless acknowledged, and adds no limiter or normaliser of its own.
  `report: true` writes `<file>.report.json` with the settings, revision, plugin ids/versions,
  measured levels and the decoded-file verification (frames, rate, channels, content hash).

## 11. Not supported in this build
ASIO (WASAPI/ALSA/CoreAudio via the default hosts only), aggregate-device clock correction, ARA /
note-level pitch editing, native analog summing/tape models (deferred), VCAs beyond the existing
basic VCA tracks, a separate cue/headphone mix (a cue path is detected but not mixed), detailed
tape-machine UI, keypad-vs-main-row key distinction and Win-key nudge chords (toolkit
limitation), HTTPS/PWA/microphone for the phone remote, QR pairing images, runtime crash
containment of hosted plugins (scanning is isolated; see E03), Bend crossfades at anchors.
Window › Help › Command Help lists every command this build has with its limits; a tool that is
not listed does not exist here.
