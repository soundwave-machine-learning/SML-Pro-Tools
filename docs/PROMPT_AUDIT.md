# Q-DRE prompt audit — revision 1.6
Date: 2026-10-11. Reviewer: primary assistant using Q-DRE and Build Reviewer guidance. This is a self-review of the authored package, not an independent external certification.

## Verdict
Incoming revision 1.5: NOT READY to hand directly to Claude unchanged because active instructions conflicted. Corrected revision 1.6: READY FOR CONDITIONAL PREFLIGHT AND STAGED IMPLEMENTATION. Full completion within six hours is NOT established. Product build/release readiness is NO; no DAW implementation or target test was run in this audit.

## Evidence inspected
All 24 incoming files were read, including four contracts, master, every technical/research spec, matrix/brief/handoff and manifest. Manifest and ZIP bytes were checked against the local current revision before edits. Historical pinned source observations were reviewed as historical evidence, not re-resolved current HEAD. Avid's 2019.5 shortcut table and dedicated 12.3 trim/selection-boundary pages were reopened to resolve the unsafe shortcut claim. No broad vendor research restart was needed.

## Findings and corrections
| ID | Severity / incoming failure | Correction / remaining limit |
|---|---|---|
| AUD-01 | P0: amendment stacks and multiple “current” versions; stale Console processing instructions | Rewrote brief/plan/master/handoff/all four contracts into one revision; assigned domain authorities and removed active model instructions |
| AUD-02 | P0: withdrawn native-model quality/listening gates could block current slots | Current M-T1/2/5/6 software and M-T7 native separated; M-T3/4/8 explicitly deferred; no native sonic model code requested |
| AUD-03 | P0: post-pan tap conflicted with independent send pan, risking double pan or impossible inversion | SL02 specifies pre-pan pre/post-fader taps and one panner per route; noncommuting/one-sided fixtures verify order |
| AUD-04 | P1: rigid stereo span at [-1,+1] permitted zero horizontal movement | VM02 defines visible bounded center/span behavior; full-width node can pan and reports narrowing |
| AUD-05 | P1: M/S width arithmetic at unity could alter samples despite identity claim | Exact pass-through at settled 100%; arithmetic probe showed a 1e-8 opposite channel lost with float sums at unity |
| AUD-06 | P1: compensated pan could boost every new nested stereo bus | Explicit neutral bus/Main defaults and same-profile identity comparisons; legacy graph/arithmetic versioning |
| AUD-07 | P1: unspecified shared-return solo, pre-fader mute and legacy graph migration | Defined new-session mute/solo-safe/source propagation; preserve legacy until explicit migration or report compatibility blocker |
| AUD-08 | P1: vague “dry,” monitoring overrides and offline-only bypass policy could change exports | Named Raw/Edited Dry/Processed/Full Mix paths; both realtime/offline use declared normal graph; monitor differences visible |
| AUD-09 | P1: A2/A3 mixed native keyboard/performance/owner tests into portable continuation | Central classification separates A software, H native, claim-only and deferred checks while retaining all features |
| AUD-10 | P1: 10/30-minute tests, transport-stop stress and 50-minute final allowance conflicted | Two uninterrupted 30-minute configurations; separate transport stress; withdrew unsupported sprint-minute estimates, retained total six-hour cap |
| AUD-11 | P1: callback “load” based on observed interval could reward scheduler stalls | Fixed denominator N/Fs; separate compute/deadline/interval jitter, no percentile excuses for xruns |
| AUD-12 | P1: recording loopback and monitoring delay could be conflated; calibration could double-count | Separate measurement topology/uncertainty, output/input ledger/sign, raw-file preservation and known-delay validation |
| AUD-13 | P1: Shift+Numpad advertised as PT selection-only without matching evidence | Reference preset leaves this chord unassigned; explicit SML command remains; confirmed dedicated trim/boundary mappings |
| AUD-14 | P2: fractional nudge reset/undo and below-sample behavior underspecified | Defined per-target logical anchor/cumulative count/ties-to-even, reset triggers and rejected-command behavior |
| AUD-15 | P2: full scope could be lost at early budget stop; no consolidated traceability | Added R01–R26 matrix/index, always-final-report rule and exact package verifier; no fabricated later sprint reports |

P0 = could direct contradictory or materially incorrect implementation. P1 = correctness/readiness risk. P2 = clarity/continuity issue. These are prompt defects, not observed defects in a running product.

## Remaining blockers and provisional decisions
| Item | Status / impact | Resolving action |
|---|---|---|
| Six-hour feasibility | High risk; not established by inspection; original feature list spans engine, editor, host and UI | Claude preflight inventories actual working code and estimates available work; continue within cap with honest partial coverage |
| Native platform | Windows x64 ASSUMED; actual target/hardware/plugins UNKNOWN | Use known session choice if supplied; portable work now, native H validation only with actual access |
| Source baseline/toolchain/CI | Pinned observations historical; no current build or full-tree audit here | Reconcile actual SHA/AGENTS/dependencies and run full baseline in Sprint 1 |
| Exact feature test commands | Not established in target source yet | Discover harnesses, implement tests and record real commands in Sprint 1; missing future command names are not invented blockers before that work |
| Legacy graph compatibility | Required, unimplemented | Preserve legacy execution or report the exact compatibility gap; never silently alter saved mix sound |
| Provisional SML pan/width/mode rules and numeric targets | Defined and implementable; not vendor equivalence or measured performance | Implement/check own contract; run reference/native tests before matching claims |
| Native physical precision/latency | <=10 ms proposed, <=5 ms stretch; analog sample placement depends on measured uncertainty | Actual CL-T6/7; no clock-jitter or HDX claim |
| Native summing/tape algorithms | DEFERRED BY OWNER | Separate later research/approved model contracts, not current gate |
| Owner usability/listening and personal plugins | PENDING | Native candidate review and named-plugin matrix |

These do not require another approval before already-authorized local preflight. They prevent a guarantee that all features or release readiness will be achieved in one six-hour run. No requested current feature was removed to force feasibility.

## Checks actually rerun during this audit
- Incoming manifest SHA256s and ZIP bytes/integrity: PASS for all 24 incoming files.
- Small mathematical probes: confirmed prior full-span rightward allowance was 0; f32 width identity example lost the small right channel; gain/bias composition differs by order. These are specification probes, not DAW tests.
- Corrected formula/identity probes and outgoing package verifier: recorded by the audit-check tool run and tools/verify_prompt_package.py; current manifest covers final bytes.
- DAW build, cargo tests, recording, native plugin lifecycle, acoustic/analog latency, listening, commercial-DAW comparison: NOT EXECUTED.

Current allowable handoff: provide revision 1.6 as a whole and run its master prompt. Do not mix old sprint files into the new package. Product release ready: NO. Prompt ready for conditional preflight: YES.
