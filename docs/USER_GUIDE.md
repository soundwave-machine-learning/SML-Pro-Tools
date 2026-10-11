# SML Recording — user guide

Working draft written during the four-sprint build. Sections describe what the software does
today; anything not yet validated on the target machine is marked.

## 1. Recording dry vocals
- Record-enable a track (red button), press Record. Each take is captured **dry** — exactly the
  samples your interface delivers, before any clip processing, insert, slot or fader — straight
  to a new 32-bit float WAV beside the session (`Audio Files/`), or to the system temp folder for
  an unsaved session. Takes never overwrite earlier files.
- If the disk cannot keep up or fails, recording stops and the status bar says so; every frame
  written before the problem is kept. After a crash, the take file can be recovered to the last
  durable checkpoint (at most about one second is lost).
- Device/rate/buffer: the engine can open an exact device/rate/buffer and reports an error
  instead of silently using another device. The device picker in the UI does not yet use this
  path (it opens the system default input). ASIO is not compiled in this build.
- *Not yet validated on a real Windows interface (H1).*

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
