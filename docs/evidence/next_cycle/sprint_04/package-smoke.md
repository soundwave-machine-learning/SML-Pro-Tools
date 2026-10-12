# Package smoke (next cycle, Sprint 4)

Artifact: `dist/release/soundcraft-0.5.0-linux-x86_64.tar.gz` (not committed; sha256
`94b66f8fbe1b02d06c3024d96fcf3cf4bfac1e1218d22bd812385dceb7946f1b`, built by `packaging/linux/package.sh --formats tar` from commit `bdda836`,
log `package-linux.log`). Smoke log: `package-smoke.log`.

| Check | Result |
|---|---|
| Clean extraction in a scratch folder; `soundcraft-cli --version` | PASS (0.5.0) |
| `commands` lists the registry (528), `plugins` catalogue empty (no bundled effects) | PASS |
| `plugin-worker` entry point present (sandboxed hosting) | PASS (usage printed) |
| Headless script on the demo session: `assistant.provider` (unavailable, honest), `assistant.ask` (grounded Explain with citations), `mastering.status`, `engine.plugin_capabilities` (seven AWAITING_EXTERNAL_PLUGIN rows), `engine.host_faults`, `help.limits`; save; bounce with verification | PASS |
| Reopen the saved session; `file.bounce_mix` with `tail: auto` and `report`; `session.plugin_versions`; `mastering.meters` incl. loudness range | PASS; report JSON written and decoded-file verification `header_ok: true` |
| GUI binary starts far enough to print its version; a window cannot open here (no display) | PASS (version) / NOT POSSIBLE (window) |
| Install into a system prefix, desktop launch, reopen on a clean machine, Windows zip run | NOT RUN — no target machine (R36 native pending) |
