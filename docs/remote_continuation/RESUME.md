# Resume point — Remote Continuation 3.0

Updated 2026-10-12 after Sprint 1. Branch `fable/recording-core`, local commits only (no push:
the package authorises local commits; push/merge/publication are the owner's decision).

| Step | State | Commit |
|---|---|---|
| Package install (X01) | done, verifier PASS | `b774674` |
| Sprint 1 RC01–RC03 | done; gate exit 0, 683 passed | `fix: close recording-core software safety gaps` (HEAD after amend) |
| Sprint 2 RC04–RC06 | done; gate exit 0, 698 passed (attempt 4) | `0c0da65` frozen eval, `51f72ff` first run, `b77e151` milestone, `b91462b` |
| Sprint 3 RC07–RC09 | done; gate exit 0, 700 passed (attempt 3) | `48e58b6`, `f8163e9` milestone, `bc9cacd` |
| Sprint 4 RC10–RC12 | started: X05 frozen `59aa76b`; generator draft in session scratchpad | — |

Next safe action: Sprint 4 — X05 meter comparison (examples/meter_vectors.rs + tools/compare_meters.py),
export/durability faults, stress/backpressure, rebuild Linux + Windows packages from the final
source, attach them, final reports.
Gate command: `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only cargo xtask ci`
(full debug info no longer fits the disk allowance). Disk: prune `target/debug/incremental` and
superseded test binaries if free space drops under ~3 GB.
