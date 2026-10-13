# SML Studio — interface polish pass 01

Date: 2026-10-13. Environment: **P (portable)** — a Linux x86_64 build container, Rust 1.97.0,
egui 0.36, offscreen wgpu renders. There was no Windows host, display, audio device or listener.
**Nothing here was observed on Windows**, and the installed candidate was not touched.

## Source identity

| | |
|---|---|
| Branch | `fable/recording-core` (not `main`; nothing pushed, merged or published) |
| Starting HEAD | `d0dc6d0` (Hotfix Pass 01 complete). Clean tree. HF01–HF06 are kept, including the approved New Tracks dialog. |
| Code commits | `2d28a76` tokens, control bar, mixer · `ecfa8fb` edit, record, master, assistant, Graphite · `d10b7a8` accessible strip state, bounce labels, tests · `59038e2` floating windows below the bars |
| Documentation | the commit that adds this file (style guide, manual, Grok guide, evidence) |
| Candidate | see *Candidate* below |

Time: work ran from 03:11 to 03:52 UTC (about 40 minutes of the three hours allowed). It stopped when the
listed areas were complete and tested; nothing was repeated to fill time.

## What changed, by workspace

### Shared design system (`theme.rs`, `widgets.rs`)
- **SML Studio Dark palette:**
  - charcoal surfaces stepped so panels, raised groups and the window are distinguishable;
  - a restrained blue accent;
  - brighter primary and secondary text.
- **New semantic tokens:** `surface_raised`, `focus`, `playback`, `warning`, `error`,
  `text_disabled`, `selection_stroke`, `loop_range`. Light and High contrast define them too.
  Older theme files load, with the new tokens taken from Dark.
- **Type scale and metrics:** `theme::size` (caption 10, small 11, body 13, title 14, numeric 13,
  large numeric 15) and `theme::metric` (control height 22 / 18, gap 6, group gap 12, radius 3),
  plus `group_frame()`.
- **Control states, in egui and in custom widgets:**
  - resting: hairline edge;
  - hovered: visible edge;
  - pressed or open: accent fill with a focus edge;
  - keyboard focus: a cyan ring (previously the accent);
  - disabled: faded, with `text_disabled`.
- **Shared widgets:**
  - The record button's icon now comes from tokens.
  - The fader scale is 9 points (was 8), with the unity mark highlighted.
  - The fader cap lights while it is dragged.
- **Graphite:** a built-in alternate dark palette with a steel-teal accent and Dark's state
  colours, selected through the existing theme mechanism (`ui.theme {"mode": "graphite"}`).

### Control bar and recording workspace
- **Segments:** controls sit in raised segments (transport, tempo, click, loop, recording status,
  audio device). Segments wrap whole on narrow windows.
- **Buttons:** Stop / Play / Record, Click, Count Off, Loop and Audio Setup are chips with icons.
  Play lights green and Record red. Accessible names are unchanged, so tests, the manual and Grok
  paths still hold.
- **New recording-status segment:**
  - shows "No track armed", "Armed: <track> · <input>" or "Armed: N tracks";
  - shows "I n" for tracks monitoring their input;
  - its tooltip lists the armed tracks, their inputs, the monitoring mode and the input device,
    or that the device "opens on the first record or arm";
  - clicking it opens Audio Setup on the armed track.
- **Device status mark:** a green dot means running, an amber ring means waiting, a red ✕ means no
  output. The shape differs, not only the colour.
- **Compact-window menu clipping, fixed:**
  - The session title shortens first.
  - If the layout switcher still does not fit, it moves into the control bar instead of covering
    "Window".
  - Floating windows (Assistant, Recording…) now stay below the bars, as Audio Setup did.
- **Record layout panel:** sections DEVICES, TRACK, MONITORING and TRANSPORT.
  - TRACK states the input the track records and offers Arm / Armed and Input monitor chips.
  - It gains an Audio Setup shortcut.
  - It says plainly that Tracking and Mixing currently process the same (known issue K3).
