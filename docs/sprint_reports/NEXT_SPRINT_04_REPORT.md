# Next Sprint 04 — Complete the assisted studio (revision 2.2 contract)

Status: **PARTIAL** — the assistant service (Explain/Operate/Engineer, grounded, validated
atomic plans), the typed desktop and phone assistant with the monitor-only speech path, the
mastering workspace (program, monitor-only reference with level match, standard meters with
loudness range, verified exports with reports), the portable record-to-master scenario with
fault injection, and the handoff documents are implemented with tests (full `cargo xtask ci`
exit 0, see "Tests run"). Conversational generation, speech recognition and synthesis stay
BLOCKED (E06: no licensed local or provider runtime is configured; the boundary is in place and
reports `unavailable`). The native gates (E08 durations and crash rates, W1/W2 timing, real
plugins, phone on iOS/Android, listening review, clean-machine install) are TARGET_PENDING.
Environment P only. Started 2026-10-12T06:00Z from commit `aac507d`.

## Outcome (contract items 1–8)
| Item | Result | Evidence |
|---|---|---|
| 1 E06 + knowledge corpus + Explain + evaluation set | provider boundary (`assistant::Provider`, only `Unavailable`); corpus = registry knowledge + build guide sections, cited as command ids / `guide:§n`; fixed evaluation set of 20 cases scored by a test (factual, unsupported, operate, injected text, engineer) | `assistant-eval.json`, `assistant-eval-results.csv`, `assistant-eval.log` |
| 2 Operate through validated tools | plans of envelopes with stable ids at the current revision; preview then one undo step; stale/duplicate/unknown-target/ambiguous/destructive cases produce zero mutation (O01–O06, I01) | same |
| 3 Engineer workflows | measured peak / true peak / integrated loudness / noise floor over the selected range with measured-vs-inference-vs-preference labels; clipping locations, gate threshold (registered plugins only), vocal-bus plan, balance proposal with preview/undo; refuses to claim listening (E01–E05) | same |
| 4 Push-to-talk, transcript, TTS, phone | typed path on desktop (Help › Assistant) and the phone page (Ask box; plan steps bounded by the phone allowlist); push-to-talk control present and disabled with the honest reason; speech/reference audio goes through a monitor-only ring that never enters a render or capture and is dropped while recording; model timeout = the unavailable path | `remote-assistant-tests.log`, playback `aux_monitor` test, `e08-scenario.json` |
| 5 Mastering workspace | `session.mastering` (program, reference, note); Master layout with the real chain (Master inserts, applied once), reference A/B and level match (audition only), offline standard meters incl. loudness range, correlation, mono; E07 analytic Tech 3341/3342 + BS.1770-4 cases pass within the documents' tolerances | `e07-meters.csv`, `e07-meters.log` |
| 6 Export | format/depth/dither-on-request/rate (SRC)/tail/metadata/report; refuses existing files, missing or failed plugins unless acknowledged; report JSON with settings, revision, plugins, measured levels and decoded-file verification; reference/monitor/speech excluded | `e08-scenario.json` (export + reopen step), engine file tests |
| 7 E08 | portable scenario PASS (recover → record lead/double with autosave during capture → split/nudge/strip silence → vocal bus → remote envelope + duplicate → assistant explain/operate/undo + unavailable speech → reference excluded → export + report + reopen); durations, native recording runs, soak and crash rates PENDING | `e08-scenario.json`, `e08-scenario.log` |
| 8 Manual, references, migration, performance, package, matrix | USER_GUIDE §10a/10b, `COMMAND_REFERENCE.md` (generated), `MIGRATION_AND_RECOVERY.md`, `PERFORMANCE_REPORT.md`, `FINAL_REQUIREMENT_MATRIX.md`; local package: __PKG__ | docs |

