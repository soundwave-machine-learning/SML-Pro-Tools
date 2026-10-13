# SML Studio interface style guide

For contributors changing the interface. It describes the rules the code follows today. The
source of truth for every value is `crates/ui-egui/src/theme.rs`. When a rule here and the code
disagree, fix one of them in the same change.

## 1. Principles

- **One look, one set of tokens.** Colours come from `Tokens` (the active palette), sizes from
  `theme::size`, control metrics from `theme::metric`. Do not write a colour literal in
  workspace code. If you need a new meaning, add a token (§6).
- **State never depends on colour alone.** Every state has a second cue:
  - text (REC, MUTE, "muted ·", "byp", "!");
  - shape (filled dot, ring or cross; an edge bar on inserts);
  - fill change (neutral to solid).
- **Honest values.** Show only what the session, the open devices or a measurement report. When
  something is not known, say so: "Unknown", "Not measured", "no output stream", "opens on the
  first record or arm". Never show a placeholder meter or a pretend device.
- **Restraint.** Charcoal surfaces, one accent, no glow, gradients or textures, and nothing
  drawn over waveforms or meters except their own markers.
- **Critical controls are named.** Every button has an accessible name (`WidgetInfo`) and a
  tooltip. Accessible names are part of the interface contract: tests, the manual and the
  Grok guide address controls by them. Change one only together with those.
- **Presentation stays off the audio path.** Painting reads state; it never processes audio,
  scans plugins or loads anything. Per-frame work is a few rectangles and text runs, with no
  textures and no animation loops.

## 2. Semantic colours

Values are for **SML Studio Dark** (`Tokens::DARK`, the default). **Graphite**
(`Tokens::GRAPHITE`) lifts the surfaces and uses a steel-teal accent; it keeps Dark's state
colours. **Light** and **High contrast** define every token too.

| Token | Dark (RGB) | Use for | Not for |
|---|---|---|---|
| `window_bg` | 24 25 27 | the window behind everything | |
| `panel_bg` | 31 32 35 | side panels, cards | |
| `panel_bg2` | 38 40 43 | floating windows | |
| `surface_raised` | 36 38 42 | control-bar segments, grouped controls | large areas |
| `toolbar_bg` / `menu_bar` | 27 28 31 / 19 20 22 | bars | |
| `border` | 11 12 13 | hairlines between surfaces, control outlines | text |
| `border_light` | 58 61 66 | hovered edges, separators, window outlines | resting controls |
| `text` | 224 226 230 | primary text and values | |
| `text_dim` | 158 163 171 | secondary text, units, hints | values the user must read precisely |
| `text_disabled` | 104 108 116 | disabled labels, unavailable values | |
| `section_label` | 170 176 186 | captions such as INSERTS A-E, DEVICES, TRACK | |
| `accent` | 44 132 192 | selected/active toggles (filled), selected tabs | warnings, record |
| `focus` | 88 196 232 | keyboard focus ring, pressed/open edge, links, MAP badge | fills |
| `selection` / `selection_stroke` | translucent blue / 86 170 226 | edit-range fill / its edges, selected clips and strips | |
| `playback` | 70 196 112 | Play lit, "output running" dot | meters |
| `rec` | 220 58 52 | record arm, Record lit, REC tag | errors |
| `warning` | 236 168 56 | not blocking but needs attention: no output, an unverified state | the loop range |
| `error` | 240 92 80 | faults: missing plugin, failed device, FAULT tag | record |
| `loop_range` | 232 150 52 | the Loop lane and loop markers | |
| `solo` / `mute` / `input` | yellow / orange / green | S / M / I buttons and their tags | |
| `meter_green` / `meter_yellow` / `meter_red` | | meter zones only | anything else |
| `counter_bg` / `counter_text` | black / green | numeric readouts (counter, fader dB, pan) | |

Contrast: `theme::validate` refuses a palette in which text on panels, the window, buttons,
rulers or counters falls below 3:1. It also refuses one where record, solo, mute or meter clip
cannot be told from its resting colour. New built-in palettes are added to that test
(`themes_are_validated_data_with_fallback`) and to `tests/interface_polish.rs`.

## 3. Typography

| Constant | Size | Use |
|---|---|---|
| `size::CAPTION` | 10 | section captions (bold, `section_label`), units. The smallest text in the app; mixer state tags are 9 bold on a filled tag. |
| `size::SMALL` | 11 | secondary text, status details, hints |
| `size::BODY` | 13 | labels, buttons, list rows |
| `size::TITLE` | 14 | window and panel titles |
| `size::NUMERIC` | 13 | numeric readouts, monospaced (`mono`) |
| `size::NUMERIC_LARGE` | 15 | primary numeric entry (BPM) |

