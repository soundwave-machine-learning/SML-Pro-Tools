# Next Sprint 02 — Finish the studio workflow (revision 2.2 contract)

Status: **PARTIAL** — every R10–R22/R37 behaviour has a portable implementation and tests
through the shared engine/command service (full `cargo xtask ci` exit 0, see "Tests run"), with
the named gaps below; native evidence (W1/W2 menu timings, keyboard/keypad, phone deployment,
vocal walkthrough at 100/150/200 %, Bend listening) is TARGET_PENDING. Environment P only
(`docs/evidence/next_cycle/sprint_01/environment.json`). Started 2026-10-12T01:00Z from commit
`3f7e50f` (Sprint 1 PARTIAL + measurements).

## Outcome (contract items 1–9)
| Item | Result | Evidence |
|---|---|---|
| 1 Vocal workflow gaps (R10) | portable PASS: the ledger rows carried from the prior cycle rerun, plus a lead/double workflow through the new tool commands (split at breath, erase, mute with explicit state, clip-gain envelope, Bend, Listen) with save/reopen and unchanged source hashes | `vocal-coverage.csv`, `vocal-edit-tests.log` |
| 2 Toolbar: eight tools, eight Paint shapes, Bend render path, Listen transaction (R11) | PASS_PORTABLE for T01–T05, T07–T10, T-T1–T-T5; T06 portable (plugin-parameter lanes finish in Sprint 3 A3); T11 carried; T-T6 native | `toolbar-coverage.csv`, `toolbar-tests.log`, `audio_tools` UI test |
| 3 Nudge section, exact semantics (R12) | PASS_PORTABLE rerun (N01–N06, N08 incl. 1000 × 1 ms at 44.1 kHz returning exactly); N07 physical keypad / N09 native pending; B03 toolkit blocker unchanged | `nudge/nudge-engine-tests.log` |
| 4 Shortcut inventories + Pro Tools / Studio One / Custom profiles (R13) | PASS_PORTABLE for the profile system (explicit reversible switch, capture reassignment, unbind/reset/import/export, conflict inspector incl. OS-reserved and undeliverable chords, text/dialog precedence); inventory: Pro Tools rows implemented-and-mapped, **Studio One rows pending native verification** (manuals S02/S03/S05 unreachable from P: proxy 403), keypad rows unsupported (B03) | `shortcut-inventory.csv`, `shortcut_profiles` tests |
| 5 Buses/sends/solo/spill, Console/Visual/Split, profiles, slots (R14–R17) | PASS_PORTABLE rerun of the prior-cycle suites (B-T, M-T, SL-T, VM-T software rows); no new DSP; personal plugin rows AWAITING_EXTERNAL_PLUGIN | `routing-mixers-profiles-slots-tests.log`, `bus-slot-commands-tests.log` |
| 6 Record/Edit/Mix/Master layouts, SML Studio naming, data-only themes (R18, R20) | PASS_PORTABLE: four layouts of one app (identity check test), Master = labelled navigation shell; themes are validated JSON (dark/light/high-contrast built-ins, editor with live preview, import/export/reset, 3:1 contrast and state-distinguishability rules, broken saved theme → Dark at launch); appearance checked at 1.0/1.5/2.0 scale | `studio_layouts` test, `theme` tests, `theme_docking` test |
| 7 E04 menu/search stalls (R19) | portable: menu tree and search index built once (were rebuilt per frame/keystroke); 100-rep cold/warm distributions at W1 recorded (software renderer); **native W1/W2 and audio continuity under menu use TARGET_PENDING** | `menu-timing.csv` |
| 8 LAN remote with pairing/revocation, phone controls, acknowledged commands, reconnect snapshots, bounded meters; monitor volume/dim/mono; cue only for a real route (R21, R37) | PASS_PORTABLE (software): hand-written HTTP service, 6-digit pairing with TTL/attempts, HttpOnly cookie or bearer tokens, Origin check, allowlist, rate limit, bounded queue, R09 envelopes (dedup/stale), record nonce (no replay after reconnect), coalesced meters, two-client stale rejection, disconnect leaves transport; monitor stage on the device output only; cue reported unavailable without a dedicated path. **No HTTPS → labelled browser-only HTTP; no PWA/microphone/encryption claims; no QR image; real phone test TARGET_PENDING (M)** | `remote-tests.log`, `studio_layouts` phone test, `monitor-tests.log` |
| 9 Searchable build-versioned command help with limits (R22) | PASS_PORTABLE: `help.search`/`help.limits` over the real registry + UI commands, active-profile binding, status (DISABLED rows), risk, stated limits; nonexistent tools return nothing | `help::` tests, Command Help window |

## Requirement statuses
See `docs/next_cycle_execution/REQUIREMENT_STATUS.csv` (authoritative). Summary: R10 PASS_PORTABLE;
R11 PASS_PORTABLE (T-T6 native); R12 PASS_PORTABLE (N07/N09 native); R13 PARTIAL (Studio One
chords unverified against the manual); R14–R17 PASS_PORTABLE (carried, rerun); R18 PASS_PORTABLE
(Master = shell); R19 PARTIAL (portable timings only); R20 PASS_PORTABLE; R21 PARTIAL (HTTP only,
no mobile test); R22 PASS_PORTABLE; R37 PARTIAL (no separate cue mix).