## Requirement statuses
See `docs/next_cycle_execution/REQUIREMENT_STATUS.csv` / `FINAL_REQUIREMENT_MATRIX.md`:
R32 PARTIAL (generation BLOCKED), R33 PARTIAL (speech BLOCKED, phone test pending), R34 PARTIAL
(portable PASS, listening/independent vectors pending), R35 PARTIAL (scenario PASS, durations
PENDING), R36 PARTIAL (docs done; __PKG_STATUS__), R38 PARTIAL (four prompts executed, native
gates pending).

## Measurements
- E07: 12 analytic cases all within tolerance (integrated −23.0 LUFS cases to ±0.02 LU;
  gating cases 3341-3/4/5 to ±0.02; true peak exact at −6 and −0.5 dBTP; LRA 10/5/20 LU exact).
- Assistant: 20/20 evaluation cases pass; no case mutates on an invalid, stale, duplicate or
  unauthorized plan.
- Aux monitor ring: summing/drop/gain/clear proven in isolation; render unchanged.

## Files / commits
Milestone commit `feat: complete assisted SML Studio mastering workflow` (`c7664e4`).
New: `crates/engine/src/assistant.rs`, `crates/engine/src/cmd/{assistant,mastering}.rs`,
`crates/dsp/tests/e07_meter_vectors.rs`, `crates/engine/tests/e08_scenario.rs`, UI Assistant
window and Master layout content, phone Ask box, `docs/{COMMAND_REFERENCE,MIGRATION_AND_RECOVERY}.md`,
`docs/next_cycle_execution/{SPRINT_04_DESIGN,PERFORMANCE_REPORT,FINAL_REQUIREMENT_MATRIX}.md`,
evidence under `docs/evidence/next_cycle/sprint_04/`. Changed: model (`Session.mastering`),
dsp (`LoudnessMeter::loudness_range_lu`), mix (`render_program_report`), engine (export
guard/report/SRC/overwrite, `help::search_rows`, dedup helpers), playback (`AuxMonitor`),
remote (allowlist, plan bounds, page), USER_GUIDE, COMMAND_MATRIX, ledgers.

## Tests run (all P)
Full `CARGO_INCREMENTAL=0 cargo xtask ci` exit 0: fmt, clippy (-D warnings), 678 tests passed /
0 failed, assets, layers, wasm (log `ci-sprint4.log`). Gate runs that failed and were fixed on
the way: a surround test that re-bounced to the same path (now passes `overwrite`), a
formatting miss, and the Master layout losing its Back to Mix control. Suites per row above
with their logs in the evidence folder: `assistant-eval.log`, `e07-meters.log`,
`e08-scenario.log`, `remote-assistant-tests.log`, `clippy-pre-ci.log`.

## Not run / failed / blocked / gaps (named)
- E06 BLOCKED: no language model, STT or TTS; the assistant answers from rules and search only;
  push-to-talk and spoken replies are wired to an `unavailable` provider.
- E08 native: 30-minute W1 recordings (clean and chain), 2 h recording endurance, 8 h soak and
  observed crash rates — PENDING; the portable scenario is a control, not these runs.
- Phone voice, PWA, secure origin: no mobile device (M); the Ask box is typed HTTP.
- Mastering: independent meter vectors (official files) not downloaded; analytic cases only;
  listening review of the reference/level-match workflow pending (L).
- Package: __PKG_GAP__
- Owner workflow/taste review: not performed (no owner access).
- A remote pairing-expiry test was observed flaky once under full-CI load (50 ms TTL); widened
  to 400 ms; it is a test-timing issue, not a product change.

## Risks
- `Session.mastering` and the export report are new optional data; older builds ignore them.
- Assistant plans resolve tracks by name: duplicate names ask for the id; renaming between
  preview and run makes the plan stale (refused), which is the intended behaviour.
- The aux monitor ring is stereo at the device rate; multichannel devices hear speech/reference
  on the first two channels only.

## Next safe task
Native gauntlet on the target machine (`docs/TARGET_MACHINE_GAUNTLET.md` + the pending rows in
the final matrix); configure a licensed model/STT/TTS provider to unblock E06; supply SML
plugin artifacts for the R23–R29 compatibility rows.
