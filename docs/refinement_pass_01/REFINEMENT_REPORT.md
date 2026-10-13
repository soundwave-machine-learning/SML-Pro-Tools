# SML Studio — UI refinement pass 01

Date: 2026-10-13. Environment: **P (portable)** — a Linux build container, Rust 1.97.0, egui 0.36,
headless renders and UI tests. **No Windows interaction was observed**: every Windows check below
is pending.

## Source identity

| | |
|---|---|
| Branch | `fable/recording-core` (not `main`; nothing pushed, merged or published) |
| Starting checkpoint | `0029f79` (interface polish pass 01). Clean tree. Hotfix HF01–HF06 and the polish work are kept. |
| Code commit | `3aa6554` |
| Documentation | the commit that adds this file |
| Candidate | see *Candidate* below. Staged only: the installed candidate is not replaced. |

## 1. Visual mixer labels and coincident channels

**Before:**
- Every name was printed centred above its node, with no collision handling.
- In the demo, six names were piled on top of each other at the centre (unreadable).
- Nodes at the same pan and gain covered each other, and only the last one drawn could be
  clicked or dragged.

**After** (`crates/ui-egui/src/visual_mixer.rs`):
- `place_labels`, a pure function, places each label by trying spots around its node: above,
  below, right, left, then further above or below with a thin leader line. A spot is taken only
  if it stays on the canvas and overlaps no other label and no node.
- `clusters` groups nodes whose rectangles overlap. A group gets one chip, "Snare +3", whose
  accessible name lists the members. Clicking the chip lists every member with its gain, pan,
  width and states; choosing one selects it.
- Selected nodes are drawn and hit-tested last, so a drag at a shared spot moves the chosen
  channel.
- Hovering a node shows a card with its gain, pan, width and states. The line above the stage
  reads "Selected: Snare — −4.0 dB · C".
- **Values:** node positions still come only from gain, pan and width. The new code reads the
  mixer and never writes it. The only writes remain the existing drag, double-click and list
  commands.

## 2. Comfortable / Compact mixer density

- **Control:** Comfortable and Compact chips in the mixer header (Console and Split), plus
  `ui.mix_density {"density": …}`.
- **Persistence:** both use the existing `narrow_mix` preference (View › Narrow Mix,
  `window.narrow_mix`), saved in `ui.json`. Nothing new is stored and nothing in the session
  changes.
- **Compact readability:**
  - section captions shorten when the full one does not fit ("INS A-E", "SND A-E");
  - names end in "…" (full name on hover and in the accessible name);
  - pan readouts are monospaced, each confined to its half of the strip;
  - width reads "100%" in smaller type;
  - slot names are clipped to their slot.
- **Both densities:** fader scale labels that would collide are skipped; the ticks stay, and 0 dB
  is always labelled. Previously 30/40/60 printed on top of each other.

## 3. Compact Record toolbar

**Before:** in the Record layout on a 1366×768 screen at 150 % (911×512 points), the toolbar
beside the 300-point RECORD panel is about 595 points wide. The pinned Nudge (402) and transport
(176) left the scrolling tools a sliver, and Nudge's left edge was overlapped.

**After** (`crates/ui-egui/src/toolbar.rs`):
- When the pinned groups would leave the tools less than 250 points, the toolbar uses two rows:
  the tools and other groups on top at full width, Nudge and the transport below.
- When the transport does not fit beside Nudge either, as at 911×512 and at the 960×600 minimum
  window at 150 %, it is drawn in the control bar, as in the Mix and Master layouts. Stop, Play
  and Record are never hidden.
- At the extreme minimum, Nudge's row scrolls rather than clipping.
- Wider windows keep the existing one-row layouts, so the RC09 test is unchanged.

## 4. Empty input meters

**Before:** egui's progress bar always drew a fill at least as wide as it is tall, so zero
looked like a small blue dot. The Record panel also read "-inf".

**After** (`widgets::input_level_bar`, used in the Record panel and Audio Setup):
- There is no fill at zero. With a signal, the fill is the real level on the channel meters' scale
  and zones.
- The bar reads "input: no signal", or "input: no signal (input device not open)" when the input
  is not open yet.
- It is exposed to screen readers as a progress indicator with that text.
- No level is ever simulated.

## 5. Reference A/B — OPEN FUNCTIONAL DEFECT (K1)

Not repaired in this pass, and **not** labelled complete. The defect record covers the exact
source path, grep evidence, an automated reproduction, the manual reproduction steps and what a
repair must do: [REFERENCE_AB_DEFECT.md](REFERENCE_AB_DEFECT.md).

In short: the A/B button stores the reference's `listening` field, but nothing feeds the reference to
`soundcraft_playback::AuxMonitor`, the monitor-only path built for it. This pass changed only
presentation:
- the Master card now says "OPEN DEFECT";
- the wrong "Clip List › Use as reference" hint is replaced by the real way to set a reference;
- the manual and the Grok guide describe it as an open defect.

