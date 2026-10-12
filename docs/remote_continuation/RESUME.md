# Resume point — Remote Continuation 3.0

Updated 2026-10-12 after Sprint 1. Branch `fable/recording-core`, local commits only (no push:
the package authorises local commits; push/merge/publication are the owner's decision).

| Step | State | Commit |
|---|---|---|
| Package install (X01) | done, verifier PASS | `b774674` |
| Sprint 1 RC01–RC03 | done; gate exit 0, 683 passed | `fix: close recording-core software safety gaps` (HEAD after amend) |
| Sprint 2 RC04–RC06 | done; gate exit 0, 698 passed (attempt 4) | `0c0da65` frozen eval, `51f72ff` first run, `b77e151` milestone, `b91462b` |
| Sprint 3 RC07–RC09 | started: `48e58b6` (cross-build script, manual Windows workflow, backend audit) | — |
| Sprint 4 RC10–RC12 | pending | — |

Next safe action: Sprint 3 — Windows HWND editor window (draft in session scratchpad), asio
feature, run cross-build.sh, compact toolbar at 1366×768@150 %, remote secure-origin and
reconnect checks, keyboard fallback.
Gate command: `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only cargo xtask ci`
(full debug info no longer fits the disk allowance). Disk: prune `target/debug/incremental` and
superseded test binaries if free space drops under ~3 GB.
