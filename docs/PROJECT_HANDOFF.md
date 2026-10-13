# Project handoff — SML Recording (package revision 1.6, executed)

Project: SML Recording / soundwave-machine-learning/SML-Pro-Tools.
Phase: **four sprints executed autonomously; reviewable candidate; release NOT ready.**
Branch: `fable/recording-core` (local commits only — not pushed, nothing merged; `main` untouched).
Base: `7414719f3760de1c9359ec9723351a7232442e61` (= planning SHA).
Final source commit: see the "Checkpoint" line at the bottom (written after committing).

## Latest: Hotfix Pass 02 (HF07–HF13), 2026-10-13
- Checkout `/home/user/SML-Pro-Tools`, branch `fable/recording-core`, started at `cde8ae2`
  (in sync with origin, clean). Code commits `9b06131`..`30df4a9`, then documentation. Local
  commits only: not pushed, merged, published or installed.
- Report: `docs/hotfix_pass_02/HOTFIX02_REPORT.md`; native gates: `docs/hotfix_pass_02/TARGET_PENDING.md`.
- State: all seven tickets implemented and tested portably. HF13 PARTIAL (FabFilter outcome
  unknown). HF07 ASIO and WASAPI exclusive BLOCKED (SDK licence and build machine / new backend).
  Every native check pending. Release ready: **NO**.
- Next action: owner runs TARGET_PENDING on Windows with the staged candidate
  (`dist/hotfix02-windows`, delivered as attachments).

## State by target
| Target/readiness | State |
|---|---|
| Prompt package | verified (`tools/verify_prompt_package.py` PASS), integrated under `docs/` with provenance in `docs/package_provenance/` |
| Portable build/tests | **PASS** — `cargo xtask ci` exit 0, 616 tests (final: `docs/evidence/sprint_04/regression.log`) |
| Feature coverage | A1–A4 executed rows PASS; named PARTIAL rows in `docs/AUTONOMOUS_BUILD_FINAL_REPORT.md` |
| Windows artifact | unsigned cross-built zip (mingw) from `7838032` in `dist/release/` — **never run on Windows** |
| Linux artifact | tarball from `7838032` in `dist/release/` — clean-extraction headless CLI smoke PASS |
| Native driver/plugin tests (H1–H3) | **NOT EXECUTED — no target access** |
| Physical latency/calibration/stability | **NOT EXECUTED** |
| Owner UX/audio review | **PENDING** |
| Native summing/tape models | **DEFERRED BY OWNER** |
| Signed/published/installed | No (outside authorization) |
| Release ready | **NO** |

## Sprint 4 follow-up (after the first checkpoint `d41d690`)
CL05 optimizations (adaptive strip parallelism, per-block structure hash skipped in playback,
render allocation guard, replaced sessions freed off the audio thread); carried PARTIAL rows
closed in software (input device picker, slot I/O meters, Visual list badges/rename, loop-record
timestamp placement, automation arbitration, V01/V03/V13); review-found H1 risks fixed (input
channel substitution in recording and monitoring, multi-input take read-back memory, device loss
for input and output, resampled-take notice); the upstream Count Off control made to work; small-
screen layout (Console/toolbar scrolling). Details: `docs/sprint_reports/SPRINT_04_REPORT.md`.
Open: 64-frame deadline misses in the VM profile (unproven cause), CL05 residue (graph `sync`
on structure change), N07 keypad, native everything.

## Where things are
- Reports: `docs/sprint_reports/SPRINT_0{1..4}_REPORT.md`, `docs/AUTONOMOUS_BUILD_FINAL_REPORT.md`.
- Evidence: `docs/evidence/sprint_0{1..4}/` (logs, CSV/JSON, UI renders).
- Commands: `docs/evidence/sprint_01/validation-command-index.md`.
- User manual: `docs/USER_GUIDE.md`. Target procedure: `docs/TARGET_MACHINE_GAUNTLET.md`.
- Key new code: `crates/playback/src/{capture,clock,device}.rs`, `crates/engine/src/cmd/{nudge,bus,snapshot,slots}.rs`,
  `crates/model/src/{nudge,slots}.rs`, `crates/dsp/src/{mixmath,removed_meta}.rs`, `crates/mix/src/lib.rs`
  (graph v2, slots, profile, f64 sums), `crates/ui-egui/src/{visual_mixer,nudge_keys,plugin_scan}.rs`.

## Artifacts are not in git
`dist/release/` is gitignored and lived only in the (ephemeral) build container; both files were
attached to the session output. Their sha256 values are in `docs/evidence/sprint_04/artifact-manifest.json`.
To rebuild: the Linux package and Windows cross-build rows of
`docs/evidence/sprint_01/validation-command-index.md` (rebuilding may not be byte-identical).

## Environment notes for the next run
Container needed `libasound2-dev pkg-config libudev-dev mesa-vulkan-drivers libvulkan1` and the
`wasm32-unknown-unknown` target for `cargo xtask ci`; `gcc-mingw-w64-x86-64` + target
`x86_64-pc-windows-gnu` for the Windows cross-build. Use `CARGO_INCREMENTAL=0` and prune
`target/debug/deps` executables: the disk allowance filled once (linker bus error).

## Resume rules
Inspect the branch history and reports before acting; never infer completion from this file.
Unfinished rows stay PARTIAL until real evidence exists. Native results must come from the target
machine. Next action: `docs/AUTONOMOUS_BUILD_FINAL_REPORT.md` → "Exact next action".

## Checkpoint
Final report/handoff source commit: `c74ac7a6ef2e8c2e4dd46f68940d72af53e13af6` (2026-10-11 ≈19:47Z) on `fable/recording-core`;
artifacts built from `7838032e5d42a192e8355b108247d3a7d882fb22` (later commits are docs only).
Final regression: `cargo xtask ci` exit 0, 616 passed, 0 failed. Not pushed; nothing merged.
This line is added in a follow-up commit (a commit cannot contain its own SHA).