## Tests and checks

| Check | Result |
|---|---|
| `tests/ui_refinement.rs` (new, 6 tests) | pass. **Labels:** none overlap, all on screen, every channel named, at 1920×1080, 1366×768, 911×512 and 960×540@200 %; hovering changes no value. **Coincident channels:** the cluster chip lists every member; choosing one selects it; a drag at the shared spot moves only that channel (one undo; undo restores). **Density:** chips and command, compact strips narrower with names not overlapping, the same preference as Narrow Mix, survives a prefs round trip, refuses bad values, never changes the mix. **Record toolbar** at 911×512, 640×400 and 1366×768: Nudge drawn, transport in the toolbar or the control bar, no overlaps. **Empty meter:** "input: no signal (input device not open)", no dBFS. **K1 reproduction:** listening stored, no audio queued, Master card says OPEN DEFECT. |
| Unit tests | `label_placement_never_overlaps_labels_or_nodes` (dense synthetic stage; clusters, including chained overlaps) and `narrow_toolbar_stacks_without_hiding_nudge_or_transport` (520 and 611 points; one row at 911, 1366 and 2560). |
| Existing suites | all `soundcraft-ui-egui` tests pass, including RC09 toolbar reachability, HF01–HF06, polish and Audio Setup meter tests. No existing test was changed. |
| Full gate `cargo xtask ci` on `3aa6554` | fmt, clippy, **test (739 passed, 0 failed)**, assets, layers: pass (`evidence/ci.log`). The docs step first **failed**: the generated references lacked the new `ui.mix_density`, and two documents wrote a field path that the checker reads as a command id. I regenerated the references (`cargo xtask docs`) and reworded the field; then docs-check, wasm and assets passed (`evidence/ci-remaining-steps.log`). No code changed between the runs. |

## Evidence (`evidence/`)

- `compare-*.png`: 12 before (`0029f79`) / after pairs.
  - Visual at 1920×1080, 1366×768, 911×512 and 960×540@200 %.
  - Console in compact and comfortable density.
  - Split at 911×512.
  - Record at 911×512, at 640×400 (minimum window at 150 %) and at 1366×768.
  - Audio Setup with an armed track, and Master with a reference and A/B on.
- `render-scripts/`: the scripts used for both builds. The "before" renders came from `ui_shot`
  built at `0029f79`.
- `k1-reference-ab-code-search.txt`, `ci.log`.

Renders show layout only. They do not show native DPI, font hinting, pointer feel or audio.

## Pending and known gaps

1. **Windows interaction (all four items):**
   - Visual cluster chips and dragging coincident nodes with a real mouse;
   - Compact readability at your display scale;
   - the stacked Record toolbar and the control-bar transport at 1366×768 at 150 %;
   - the input bar with a real microphone signal.
2. **K1 reference A/B: open functional defect,** to be repaired as separate audio-path work
   (REFERENCE_AB_DEFECT.md).
3. At the 640×400 minimum window the Record layout is very tight. The edit area and the docked
   Tracks/Clips lists compress (pre-existing), and Nudge's row scrolls.
4. A dense cluster's chip names only the first channel ("Snare +6"). The others are in its list,
   tooltip and accessible name, not on the canvas.

## Candidate (staged, not installed)

Built with `OUT=dist/refine01-windows bash packaging/windows/cross-build.sh`. This is the GNU
cross-build (`x86_64-pc-windows-gnu`, rustc 1.97.0, release, `--locked`): WASAPI only, unsigned,
no installer, with the documentation bundle at revision 2026-10-13.4. Source `4d8749f` (clean
tree). Program code is identical to `3aa6554`; the compiled-in manual is the one at `4d8749f`.

| Artifact | SHA-256 |
|---|---|
| `soundcraft-0.5.0-windows-x64-gnu-portable.zip` | `115666bd39f2650cae6f67b10e2d030b7bc1cd3b079b12dced18c0fc32ef90a7` |
| `soundcraft.exe` (inside) | `a79cf45f45e1321df6f05c4f2ac12edadaf1073ee59f6543ea0c238eefe37f80` |
| `soundcraft-cli.exe` (inside) | `fb50823b4ce9118332827c5f4fdef152ad27112c14ff3db9476770113b93ac1b` |
| `MANIFEST.json` (beside the zip) | `1bab5415af6085885eb1d54e6bcad19152c55577a8f762300a5adc4e336da176` |

**Not installed.** The installed Hotfix Pass 01 candidate was not touched. To try this build,
extract it into a new folder and run it there; it shares `%APPDATA%\SoundCraft`, including the
density preference. Reference A/B in this build is still the **open defect K1**.
