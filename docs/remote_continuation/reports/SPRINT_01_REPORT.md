# Sprint 01 report — close the remaining software gaps (RC01–RC03)

Revision 3.0, executed 2026-10-12 in environment **P** only (Linux x86_64 container, 4 vCPU,
15 GiB RAM, rustc 1.97.0; no audio device, no display, no Windows host, no physical keyboard).
Nothing below was observed on W, H, M or L. Evidence classes: RERUN = ran here this sprint;
INSPECTED = read in source/logs; REPORTED = taken from an earlier report without rerun.

## Source identity
- Start: branch `fable/recording-core`, HEAD `9f9b4d30020680a5a18a23cb0ba1ba09a349b2ff`, clean,
  equal to `origin/fable/recording-core` (INSPECTED). No newer user changes to protect.
- Package install checkpoint: local commit `b774674` (docs only; hook-requested push declined:
  the package authorises local commits only).
- Sprint 1 milestone: local commit `fix: close recording-core software safety gaps` (hash in
  RESUME.md / RC_STATUS.csv). Not pushed.

## Outcome by requirement

| ID | Result | Evidence (sprint_01/) |
|---|---|---|
| RC01 | **PASS (P).** Full gate rerun at the verified source before any edit: fmt, clippy, test, assets, layers, wasm all ok, 678 passed / 0 failed / 1 ignored in 67 suites, exit 0 (carried gap C07 closed). Commands discovered from `xtask/src/main.rs` and `.github/workflows/release.yml`, not from prose. 38-row gap ledger with per-row provenance and a `status_now` column. | `baseline-ci.log`, `baseline.json`, `gap-matrix.csv` |
| RC02 | **PASS_PORTABLE for supported operations; H timing pending.** The off-thread `GraphPrep` + allocation-free `apply_prepared` path is now guarded for 13 structural edits during rendering, adding reorder tracks, add a latent insert, bypass it, route a bus into a sidechain and **replace the whole session** to the earlier nine: 0 allocations / reallocations / deallocations on apply and on four following blocks, 0 prepared fallbacks, output identical to the offline engine for every edit. Overrun status re-measured (release, idle CPU): at 64 frames the workload's p99 compute is 80 µs = 6 % of the 1333 µs budget; the no-work control misses 60/45 000 periods from scheduler preemption alone, so the 64-frame overruns are still not attributable to compute in P (B02 unchanged: native realtime-thread check pending). | `sprint1-ci.log` (structural_alloc), `callback-profile.csv`, `callback-profile-run.log` |
| RC03 | **PASS_PORTABLE; keyboard W/H pending.** Sandbox: declared added delay (one `max_block`) matches the measured impulse delay exactly with variable callback sizes 256/100/37/1/200/255/64/3/128/17, offline and paced like a realtime callback. **Gap found and fixed:** a worker that crashed mid-render (auto-restart on) used to yield an export with a silent gap; every substituted block is now counted and `file.bounce_mix` refuses (nothing written; `acknowledge_missing` does not cover it). Crash/hang containment, restart with state, no restart while recording: rerun. Toggles: QuickPunch/TrackPunch/Destructive Record/DestructivePunch refuse with a reason, and **a legacy session's QuickPunch flag is no longer shown as checked** on the disabled menu item (flag kept). Keypad: assessed, see below. | `fault.log`, `latency.csv`, `sprint1-ci.log` |

## Carried audit gaps handled this sprint

