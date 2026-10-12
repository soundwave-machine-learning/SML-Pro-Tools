# Resume point — Remote Continuation 3.0

Updated 2026-10-12 after Sprint 1. Branch `fable/recording-core`, local commits only (no push:
the package authorises local commits; push/merge/publication are the owner's decision).

| Step | State | Commit |
|---|---|---|
| Package install (X01) | done, verifier PASS | `b774674` |
| Sprint 1 RC01–RC03 | done; gate exit 0, 683 passed | `fix: close recording-core software safety gaps` (HEAD after amend) |
| Sprint 2 RC04–RC06 | done; gate exit 0, 698 passed (attempt 4) | `0c0da65` frozen eval, `51f72ff` first run, `b77e151` milestone, `b91462b` |
| Sprint 3 RC07–RC09 | done; gate exit 0, 700 passed (attempt 3) | `48e58b6`, `f8163e9` milestone, `bc9cacd` |
| Sprint 4 RC10–RC12 | done; gate exit 0, 704 passed; packages rebuilt from `b641b5a` and attached | `59aa76b` X05 frozen, `b641b5a` candidate milestone, closing docs commit |

Next safe action: the native checklist in `TARGET_PENDING.md` (start with T10 Windows unzip +
WASAPI, T12 VST3 editor on Windows or the manual `windows-native-tests.yml`, T01, T06). Push,
merge and release remain the owner's decisions; nothing was published. Candidate source for
the attached packages: `b641b5ad938d47a9387f611cf717f9933c7cdb19`.
Gate command: `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only cargo xtask ci`
(full debug info no longer fits the disk allowance). Disk: prune `target/debug/incremental` and
superseded test binaries if free space drops under ~3 GB.
