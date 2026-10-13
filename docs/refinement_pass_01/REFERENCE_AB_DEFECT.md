# Open functional defect K1: reference A/B produces no audio

**Status: REPAIRED in Hotfix Pass 03 (HF15)**, in source and portable tests; native listening on
Windows is pending. The repair: `crates/playback/src/lib.rs` `AudioState::fill` reads the
reference from the session pool (decoded at import), applies its level-match gain, aligns
reference frame 0 with session position 0, crossfades program and reference over 10 ms, and
reports `Player::reference_state()`. It is added on the device output only (never in renders,
exports, captures or the program meters). The reproduction test below was replaced by
`ui_refinement.rs::reference_ab_plays_the_reference_on_the_monitor_output` and
`soundcraft_playback` `reference_ab_tests`. The history below is kept as it was.

**Status at refinement pass 01: OPEN.** Reference listening was **not** complete. The UI pass that records this defect
(refinement pass 01) changes only presentation: it states the defect on screen and corrects a
wrong hint. It does not repair the audio path. The repair is separate work.

## Symptom

In the Master layout, with a reference set, the **A/B: listen to reference** button lights and
the session stores the reference's `listening` field (`true`). When you press Play you still hear
the program; the reference is never heard. Level match and the offline meters do work.

## Exact source path (code at `0029f79`, unchanged by this pass)

| Step | File and lines | What it does |
|---|---|---|
| Button | `crates/ui-egui/src/layouts.rs`, `master()`, the PROGRAM AND REFERENCE card ("A/B: listen to reference") | runs `mastering.reference {"listening": !listening}` |
| Command | `crates/engine/src/cmd/mastering.rs`, `mastering.reference` (about lines 36–69) | stores `Reference { source, gain_db, listening }` in the session. Nothing else. |
| Model | `crates/model/src/session.rs`, `pub struct Reference` (about lines 801–809) | the doc comment says "The listener hears the reference instead of the program (A/B)". No code implements it. |
| Intended audio path | `crates/playback/src/lib.rs`, `AuxMonitor` (about lines 105–200): `push`, `set_gain_db`, `solo`; summed in the device callback `fill` (about lines 548–552): `if aux.solo { program = 0 }`, then `aux.pop_into(frame)` | a monitor-only ring that would play the reference on the speakers and never in a bounce or capture |
| **Missing link** | — | No code reads `listening` to drive playback. No code pushes reference audio into `AuxMonitor` (its only producer is assistant speech, `crates/ui-egui/src/assistant_ui.rs:272`). No code sets `AuxMonitor::solo` or calls `set_gain_db` for the reference. |

Evidence: `evidence/k1-reference-ab-code-search.txt` (grep output).

## Reproduction

**Automated (portable, runs in CI):**
`cargo test -p soundcraft-ui-egui --test ui_refinement defect_k1_reference_ab_feeds_no_audio_to_the_monitor_path`

1. Takes the demo session.
2. Sets a reference (`mastering.reference {source}`).
3. Opens the Master layout and clicks **A/B: listen to reference**.
4. Presses Play and runs 30 frames.
5. Asserts that the session stores `listening = true`, but the player's `AuxMonitor` has no queued
   frames and its `solo` is off: no reference audio can reach the output.
6. Checks that the Master card shows "OPEN DEFECT".

The test documents the defect. When the audio path is repaired it fails by design, and must be
replaced with a test of the repaired behaviour.

**Manual (Windows, with audio; not performed in this session):**
1. Open a session with a mix and import a reference file (File › Import › Audio…).
2. Note its source id: control channel `session.inspect` → `sources`, or `soundcraft-cli`.
3. Run `mastering.reference {"source": <id>}`. There is no menu entry for this. The old hint
   "Clip List › right-click › Use as reference" named an item that does not exist; the card now
   says to use the command.
4. Window › Master layout. The PROGRAM AND REFERENCE card shows "Reference: source <id> at +0.0 dB".
5. Click **A/B: listen to reference**; it lights.
6. Press Play. **Expected:** the reference, at the level-matched gain, instead of the program.
   **Actual:** the program continues; no reference is heard.

## What a repair must do (for the separate fix; not implemented here)

- When `listening` is true and the transport plays: stream the reference source into
  `AuxMonitor` with `push` (from a non-real-time thread), converted to the device rate. Set
  `solo = true` and apply `set_gain_db(gain_db)`. Clear and un-solo when listening stops, the
  transport stops, or the reference changes.
- Keep it monitor-only: never in `render_range` / bounces, never in the capture ring. The
  `aux_monitor_tests` in `crates/playback/src/lib.rs` already prove that property for the ring.
- Decide position alignment with the program (start of reference vs program position) and say it
  in the manual.
- Replace the reproduction test with tests of the repaired behaviour, then verify by ear on
  Windows.

## Presentation in this build

- Master card: "OPEN DEFECT: A/B stores the setting only — no reference audio reaches the
  speakers (known issue K1). Compare in another player at the level-matched gain."
- Manual §2 capability table, §18 and §26 K1 say the same.
