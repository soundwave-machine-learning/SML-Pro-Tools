# Execution command matrix (E01) — discovered from the repository, not guessed

Discovered 2026-10-11 from `xtask/src/main.rs`, `AGENTS.md`, `packaging/`, `.github/workflows/` and
the previous cycle's `docs/evidence/sprint_01/validation-command-index.md`. Every command below was
executed in this container (environment P) unless its row says otherwise.

| Purpose | Exact command | Source | Notes |
|---|---|---|---|
| Full repository gate | `CARGO_INCREMENTAL=0 cargo xtask ci` | `xtask/src/main.rs::ci` | fmt `--check`, `clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, assets (ATTRIBUTION rows), layers, `cargo check --target wasm32-unknown-unknown`. `CARGO_INCREMENTAL=0` only because the container's disk allowance is small. |
| Single crate tests | `cargo test -p <crate> [filter]` | cargo | crates: soundcraft-{time,audio-io,midi,dsp,model,mix,engine,playback,automation,ui-egui,clap-host,vst3-host,au-host,video}, apps soundcraft/soundcraft-cli/soundcraft-web |
| Allocation guard (steady state) | `cargo test -p soundcraft-mix --test realtime_alloc` | prior cycle | `stats_alloc` global allocator; serial strips 0 allocations |
| Structural-mutation allocation guard (B01) | `cargo test -p soundcraft-mix --test structural_alloc` | this cycle | see EXPERIMENT_DECISIONS.md (B01) |
| Capture/recovery | `cargo test -p soundcraft-playback -- capture:: overflow writer_error crash_recovery ring_memory disarmed disconnect` | prior cycle | |
| Clock/device | `cargo test -p soundcraft-playback -- clock:: device::` | prior cycle | |
| Headless callback profile | `cargo run --release -p soundcraft-engine --example sml_profile -- cb.csv rp.csv` | prior cycle | not native |
| Offscreen UI render | `cargo run -p soundcraft-ui-egui --example ui_shot -- out.png script.jsonl --size WxH --scale PPP` | AGENTS.md + prior cycle | needs a Vulkan driver (lavapipe) |
| Linux package | `cargo build --release --locked -p soundcraft -p soundcraft-cli && packaging/linux/package.sh --skip-build --formats tar` | `packaging/linux/package.sh` | |
| Windows cross-build | `cargo build --release --locked -p soundcraft -p soundcraft-cli --target x86_64-pc-windows-gnu` | prior cycle | mingw; NOT a Windows runtime test |
| Windows native package | `packaging/windows/package.ps1` | repo | NOT RUNNABLE here (needs PowerShell/MSVC) |
| macOS / FreeBSD / web packaging | `packaging/macos`, `packaging/freebsd`, `packaging/web` | repo | not run; not the delivery target |
| Package verifier (documents only) | `python3 -I docs/next_cycle_contracts/tools/verify_package.py` | package | validates the contract folder, not the product |
| Headless CLI workflow | `soundcraft-cli run --demo --cmd '<id>=<json>' --bounce out.wav` | AGENTS.md | |
| GUI with control channel | `cargo run --release -p soundcraft -- --demo --control 0` | AGENTS.md | needs a display; not runnable here |

Pre-existing conditions: `cargo check --target wasm32-unknown-unknown` prints one dead-code warning
(`CaptureStates` variant) that predates both cycles (`docs/evidence/sprint_01/baseline-wasm.log`).

## Paint shape conventions (T06, declared before tests were authored)

Command `automation.draw` (`crates/engine/src/cmd/paint.rs`). Orientation: time runs from `start`
to `end` (samples); values are the parameter's own units (dB for volume/send level, −1..1 for pan).
Endpoint convention: `from` is the value at `start`, `to` the value at `end`; both endpoints are
written (the final endpoint survives coalescing). Points inside the range are replaced; points
outside are preserved; one undo record per draw. Periodic shapes (square, triangle, saw, sine)
use `baseline` (centre value), `amplitude` (peak deviation), `cycles` (over the range, > 0) and
`phase` (fraction of a cycle, 0..1). Square: high for the first half of each cycle. Triangle:
baseline at the cycle start, peak at the half cycle. Saw: rises from −amplitude to +amplitude
across the cycle. Sine: starts at the baseline, rising. Parabola: `from`→`to` with exponent
`curve` (1 = line). Point density: one point per `density_ms` (default 10 ms, minimum 1 ms),
capped at 100 000 points per draw; the declared error tolerance is the linear interpolation
error between points at that density. Values are clamped to the parameter range; non-finite
input is rejected before any change. Transform: shifts/scales the existing points inside
`[box_start, box_end]` (inclusive) by `time_shift`, `time_scale` (0 < x ≤ 1000), `value_offset`,
`value_scale`; points that leave the box are dropped; the clip-gain envelope is a separate domain
(`clip.gain_draw`, Sprint 2) and is never written by this command.

## Sprint 2 additions (discovered/created 2026-10-12)

| Purpose | Exact command | Source | Notes |
|---|---|---|---|
| Audio tool commands (T02–T07) | `cargo test -p soundcraft-engine --lib -- tools:: paint::` | this cycle | split/erase/mute/gain_draw/tool settings; `automation.draw` shapes |
| Listen audition transaction (T09) | `cargo test -p soundcraft-engine --lib -- audition::` and `cargo test -p soundcraft-mix --lib listen_mask` | this cycle | engine state machine; mixer route mask on the realtime engine only |
| Bend warp service (T08, V11) | `cargo test -p soundcraft-engine --lib -- bend:: warp::` | this cycle | edit map, bounded admission, stale drop, undo, pitch within 2 % |
| Toolbar UI (T-T1/T-T4/T-T5) | `cargo test -p soundcraft-ui-egui --test audio_tools` | this cycle | egui_kittest gestures for all eight tools, Escape rollback, Link/alternate zones |
| Shortcut profiles (R13) | `cargo test -p soundcraft-ui-egui --lib -- shortcut` and `--test shortcut_profiles`; `SML_WRITE_INVENTORY=<csv> cargo test -p soundcraft-ui-egui --lib inventory_ledger` | this cycle | profiles, capture, import/export, conflicts; writes `shortcut-inventory.csv` |
| Themes (R20) | `cargo test -p soundcraft-ui-egui --lib -- theme` and `--test theme_docking` | this cycle | validation/fallback; appearance at 1.0/1.5/2.0 scale incl. high contrast |
| Layouts, monitor, remote through the host (R18/R37/R21) | `cargo test -p soundcraft-ui-egui --test studio_layouts` | this cycle | layout switch identity check; monitor outside the document; phone client over HTTP driving the UI thread |
| Remote service (R21) | `cargo test -p soundcraft-remote` | this cycle | pairing, Origin, dedup/stale, record nonce, rate limit, bounded queue, meters |
| Menu/search timing E04 (portable) | `SML_MENU_TIMING_CSV=<csv> cargo test -p soundcraft-ui-egui --test menu_timing` | this cycle | 100 repetitions cold/warm at W1 (16 tracks/2000 clips), software renderer; native W1/W2 pending |
| Knowledge (R22) | `cargo test -p soundcraft-engine --lib -- help::` | this cycle | versioned question set incl. nonexistent tools |
| Vocal tools workflow (R10 E2E extension) | `cargo test -p soundcraft-engine --test vocal_workflow` | this cycle | lead/double through the tool commands, save/reopen, source hashes |
| Monitor stage math (R37) | `cargo test -p soundcraft-playback --lib monitor_tests` | this cycle | device-output-only gain/dim/mono |

New command ids this sprint: `edit.split_at`, `edit.erase_clips`, `edit.mute_clips {muted?}`,
`clip.gain_draw`, `edit.tool_link`, `edit.alt_tool`, `edit.paint_shape`, `audition.start/stop/status`,
`clip.warp_set/warp_clear/warp_poll/warp_status`, `help.search`, `help.limits` (engine);
`window.record/master/remote/help/theme_editor`, `remote.start/stop/pair/revoke/status`,
`monitor.volume/dim/mono/cue`, `ui.theme_import/export/reset/token`,
`ui.shortcut_profile/shortcut_set/shortcuts_reset/shortcuts_export/shortcuts_import/shortcuts_conflicts` (UI layer).

## Sprint 3 additions (discovered/created 2026-10-12)

| Purpose | Exact command | Source | Notes |
|---|---|---|---|
| Host capability matrix (R31) | `SML_HOST_MATRIX_CSV=<csv> cargo test -p soundcraft-engine --test clap_e2e -p soundcraft-vst3-host --test fixture_e2e` | this cycle | live-instance records of the CLAP and VST3 fixtures + the seven external targets → `host-capability-matrix.csv` |
| Sidechain, PDC fixtures, bypass compensation, offline tail (R24/R25/R29/R31) | `cargo test -p soundcraft-mix --lib -- sidechain insert_pdc offline_render_tail compensation` | this cycle | 0/1/63/128/1024-sample fixtures parallel/nested/bypassed; aux exact vs one-block lag on tracks |
| CLAP sidechain port end to end; export guard; versions (R31) | `cargo test -p soundcraft-engine --test clap_e2e` | this cycle | fixture `sidegate` plugin; refused substituted master; recorded versions |
| Sandboxed hosting E03 follow-up (R31) | `SML_SANDBOX_CSV=<csv> cargo test -p soundcraft-engine --test sandbox` | this cycle | harness-less; the test binary is its own worker; parity, crash, hang, restart, state |
| Bend re-render after reopen (R30) | `cargo test -p soundcraft-engine --lib -- bend::` | this cycle | derived media missing → re-render from the original |
| W1 hosted chain under autosave + state capture (task 9, portable control) | `SML_W1_HOSTED_CSV=<csv> cargo test -p soundcraft-engine --test w1_hosted_load` | this cycle | per-block render time vs budget, debug build; native pending |

New command ids this sprint: `engine.plugin_capabilities`, `engine.host_faults`, `session.plugin_versions`,
`mix.insert_sidechain` (engine); `file.bounce_mix`/`file.bounce_stems` gained `tail` and the
failed-plugin refusal; plugin ids accept the `sandbox:` prefix; worker entry points
`soundcraft --plugin-worker <id> [dirs…]` / `soundcraft-cli plugin-worker <id> [dirs…]`.