- **Nudge** stays its own toolbar group in the Edit toolbar (unchanged).

### Mixer
- **Strip order, top to bottom:**
  1. identity: colour bar, name, kind and state tags;
  2. processing: preamp, instrument, inserts, slots;
  3. sends and routing: sends, I/O, automation, group;
  4. pan and width (W moved out of SLOTS);
  5. fader and meter with the dB readout;
  6. channel actions: I / R, then S / M.
- **State tags:** REC, IN, SOLO, MUTE and **FAULT** (a missing plugin, or a crashed sandboxed one)
  appear as text tags, in addition to the coloured buttons. The tag row is accessible as
  "<track> state: …".
- **Selection:** the selected strip has an outline as well as the lighter surface, and its name
  field a selection-coloured edge.
- **Inserts:** hosted plugins show their names; previously a blank dot looked like an empty slot.
  Bypassed inserts read "byp …" with an amber edge. Missing ones read "! …" with a red edge and a
  tooltip.
- **Strip height:** strips size to the tallest content, so a fader never sits under the buttons on
  short screens; the console scrolls (HF01 kept).
- **Header:** segmented Console / Visual / Split tabs and "Snapshots" chips (stored slots solid,
  empty ones dim with a "click stores" tooltip), with tooltips on the bus buttons. The Split
  divider has a grip that lights on hover or drag. Independent pane scrolling and sizing are
  unchanged.

### Edit workspace
- **Playhead:** a dark underlay keeps it visible over bright clips, and a head marks it on the
  rulers.
- **Selected range:** crisp `selection_stroke` edges at start and end.
- **Selected clips:** a 1.5-point outline.
- **Muted clips:** "muted ·" in the name bar and a `text_disabled` waveform.
- **Loop lane:** uses the `loop_range` token. Clip borders and name text come from tokens.
- **Unchanged:** `x_of` / `sample_at`, hit testing, selection rules and tool gestures. The edit,
  clip-menu and Nudge test suites pass.

### Dialogs, browsers, mastering, assistant
- **Audio Setup:** DEVICES and SELECTED TRACK INPUT captions; its warnings use the `error` token.
- **New Tracks:** unchanged layout (approved design, full-width Name row); it inherits the shared
  control states.
- **Bounce Mix:** Format and Bit Depth labels now sit before their combo boxes.
- **Plugin chooser:** vendor submenus show counts ("Vendor (n)") and each plugin's id on hover,
  from the list already in hand. Nothing is scanned or loaded to show it.
- **Master workspace:** three cards.
  - PROGRAM AND REFERENCE says that A/B stores the setting only and plays no audio (K1).
  - METERS shows a right-aligned monospaced table with units and "Not measured yet". No new
    meter DSP.
  - EXPORT has a **Bounce Mix…** button that opens the existing dialog.
- **Assistant:** the first line, "ANSWERS FROM", reads "grounded rules — no local model
  configured" or names the configured local model and its fallback. Push-to-talk is "enrolled
  phrases only".

Not changed: audio algorithms, routing, command ids, shortcut mappings, session format,
automation, plugin state, dry recording, third-party plugin editors.

## Tests and checks

| Check | Result |
|---|---|
| `tests/interface_polish.rs` (new, 5 tests) | pass. Covers: the switcher never covers "Window" at 700, 911, 960@200 %, 1366 and 1920 points in all four layouts, and still switches; strip order (name above I/O above S/M) with text state tags, name-click selection unchanged; missing insert → FAULT; recording status opens Audio Setup on the armed track without starting the transport; Dark, Graphite, Light and High contrast validate with distinct state colours. |
| `tests/shared_controls.rs` | one expectation updated: the keyboard focus ring is drawn in the new `focus` token instead of `accent`. Still asserts a ring on the same rectangle. |
| All `soundcraft-ui-egui` tests (HF suites, edit, clip menu, docking, shortcuts, layouts…) | pass after each batch |
| Theme unit tests | pass (token count 88; Graphite validated) |
| Full gate `cargo xtask ci` on `59038e2` | **pass**: fmt, clippy, test (731 passed, 0 failed), assets, layers, docs, wasm — `evidence/ci.log`. The documentation and evidence in the following commit were checked again with `cargo xtask docs-check` and `cargo xtask assets`. |