## Measurements
- E04 (portable, software renderer, W1 = 16 tracks / 2000 clips, 100 repetitions): menu open
  warm p50 9.6 / p95 12.5 / p99 18.1 / max 18.1 ms, cold 9.9 ms; indexed search warm p50 10.4 /
  p95 14.7 / p99 15.6 / max 15.6 ms, cold 11.3 ms (`menu-timing.csv`). These bound the
  UI-thread model/layout cost (every frame is CPU-rasterised in the harness); delivered native
  frames, input latency and audio continuity under menu use are not measured here.
- Toolbar drawing limits frozen before scoring: Paint density 10 ms default / 1 ms minimum /
  100 000 points; Bend anchors ≥ 64 samples apart, 0.25–4× per segment (COMMAND_MATRIX.md).

## Files / commits
Milestone commit `feat: complete SML Studio editing mixing and remote workflow` (SHA in
`STATE.md`). New: `crates/remote/` (service + page), `crates/engine/src/{warp.rs,
cmd/tools.rs, cmd/audition.rs, cmd/bend.rs, cmd/help.rs}`, `crates/ui-egui/src/{layouts.rs,
shortcut_profiles.rs}`, UI tests `audio_tools.rs, shortcut_profiles.rs, studio_layouts.rs,
menu_timing.rs`, `docs/next_cycle_execution/SPRINT_02_DESIGN.md`, evidence under
`docs/evidence/next_cycle/sprint_02/`. Changed: model (`Tool` +8 variants, `EditState`
tool/paint fields, `Clip.warp`, `Session.listen`, `ListenMask`), mix (listen route mask),
playback (monitor stage), engine (execute hooks for audition, renderer field, `edit.mute_clips`
`muted`), ui-egui (edit window gestures, toolbar group, shortcuts resolver, theme data, menus
caching, windows: shortcuts/remote/help/theme editor, panels monitor controls), app (title,
remote autostart), xtask layers, USER_GUIDE §3a–3e.

## Tests run (all P)
Full `CARGO_INCREMENTAL=0 cargo xtask ci` exit 0: fmt, clippy (-D warnings), 661 tests passed /
0 failed, assets, layers, wasm (log `ci-sprint2.log`; the wasm step first failed because the
remote crate pulled OS randomness into the web build and was fixed by making SML Remote
native-only); the suites named per row above with their logs in
the evidence folder. Fault/edge models actually exercised: locks on every tool command, zero-
length split, Shuffle ripple, Escape during sweeps, stale/crossed/too-steep anchors, stale render
results after undo, queue ceiling, audition while playing/recording, non-source tracks, invalid
theme files (format, scale, contrast, state colours, garbage, partial), wrong/expired/too-many
pairing codes, foreign Origin, missing/revoked tokens, disallowed commands, duplicate request
ids, stale revisions, old record nonces, rate limit, full queue, dropped connections.

## Not run / failed / blocked / gaps (named)
- Native: T-T6 walkthrough at 100/150/200 %, physical keypad (N07), N09 load, W1/W2 native menu
  timings and audio continuity under menu use, real phone pair/disconnect/replay on iOS/Android,
  Bend listening review, owner plugin rows (V12/H2) — TARGET_PENDING.
- R13: Studio One chords are best-known defaults; the reference manuals could not be fetched
  (proxy 403), so every Studio One row is reported "pending native verification"; keypad-only
  chords are unsupported (B03). Not advertised as complete.
- R21: no HTTPS provisioning route exists in the tree (no TLS crate); the service is labelled
  browser-only HTTP; PWA installability, microphone and encryption are not claimed; no QR image.
- R37: a dedicated cue path is detected but no separate cue mix/level exists; reported, not relabelled.
- R19: menu/plugin metadata caching done; native distributions pending.
- T11 quantize preview and a snap-override gesture test not added; strip-silence preview UI unchanged.
- Bend: segments butt-joined (no crossfade), stated in help/limits and the guide.
- No test was weakened. Pre-existing bug fixed on the way: the Edit window registered a track
  row after drawing it, so a drag starting on the top track could not look its own row up
  (affected cross-track moves and sweeps); rows are now registered before they draw.

## Risks
- The `Tool` enum grew by eight variants; sessions saved by this build with an audio tool
  selected load in older builds only if they tolerate unknown `tool` ids (they default on error).
- Remote: plain HTTP on a LAN exposes commands to anyone who can pair; pairing is time-boxed and
  attempt-limited, tokens are per client and revocable, and the allowlist excludes
  save/export/destructive commands. Do not bind to 0.0.0.0 on an untrusted network.
- Listen relies on the realtime engine's route mask; a third-party realtime host that bypasses
  `Player::update_session` would not honour it (none exists in this tree).

## Next safe task
Sprint 3 task 1 (plugin hosting hardening per `SPRINT_03.md`), keeping `CARGO_INCREMENTAL=0
cargo xtask ci` at exit 0; the E03 numbers are the admission criteria for the opt-in sandboxed
instance mode (R31).
