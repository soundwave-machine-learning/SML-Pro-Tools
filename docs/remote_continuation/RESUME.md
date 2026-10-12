# Resume point — Remote Continuation 3.0

Updated 2026-10-12 after Sprint 1. Branch `fable/recording-core`, local commits only (no push:
the package authorises local commits; push/merge/publication are the owner's decision).

| Step | State | Commit |
|---|---|---|
| Package install (X01) | done, verifier PASS | `b774674` |
| Sprint 1 RC01–RC03 | done; gate exit 0, 683 passed | `fix: close recording-core software safety gaps` (HEAD after amend) |
| Sprint 2 RC04–RC06 | next | — |
| Sprint 3 RC07–RC09 | pending | — |
| Sprint 4 RC10–RC12 | pending | — |

Next safe action: Sprint 2 — freeze `evidence/sprint_02/heldout-eval.json` first, then the
local-endpoint language provider (inference BLOCKED in P, X03), built-in TTS/STT, routing proof.
Gate command: `CARGO_INCREMENTAL=0 cargo xtask ci`. Disk: prune `target/debug/incremental` and
superseded test binaries if free space drops under ~3 GB.
