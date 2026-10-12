# Reliability audit (R05, Remote Continuation 3.0 Sprint 1)

Written 2026-10-12 from the source at base `b774674` (= `9f9b4d3` + package install) plus the
Sprint 1 working tree, environment P (Linux x86_64 container, no audio device, no display).
Every row names the code and the test that holds it. "INSPECTED" means read in source; "RERUN"
means the named test ran in this sprint's gate (`evidence/sprint_01/sprint1-ci.log`). Nothing here
was observed on the owner's Windows machine (W) or real hardware (H); those rows say so.

## 1. Never-crash rules

| Control | State | Evidence |
|---|---|---|
| `#![deny(clippy::unwrap_used, expect_used, panic, unimplemented, todo, unreachable)]` at every crate root | All 15 library crates and the 3 apps. **Gap found and fixed:** `crates/remote` had no deny line (no violations once added; clippy clean). `xtask` is build tooling, exempt as before. | INSPECTED (grep over `crates/*/src/lib.rs`, `apps/*/src/main.rs`); RERUN clippy `-D warnings` |
| Tests may unwrap | `clippy.toml`: `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-panic-in-tests` | INSPECTED |
| `unsafe` confined to the FFI host crates | Workspace `unsafe_code = "forbid"`; only `au-host`, `clap-host`, `vst3-host` re-allow it on their `ffi` module. Unsafe blocks / `// SAFETY:` comments: au 34/35, clap 92/97, vst3 115/117 (counted by grep: every block has a comment; some comments cover `unsafe impl`) | INSPECTED |
| `catch_unwind` boundaries | engine command execution (`crates/engine/src/lib.rs`), symphonia decode (`crates/audio-io/src/symph.rs`), video worker and H.264 decoder, VST3 module exit (`crates/vst3-host/src/ffi.rs`); the logging test uses one | INSPECTED |
| Panic report | `install_panic_hook` logs `panic: {info}` through the app logger, then runs the previous hook (stderr) | INSPECTED; redaction RERUN (§3) |

## 2. Containment and recovery

| Control | State | Evidence |
|---|---|---|
| Safe Open (`--safe`) | Hosted plugins disabled for the run, slots pass audio and keep state | `safe_open_disables_hosted_plugins_only` (mix) RERUN |
| Unclean-exit notice + autosave generations | Running marker; generations committed atomically; corrupt newest falls back | `autosave_worker_commits_generations_and_marks_unclean_exit` (ui), `faults_at_every_boundary_keep_the_previous_generation`, `corrupt_newest_generation_falls_back`, `retention_and_budget_keep_the_newest`, `recorded_media_is_written_once_and_recoverable` (engine autosave) RERUN |
| Capture durability | Bit-exact for irregular callbacks; explicit overflow/writer errors; crash keeps exactly the durable frames; unfinished takes recoverable; no overwrite | `crates/playback/src/capture.rs` tests (`crash_recovery_keeps_exactly_the_durable_frames`, `unfinished_takes_are_discoverable_and_recoverable`, …) RERUN |
| Isolated plugin scanning | Worker processes, fingerprint cache, quarantine | `isolated_scan_contains_faults_and_caches`, `worker_main_rejects_non_bundles` RERUN |
| Opt-in sandboxed hosting (`sandbox:<id>`) | Crash and hang contained (silence, logged, restart with state unless recording); never waits unboundedly (half a block live, 3 s offline then Hung) | `crates/engine/tests/sandbox.rs` RERUN; numbers in `evidence/sprint_01/latency.csv`, `fault.log` |
| **New:** export refuses sandbox dropouts | Before this sprint a worker that crashed mid-render (auto-restart on) produced an export with a silent gap and no warning. Now every substituted block is counted (`Plugin::substituted_blocks`), `RenderReport::faulted_plugins` lists them and `file.bounce_mix` refuses, writing nothing; `acknowledge_missing` does not cover it | `export_refuses_a_worker_fault_mid_render` (sandbox harness) RERUN |
| Hosted plugins in-process by default | No crash containment for the default path (E03 decision: isolation costs one block of latency) | stated in `help` limits; unchanged |

## 3. Logs and privacy (R05 redaction)

| Control | State | Evidence |
|---|---|---|
| Log file | `<settings>/logs/soundcraft.log`, two previous kept, 16 MiB cap, bounded pre-attach buffer, audio-thread records never block | existing `logging::tests` RERUN |
| **New:** private paths | Home directory → `~` (both separator styles); folder after `/home/`, `/Users/`, `\Users\` → `<user>` (other accounts, a builder's registry path in a panic location); `Shared`/`Public`/`Default`/`All Users` kept | `private_paths_and_secrets_are_redacted` RERUN |
| **New:** secrets | Value after `token`, `secret`, `password`, `passwd`, `api_key`, `apikey`, `authorization` followed by `=`/`:` → `<redacted>` (whole `Authorization: Bearer …` value) | same test |
| **New:** applies everywhere the file is written | Ordinary records, the audio thread's held record (redacted when written, off the audio thread), the engine's panic report | `the_log_file_and_panic_reports_are_redacted` RERUN |
| Opt-out | `SOUNDCRAFT_LOG_PATHS=full` keeps paths for a developer's own debugging; secrets stay hidden | same test (`full` case) |
| Remote tokens | Tokens travel in an HttpOnly cookie or bearer header, never in URLs, never logged (crate doc); the start log line carries the URL only | INSPECTED `crates/remote/src/lib.rs`, `apps/soundcraft/src/main.rs:274` |

Residual, stated: the previous panic hook still prints the unredacted message to standard error
(not persisted by the app; a user who redirects stderr to a file gets it). Messages that
contain a username outside a profile path (for example a device named after the user) are not
recognised. Redaction is pattern-based, not a guarantee; the log is still the user's to review
before attaching it.

## 4. What remains outside P

- Device loss, driver reset and long-run endurance on real interfaces (H).
- Windows-specific paths (`C:\Users\…` redaction is unit-tested with Windows-style strings, not
  on Windows) (W).
- The 64-frame realtime behaviour with a realtime-priority audio thread (N/H): see
  `evidence/sprint_01/callback-profile.csv` (P: compute p99 ≈ 6 % of budget; preemption misses
  in the no-work control).
