# Four-sprint roadmap

Each sprint is a cohesive milestone with internal checkpoints, not a fixed number of hours. Start the next dependent task only after its named interface and correctness gates pass. Entire sprint completion is reported separately from permission to work on an independent later task.

| Sprint | Required outcome | Included work | Completion evidence |
|---|---|---|---|
| 1: Protect the session | A recoverable, measurable recording foundation | Current-build reconciliation; clock/latency/dry capture; atomic autosave and media recovery; scanner and plugin lifecycle; runtime sandbox experiment; Safe Open; command/parameter contracts; research feasibility | Current HEAD and build commands; gap map; fault-injection recovery; baseline timing/load results; experiment decisions |
| 2: Finish the studio workflow | Record/edit/mix with familiar controls and phone operation | Takes/comp/punch/fades/gain/strip silence; all audio toolbar tools; dedicated Nudge; Pro Tools/Studio One mappings; easy buses; Console/Visual/Split; slots; layouts/themes/fast menus; LAN remote; actual help registry | Workflow fixtures and native walkthroughs; mapping inventory; routing/null/PDC tests; UI timing; phone security and reconnect tests |
| 3: Harden plugin integration | A production-ready personal-plugin workflow | Scan/editor/state/automation/sidechain/PDC/offline lifecycle; real plugin compatibility; future SML integration assessments; bounded native edit rendering | Host test matrix; plugin timing/latency/load evidence; safe failures; actual interface assessments |
| 4: Complete the assisted studio | Operable expert assistant and verified record-to-master release candidate | Knowledge/explain mode; checked engineer actions; text and voice; remote assistant; mastering/reference/meters/export; integrated fault tests/soak; documentation/installable package where authorized | Grounding/action/voice tests; export verification; complete song walkthrough; native reliability report; honest release status |

## Dependency gates
- G0: current repository identified, user changes protected, real commands discovered. Required before implementation mutations.
- G1: capture, persistence/recovery and command transaction invariants pass portable tests. Required before new editing/remote mutation pathways.
- G2: shared commands, parameter IDs, route graph and non-destructive edit/render interfaces validated. Required before processor UI integration and assistant action execution.
- G3: generic host lifecycle, routing, automation/state and measured delay pass. Actual supplied plugins have separate compatibility records before assistant recipes use them; missing future products do not block DAW release.
- G4: assistant provider/action gates, mastering export gates and native reliability gates passed. Required for full release claim.

Native hardware tests can remain pending while portable work proceeds; the native product stays unvalidated. Missing an optional vendor reference blocks a matching-sound claim, not a functioning original design with its own passed criteria. Missing pitch-preserving render blocks Bend/timing completion; a moving marker alone is not a working tool. Missing an AI model blocks generative/voice completion; searchable manuals alone do not satisfy the assistant.

Critical sequence: session integrity -> commands/parameter contracts -> editing and routing -> host integration/edit rendering -> AI engineering actions and final mastering validation. UI shells, documentation and evaluation fixtures may proceed independently, but are reported as such.

The seven plugin-development projects are external to this cycle. Keep four resumable DAW prompts, validate host behavior with available real plugins, and assess future integrations only against supplied artifacts. Do not substitute proprietary model development for host reliability work.
