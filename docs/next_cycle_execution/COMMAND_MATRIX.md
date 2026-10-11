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
