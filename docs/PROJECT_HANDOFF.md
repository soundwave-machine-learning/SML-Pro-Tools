# Project handoff — SML Recording (package revision 1.6, executed)

Project: SML Recording / soundwave-machine-learning/SML-Pro-Tools.
Phase: **four sprints executed autonomously; reviewable candidate; release NOT ready.**
Branch: `fable/recording-core` (local commits only — not pushed, nothing merged; `main` untouched).
Base: `7414719f3760de1c9359ec9723351a7232442e61` (= planning SHA).
Final source commit: see the "Checkpoint" line at the bottom (written after committing).

## State by target
| Target/readiness | State |
|---|---|
| Prompt package | verified (`tools/verify_prompt_package.py` PASS), integrated under `docs/` with provenance in `docs/package_provenance/` |
| Portable build/tests | **PASS** — `cargo xtask ci` exit 0, 607 tests (final: `docs/evidence/sprint_04/regression.log`) |
| Feature coverage | A1–A4 executed rows PASS; named PARTIAL rows in `docs/AUTONOMOUS_BUILD_FINAL_REPORT.md` |
| Windows artifact | unsigned cross-built zip (mingw) in `dist/release/` — **never run on Windows** |
| Linux artifact | tarball in `dist/release/` — headless CLI smoke PASS |
| Native driver/plugin tests (H1–H3) | **NOT EXECUTED — no target access** |
| Physical latency/calibration/stability | **NOT EXECUTED** |
| Owner UX/audio review | **PENDING** |
| Native summing/tape models | **DEFERRED BY OWNER** |
| Signed/published/installed | No (outside authorization) |
| Release ready | **NO** |

## Where things are
- Reports: `docs/sprint_reports/SPRINT_0{1..4}_REPORT.md`, `docs/AUTONOMOUS_BUILD_FINAL_REPORT.md`.
- Evidence: `docs/evidence/sprint_0{1..4}/` (logs, CSV/JSON, UI renders).
- Commands: `docs/evidence/sprint_01/validation-command-index.md`.
- User manual: `docs/USER_GUIDE.md`. Target procedure: `docs/TARGET_MACHINE_GAUNTLET.md`.
- Key new code: `crates/playback/src/{capture,clock,device}.rs`, `crates/engine/src/cmd/{nudge,bus,snapshot,slots}.rs`,
  `crates/model/src/{nudge,slots}.rs`, `crates/dsp/src/{mixmath,removed_meta}.rs`, `crates/mix/src/lib.rs`
  (graph v2, slots, profile, f64 sums), `crates/ui-egui/src/{visual_mixer,nudge_keys,plugin_scan}.rs`.

## Environment notes for the next run
Container needed `libasound2-dev pkg-config libudev-dev mesa-vulkan-drivers libvulkan1` and the
`wasm32-unknown-unknown` target for `cargo xtask ci`; `gcc-mingw-w64-x86-64` + target
`x86_64-pc-windows-gnu` for the Windows cross-build. Use `CARGO_INCREMENTAL=0` and prune
`target/debug/deps` executables: the disk allowance filled once (linker bus error).

## Resume rules
Inspect the branch history and reports before acting; never infer completion from this file.
Unfinished rows stay PARTIAL until real evidence exists. Native results must come from the target
machine. Next action: `docs/AUTONOMOUS_BUILD_FINAL_REPORT.md` → "Exact next action".
