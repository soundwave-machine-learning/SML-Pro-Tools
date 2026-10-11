# SML Recording — audited four-sprint package
Revision 1.6 • 2026-10-11 • prompts and specifications only.

Use this complete package instead of revisions 1.0–1.5. Exactly four sprint contracts remain. Scope is preserved; native summing/tape sound models remain deferred by the owner.

1. Give Claude this ZIP together with the repository URL: https://github.com/soundwave-machine-learning/SML-Pro-Tools.
2. Have Claude extract and run `python3 tools/verify_prompt_package.py` from the extracted package root before integration. This checks document identity, not DAW functionality.
3. Reconcile the package's docs into the repository without overwriting unrelated user files. Existing AGENTS.md and repository instructions must be inspected. Copy the manifest/verifier if useful for provenance; after execution modifies living docs, archive the original package manifest rather than pretending those hashes still describe changed files.
4. Give Claude the complete contents of `docs/AUTONOMOUS_BUILD_PROMPT.md`. It reads and executes all four contracts; no four separate pasted prompts are needed.

Start with `docs/DOCUMENT_INDEX.md` and `docs/PROMPT_AUDIT.md`. All active documents describe one current plan, not stacked amendments. Repository implementation has NOT started in this conversation; no branch, build or audio measurement was produced.

Six hours is the usage cap, not a validated completion estimate. Full scope is ambitious and may end as a resumable partial checkpoint. Windows x64 is the planning target assumption; native driver, toolchain and personal-plugin access are unknown. Portable/headless work may proceed without native hardware, but target readiness stays blocked.

Launching the master prompt authorizes local isolated implementation/testing/commits. No remote push/merge, release trigger, owner-machine installation, purchase or signing is included. Actual summing/tape model research is a later pass.
