# Four-sprint dependency plan — revision 1.6
Planning only. Exactly four substantive contracts; no completed milestone.

| Sprint | Objective / required work | Prerequisites | Portable gate | Native/owner gates |
|---|---|---|---|---|
| 1 | Source/toolchain baseline; plugin removal; dry disk capture; real device negotiation; sample clock/recovery; fixture command index | Read governing docs; reconcile actual repository/source | A1 | H1 recording/device access later |
| 2 | Vocal tools, takes/comp/punch, exact Nudge, immutable source edits | A1 portable PASS; recorded-media and clock contracts | A2 | N07/09 keyboard/load and edit usability later |
| 3 | Mixer/buses/own plugins, pan profile, Summing/Tape slots, PDC/monitor modes, Console/Visual/Split and exports | A1 and A2 portable PASS | A3 | H2 real plugins; UI/performance/UX later |
| 4 | Final regression, target timing/stability/UX, packaging and evidence handoff | A1–A3 portable PASS for full completion; checkpoint/report work allowed at any stop | A4 | H1–H4 and owner review |

Read each sprint file for tasks/readset/evidence. R01–R26 ownership is in REQUIREMENTS_MATRIX.md. A portable gate excludes explicitly classified native/owner/claim/deferred checks, not unimplemented portable work. Source-test pass is never native-driver PASS. Mandatory portable failures block dependent feature work. If hardware is unavailable, continue through passing portable dependencies and report target readiness blocked. Missing optional commercial DAW comparisons only restrict matching claims.

Six hours of Fable usage is the total cap; if no usage counter is available use a conservative six-hour wall-time proxy and report it. There is no credible per-sprint duration estimate yet. The former 130/100/80/50 guide is withdrawn because scope grew and target stability testing alone exceeds the old final allowance. Preflight records estimated available work from actual baseline findings. Keep all requested features in the coverage report; uncompleted rows are PARTIAL, never silently removed.

Reserve the final 20 minutes for checkpoint, available checks and reporting; do not begin a task that cannot be safely stopped. Native testing needs at least two 30-minute stability configurations plus calibration and packaging time on the real machine. Start those only when a stable build and sufficient budget exist; otherwise provide the exact pending gauntlet. No fake fifth sprint or extension of the cap. At any stop preserve work and write the actual final status even if Sprint 4 was never entered.

Implementation priority within dependencies: clock/capture integrity -> edits/Nudge -> graph/host/compensation -> clean pan profile/slots -> Visual -> final verification. Native summing/tape model algorithms, M-T3/4/8 and detailed tape-machine controls are DEFERRED BY OWNER, not unfinished current code. No delegated agents by this prompt.
