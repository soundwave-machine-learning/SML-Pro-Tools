# Experiment decisions (E01–E08) — real entries only

Entries are appended as experiments actually run. "Decision" rows cite the evidence file. Nothing
here is prepopulated as PASS.

## E01 — What exists and how it is built (G0)
- Checkpoint verified 2026-10-11T23:35Z: branch `fable/recording-core`, HEAD
  `396fd0a7a7aa5b62d8dae0e49b3413c7e2e3432b`, clean tree, `main` at `7414719…` untouched, remote
  `origin` = github.com/soundwave-machine-learning/SML-Pro-Tools (branch never pushed).
- Artifact source `7838032` → HEAD diff: 10 files, all under `docs/` (reports, logs, handoff). No
  source change after the artifact build (verified with `git diff --name-only 7838032..396fd0a`).
- Commands: see COMMAND_MATRIX.md. Baseline rerun 2026-10-11T23:37–23:42Z: `CARGO_INCREMENTAL=0 cargo xtask ci` exit 0 — fmt, clippy, test (616 passed, 0 failed, 57 test binaries), assets, layers, wasm; 4m33s wall (`docs/evidence/next_cycle/sprint_01/baseline-ci.log`). This is RERUN evidence; the reported 616 is confirmed by the current count.
- Environment: `docs/evidence/next_cycle/sprint_01/environment.json` (Linux container, no audio
  device, no display, no Windows host, no owner plugins → P only).
