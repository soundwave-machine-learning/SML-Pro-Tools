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
- SML Recording ships **no bundled effects or instruments**. Use your installed VST3 (or CLAP;
  AU on macOS) plugins. Plugins are scanned in the background at startup (never while recording);
  menus show progress and offer Cancel / Scan now.
- Plugins run **inside the SML Recording process**. A crashing plugin can take the application
  down; there is no sandbox. Your recordings are protected by the on-disk capture journal (the
  take is recovered up to its last durable checkpoint), but save often when trying new plugins.
- Old sessions that used the removed built-in processors open with those inserts shown as
  **missing**: their settings are kept, audio passes through them unchanged, and exports ask you to
  acknowledge that their sound is not included.
- Not yet validated with your own plugins (EQ, dynamics, pitch correction): see
  `TARGET_MACHINE_GAUNTLET.md` §5.

## 11. Not supported in this build
ASIO (WASAPI/ALSA/CoreAudio via the default hosts only), aggregate-device clock correction, ARA /
note-level pitch editing, native analog summing/tape models (deferred), VCAs beyond the existing
basic VCA tracks, monitor-only dim/mono/cue systems, detailed tape-machine UI, keypad-vs-main-row
key distinction and Win-key nudge chords (toolkit limitation).