| Gap | Result |
|---|---|
| C01 R05 redaction + audit | Done. Logger redacts home → `~`, other profile folders → `<user>`, secret values → `<redacted>`, for ordinary records, the audio thread's held record and the engine panic report; `SOUNDCRAFT_LOG_PATHS=full` keeps paths (never secrets). `RELIABILITY_AUDIT.md` written from source; it found `crates/remote` without the no-panic deny line (added, clippy clean). |
| C03 VST3 aux sidechain | Done (host side). `Instance::configure` activates the first aux input bus with channels; `Vst3Plugin` accepts a sidechain and feeds that bus (mono keys fill every channel, silence when none, no stale key reuse). New fixture class "Fixture SideGate" passes audio only when the host has activated its aux bus and that bus carries signal; its test passes. Capability record and crate docs updated. Real third-party sidechain plugins: H pending. |
| C04 Bend clicks | Done. Each segment renders 480 samples past its anchor and crossfades (raised cosine) into the next. Test with three anchors at awkward phases: join steps were 0.358 / 0.497 / 0.688 before (25–48× the tone's own 0.0144 step, test failed), 0.0009 / 0.0144 / 0.0144 after; identity maps stay bit-exact. Help limits, guide and the assistant's evaluation check now say "crossfade". A phase mismatch can dip the level over the 10 ms fade (stated). Listening: L pending. |
| C07 full CI at HEAD | Closed by the X01 baseline above. |
| C08 ledger staleness | `docs/next_cycle_execution/STATE.md`: two duplicated checkpoint lines removed (the only edit to an earlier-cycle file). The R36 row's stale "package build pending" note is superseded in `GAP_MATRIX.csv` (prior ledger left as issued). |

## Keypad assessment (contract item 5)
egui 0.36.2 / egui-winit 0.36.2 (INSPECTED in the registry sources): `KeyEvent.location` is
discarded; the physical key is translated with `key_from_key_code`, where `NumpadAdd → Key::Plus`
(top-row is `Equal → Key::Equals`) but `Minus | NumpadSubtract → Key::Minus`. So keypad **+** is
distinguishable by its physical key and keypad **−** is not. Distinguishing only one of the
pair would make Num+ and Num− behave differently, so no change was made; the existing,
documented fallback stays (plain ± act as Num± only without text/plugin/modal/capture focus;
Nudge commands are remappable, Ctrl+Alt± for content slip). Closure needs a toolkit that
forwards `KeyLocation` or a custom eframe integration, plus a physical-keyboard check on the
target machine (TARGET_PENDING). Global system shortcuts are not intercepted.

## Commands run (exact)
- `CARGO_INCREMENTAL=0 cargo xtask ci` at `9f9b4d3` → `baseline-ci.log`, EXIT 0.
- `cargo test -p soundcraft --bin soundcraft logging` (21 passed).
- `cargo test -p soundcraft-engine --lib warp::` (failed before the fix as recorded above; 3 passed after).
- `cargo test -p soundcraft-vst3-host` (12 + 10 + harness-less module-exit checks ok).
- `cargo test -p soundcraft-mix --test structural_alloc -- --nocapture` (13 edits, all 0/0/0).
- `SML_SANDBOX_CSV=<abs>/latency.csv cargo test -p soundcraft-engine --test sandbox` → `fault.log`, EXIT 0.
- `CARGO_INCREMENTAL=0 cargo build --release -p soundcraft-engine --example sml_profile --example ipc_probe`;
  `./target/release/examples/sml_profile callback-profile.csv response-profile.csv` → EXIT 0.
- Sprint gate `CARGO_INCREMENTAL=0 cargo xtask ci`: attempt 1 failed at clippy (a test-only
  builder flagged dead code, one `manual_pattern_char_comparison`; log kept as
  `sprint1-ci-attempt1-clippy-fail.log`); both fixed; attempt 2 → fmt, clippy, test, assets, layers, wasm all ok; 683 passed / 0 failed / 1 ignored in 67 suites; EXIT 0 (`sprint1-ci.log`). RC02 guard output: `structural-guard.log`.

## Changed files
apps/soundcraft/src/logging.rs; crates/dsp/src/lib.rs (`Plugin::substituted_blocks`);
crates/engine/src/{assistant.rs, cmd/help.rs, io.rs, warp.rs}; crates/engine/tests/sandbox.rs;
crates/mix/src/{lib.rs, sandbox.rs}; crates/mix/tests/structural_alloc.rs;
crates/remote/src/lib.rs; crates/ui-egui/src/menus.rs; crates/vst3-host/src/{ffi.rs, lib.rs,
plugin.rs}; crates/vst3-host/tests/{common/mod.rs, fixture/src/lib.rs, fixture_e2e.rs};
docs/USER_GUIDE.md; docs/next_cycle_execution/STATE.md; docs/remote_continuation/**.

Interfaces: one new defaulted trait method (`Plugin::substituted_blocks`, default 0) and one new
`RenderReport` field (`faulted_plugins`); no session schema, id or saved-state change.

## Not done / not run / external gates
- H: 64-frame callback with a realtime-priority audio thread; real sidechain plugins; device loss.
- W: everything Windows (Sprint 3 cross-builds; execution stays W pending).
- L: listening to Bend joins.
- Keyboard: physical keypad check (W/H).
- Independent review: none; these are self-checks.

## Resource use
Disk: pruned ~8 GB of stale build output (incremental caches, superseded test binaries) to
continue; ~7 GB free after. Gate wall time ≈ 25–30 min per non-incremental run.

## Next safe action
Sprint 2 (RC04–RC06): freeze the held-out evaluation first, then the local-endpoint language
provider (real seam, inference BLOCKED in P per X03), built-in speech synthesis and enrolled
command recognition, routing proof and measurements.
