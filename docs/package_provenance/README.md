# Planning package provenance

`SML_Recording_Four_Sprints.zip` revision 1.6 (sha256 in `ZIP_SHA256.txt`) was extracted to a
staging directory and verified with its own `tools/verify_prompt_package.py` before integration:
`PASS: revision 1.6, 28 files, four substantive contracts, R01–R26 coverage, hashes and reference
checks. Product tests NOT run.` (2026-10-11).

Its documents were copied into `docs/` (and `docs/sprint_prompts/`; `START_HERE.md` →
`docs/SML_START_HERE.md`, `CHANGELOG.md` → `PACKAGE_CHANGELOG.md` here). `PACKAGE_MANIFEST.json`
and the verifier are archived here unchanged. Living docs (e.g. `docs/PROJECT_HANDOFF.md`) are
updated during execution, so the manifest hashes describe the original package only; do not run
the verifier against the living docs and treat expected hash changes as a regression.
