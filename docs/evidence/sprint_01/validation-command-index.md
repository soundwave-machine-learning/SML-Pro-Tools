# Validation command index (established Sprint 1, extended in later sprints)

All commands run from the repository root on the execution host recorded in `environment.json`.
"A" = portable software gate; "H" = native/owner gate (cannot run in this container).

| Gate / check IDs | Real command | What it runs |
|---|---|---|
| Full CI (every milestone) | `cargo xtask ci` | fmt check, `clippy --workspace --all-targets -D warnings`, `cargo test --workspace`, assets, layers, `cargo check --target wasm32-unknown-unknown` |
| A1 capture (identity, frame order, irregular callbacks, unique files) | `cargo test -p soundcraft-playback capture::` | `capture::tests::*` |
| A1 recovery (overflow, disk full, crash, bounded ring; disconnect added in Sprint 4) | `cargo test -p soundcraft-playback -- overflow writer_error crash_recovery ring_memory disarmed disconnect` | |
| CL-T1 / CL-T3 primitives / CL-T5 load metric | `cargo test -p soundcraft-playback clock::` | 30-min simulated variable callbacks, generations, offsets, punch, ledger signs, load denominator |
| CL-T2 negotiation | `cargo test -p soundcraft-playback device::` | fixture negotiation + real enumeration smoke |
| CL-T2 real capability audit | `cargo run -p soundcraft-playback --example device_audit` | JSON of hosts/devices this build sees |
| CL02 live path (no queued audio, variable callbacks) | `cargo test -p soundcraft-playback live_path` | |
| A1 catalogue / migration (R04/R05) | `cargo test -p soundcraft-engine -- missing no_bundled unknown_clap demo::tests` and `cargo test -p soundcraft-mix -- inserts_process delay_compensation` | |
| N01–N06, N08 (A2) | `cargo test -p soundcraft-engine cmd::nudge` | |
| N07 dispatch/focus (A2, unit level) | `cargo test -p soundcraft-ui-egui nudge_keys` | key-map unit tests (physical keypad = H) |
| B-T1–B-T6 (A3) | `cargo test -p soundcraft-engine cmd::bus` | |
| VM-T5 snapshots, width command (A3) | `cargo test -p soundcraft-engine cmd::snapshot` | |
| M-T2 pan law, VM-T3 width math, VM02 span rule (A3) | `cargo test -p soundcraft-dsp mixmath` | |
| SL-T / CL-T4 / VM-T2 engine graph fixtures (A3) | `cargo test -p soundcraft-mix sml_graph` | slots, width, profile, PDC through slots |
| Visual mixer mappings (A3, UI unit) | `cargo test -p soundcraft-ui-egui visual_mixer` | |
| Vocal coverage (A2) | `cargo test -p soundcraft-engine vocal_` | V-row fixtures added in Sprint 2 |
| CL03 tracking, slot state, V13 exports, audition | `cargo test -p soundcraft-engine cmd::slots` | |
| SL02 Post inserts, CL04 latency event | `cargo test -p soundcraft-mix -- sml_graph_post latency_change` | |
| CL04 timestamp placement | `cargo test -p soundcraft-playback -- timestamp_placement playback_anchor` | |
| SL-T6 real CLAP fixture in a slot | `cargo test -p soundcraft-engine --test clap_e2e` | builds the repo CLAP fixture |
| Background plugin scan | `cargo test -p soundcraft-ui-egui plugin_scan` | |
| CL-T5 render allocation guard, serial/parallel equality (A4) | `cargo test -p soundcraft-mix --test realtime_alloc` and `cargo test -p soundcraft-mix serial_and_parallel` | `stats_alloc` global allocator |
| Follow-up batch (A4): input picker, slot I/O meters, list badges, loop placement, arbitration, session drop | `cargo test -p soundcraft-playback -- input_request replaced_sessions`; `cargo test -p soundcraft-mix sml_graph_slot_io`; `cargo test -p soundcraft-ui-egui -- failed_input_switch route_badge`; `cargo test -p soundcraft-engine -- loop_passes vmt5_snapshot_recall` | |
| CL-T3 click/tempo placement, device loss (A4 follow-up) | `cargo test -p soundcraft-mix click_placement`; `cargo test -p soundcraft-playback -- disconnect output_loss` | |
| Headless callback/response profile (A4, not native) | `cargo run --release -p soundcraft-engine --example sml_profile -- callback.csv response.csv` | |
| UI renders | `cargo run -p soundcraft-ui-egui --example ui_shot -- out.png script.jsonl --size 1600x1000` | needs a Vulkan driver (lavapipe here) |
| Linux package | `cargo build --release --locked -p soundcraft -p soundcraft-cli && packaging/linux/package.sh --skip-build --formats tar` | |
| Windows cross-build (not a runtime test) | `cargo build --release --locked -p soundcraft -p soundcraft-cli --target x86_64-pc-windows-gnu` | needs mingw-w64 |
| H1–H4, CL-T6/7, N07 physical, N09, VM-T7/8, SL-T8, M-T7 | not runnable here | need Windows x64 target, interface/driver, owner plugins, owner review — see `docs/TARGET_MACHINE_GAUNTLET.md` |

Rows added after Sprint 1 are marked by their gate; each was executed when its sprint ran
(see the sprint reports for actual results). A row is never PASS by being listed here.