- Numbers that change while you watch (positions, dB, BPM, latency) use `mono()`, so digits do
  not jump. Right-align numeric columns (the Master meter grid is the reference).
- Captions are upper case and short: INSERTS A-E, I / O, DEVICES, TRACK, MONITORING.
- Do not add text below 9 points. Fader scale marks are 9; the unity mark (0) is brighter.

## 4. Spacing, sizes and corners

| Constant | Value | Use |
|---|---|---|
| `metric::CONTROL_H` | 22 | buttons, chips and fields in bars and dialogs |
| `metric::CONTROL_H_SMALL` | 18 | mixer strip rows and list rows |
| `metric::GAP` | 6 | between controls in a group |
| `metric::GROUP_GAP` | 12 | between groups |
| `metric::RADIUS` | 3 | controls, groups, cards (windows use 6, menus 4) |

`theme::group_frame()` draws a group: raised surface, hairline border, standard radius and
padding. Use it for control-bar segments and segmented tabs. Cards (Master workspace) use
`panel_bg` with a `border` hairline and 10-point padding.

## 5. Control states

| State | Fill | Edge | Text |
|---|---|---|---|
| Resting | `button` | `button_border` hairline | `text` |
| Hovered | `button_hi` | `border_light` | `text` |
| Pressed / open | `accent_dark` | `focus` | `text` |
| Selected / on | state colour (`accent`, `playback`, `rec`, `solo`, `mute`, `input`) | as resting | `text_dark` |
| Keyboard focus | unchanged | 1.5 pt `focus` ring | unchanged |
| Disabled | faded (`disabled_alpha` 0.45) | faded | `text_disabled`, plus a tooltip that says why |

The egui visuals are set once, in `theme::apply_tokens`. Custom-painted controls copy the same
table: `widgets::text_toggle`, `widgets::selector_box`, the fader cap and `control_bar::chip`.
A lit control changes from a neutral fill to a solid state fill, so "on" reads even in
greyscale.

## 6. Adding or changing tokens

1. Add the field to `Tokens` with a doc comment saying what it is for, and give it a value in
   `DARK`, `LIGHT` and `HIGH_CONTRAST`. `GRAPHITE` inherits from Dark unless you set it.
2. Add it to `fields_mut` (the theme editor) and update the count in the theme test.
3. Theme files written before the token existed still load: missing tokens default to Dark.
4. Add it to the table in §2.

## 7. Workspace patterns

- **Control bar** (`control_bar.rs`): one `group_frame` segment per job. In order:
  1. layouts (only when the menu bar has no room);
  2. transport (Mix and Master layouts);
  3. tempo and time signature;
  4. Click and Count Off;
  5. Loop;
  6. recording status;
  7. Audio Setup and the device status.

  Segments wrap whole, never cut in half. Each remembers its width, keyed by name.
- **Menu bar:** menus keep their room. The session title truncates first. If the layout
  switcher still does not fit, it moves into the control bar (`layouts::switcher_overflows`).
- **Mixer strip** (`mix_window::strip`), top to bottom:
  1. identity: colour bar, name, kind and state tags;
  2. processing: preamp, instrument, object, inserts, slots, EQ;
  3. sends and routing: sends, I/O, automation, group;
  4. pan and width;
  5. fader and meter, with the dB readout;
  6. channel actions: I and R, then S and M.

  Every strip uses the same rows, so sections line up. Strips are as tall as the tallest one
  needs, and the console scrolls.
- **Edit window:**
  - The selected range is a translucent fill with `selection_stroke` edges.
  - Selected clips get a 1.5-point `selection_stroke` outline.
  - Muted clips have a dimmed body, a `text_disabled` waveform and "muted ·" in the name bar.
  - The playhead is `playhead` over a dark underlay, with a head on the rulers.
  - Hit testing and sample coordinates (`x_of` / `sample_at`) are never changed by styling.
- **Dialogs:** labels sit before their controls. The New Tracks dialog keeps the owner-approved
  layout: a first row of count, format, kind and timebase, then a full-width Name row.

## 8. Checking a change

- Render the matrix with the `ui_shot` example. It covers Record, Edit and Mix
  (Console/Visual/Split) plus Master at 1920×1080, 1366×768, 911×512 (1366×768 at 150 %) and
  960×540 at 200 %:
  `cargo run -p soundcraft-ui-egui --example ui_shot -- out.png script.jsonl --size 911x512 --scale 1`
- Run `cargo test -p soundcraft-ui-egui`. The layout tests live in `tests/control_bar.rs`,
  `tests/mix_split.rs`, `tests/add_track.rs` and `tests/interface_polish.rs`.
- Renders show layout only. They do not prove native DPI, font hinting or pointer behaviour on
  Windows; say so in reports.