## Evidence (`docs/polish_pass_01/evidence/`)

- `compare-*.png`: before (`d0dc6d0`) and after, stacked, for 12 views. They cover Edit, Record,
  Mix Console / Visual / Split, Master, Audio Setup, New Tracks, Assistant and Bounce, at
  1366×768, 911×512 (1366×768 at 150 %), 960×540 at 200 % and 1920×1080.
- `after/`: all 29 renders of the matrix (Record / Edit / Mix Console, Visual, Split / Master at
  four sizes, plus dialogs). Images wider than 1920 px are halved.
- `render-matrix.sh` and `render-scripts/`: how to reproduce them.
- `ci.log`: the gate.

These are headless renders of the real application. They show layout, contrast and clipping, not
native DPI, font hinting, pointer feel or device behaviour.

## Gaps and items for native review

1. **Native Windows review**, all workspaces:
   - the owner's display scale and 1366×768 at 150 %;
   - readability of the 9–10 point captions and tags;
   - focus rings with the keyboard;
   - Graphite in a dim room.
2. **Record layout at compact widths:** the Edit toolbar beside the RECORD panel scrolls
   horizontally, as before this pass. Tools and Nudge are reachable by scrolling, not all visible
   at once.
3. **Visual mixer canvas:** overlapping node labels when channels share a position (seen in
   renders, pre-existing). Not changed.
4. **Input meters** (Record panel, Audio Setup) use egui's progress bar, which draws a small dot
   even at zero. The text says "-inf" / "input: no signal". Cosmetic.
5. **Not converted to tokens:**
   - the MIDI editor, score editor, video windows and the tempo/key ruler editors still contain
     literal colours (34 in `edit_window.rs`, mostly rulers and MIDI notes);
   - the Visual canvas keeps its own palette.

   These areas were not restyled in this pass.
6. **Mixer captions:** strip section captions remain 10 points, and fader scale 9, to fit 98-point
   strips. Larger type would need wider strips; that is a layout decision for the owner.

## Candidate (staged, not installed)

Built with `OUT=dist/polish01-windows bash packaging/windows/cross-build.sh`. This is the GNU
cross-build (`x86_64-pc-windows-gnu`, rustc 1.97.0, release, `--locked`): WASAPI only, unsigned,
no installer, with the documentation bundle at revision 2026-10-13.3. Source `4154aa8` (clean
tree). Program code is identical to `59038e2`; the compiled-in manual is the one at `4154aa8`.

| Artifact | SHA-256 |
|---|---|
| `soundcraft-0.5.0-windows-x64-gnu-portable.zip` | `f8f72c8e5a986fd92127c5c6fd36f72e3dd919c27b826cef69ddd8c107222daf` |
| `soundcraft.exe` (inside) | `05779db859a5ad86c25739de3c23efffcda0e5522caef1ea97d8f2f3f0aab726` |
| `soundcraft-cli.exe` (inside) | `9b419bde19f69b0dad1c4339a7adea43056521e3b95b6b9d7742d7eb966e4891` |
| `MANIFEST.json` (beside the zip) | `015bd85b2ad657b42a4a04d8d359cd5b70358ae09cf96752b73130638a6e9310` |

**Not installed.** The installed Hotfix Pass 01 candidate (`soundcraft.exe` `192521f6…`) was not
touched, as asked. To try this build side by side, extract the zip into a new folder and run
`soundcraft.exe` from there. It uses the same settings folder (`%APPDATA%\SoundCraft`): the
Appearance choice and the new Split/theme preferences are shared, and older builds ignore the new
fields. To replace the installed candidate later (only with your go-ahead), use
`packaging/windows/Update-SMLStudioCandidate.ps1` with this zip; it keeps a timestamped backup and
supports `-Rollback`.
