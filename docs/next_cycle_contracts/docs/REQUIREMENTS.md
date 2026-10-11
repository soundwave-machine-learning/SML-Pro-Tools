# Requirements and completion ledger

All 38 rows remain tracked. Generic host support is mandatory; R23–R29 name future external-plugin integration targets, not seven plugin-build obligations. Actual compatibility checks are conditional on supplied artifacts and use AWAITING_EXTERNAL_PLUGIN status until then. Scope limitations are explicit in DECISIONS.md; research delivery is not sonic parity. Initial implementation status for every row is UNVERIFIED. Sprint 1 replaces that with evidence-backed values. A later sprint revalidates earlier rows it affects.

| ID | Lead sprint | Requirement | Primary test family | Specification |
|---|---:|---|---|---|
| R01 | 1 | Reconcile already-run original four prompts and preserve current code | V-BASELINE | [RELIABILITY_AND_ENGINE.md](RELIABILITY_AND_ENGINE.md) |
| R02 | 1 | Reliable dry capture, real device negotiation and bounded audio callback | V-CAPTURE | [RELIABILITY_AND_ENGINE.md](RELIABILITY_AND_ENGINE.md) |
| R03 | 1 | Clock, sample placement, physical latency and honest Pro Tools/Reason comparison | V-CLOCK | [RELIABILITY_AND_ENGINE.md](RELIABILITY_AND_ENGINE.md) |
| R04 | 1 | Atomic autosave, rolling snapshots and separate recoverable recording media | V-RECOVERY | [RELIABILITY_AND_ENGINE.md](RELIABILITY_AND_ENGINE.md) |
| R05 | 1 | Deep reliability audit, Safe Open, logs and fault containment | V-RELIABILITY | [RELIABILITY_AND_ENGINE.md](RELIABILITY_AND_ENGINE.md) |
| R06 | 1 | Personal VST3 scans, native editors/state/PDC; retain working CLAP | V-HOST | [RELIABILITY_AND_ENGINE.md](RELIABILITY_AND_ENGINE.md) |
| R07 | 1 | Isolated plugin scanning, quarantine and bounded runtime-sandbox experiment | V-ISOLATION | [RELIABILITY_AND_ENGINE.md](RELIABILITY_AND_ENGINE.md) |
| R08 | 1 | Remove old bundled processors; preserve inactive legacy state | V-LEGACY | [RELIABILITY_AND_ENGINE.md](RELIABILITY_AND_ENGINE.md) |
| R09 | 1 | Shared command, parameter and knowledge contracts | V-COMMANDS | [RELIABILITY_AND_ENGINE.md](RELIABILITY_AND_ENGINE.md) |
| R10 | 2 | Takes, comping, punch/pre-roll, fades, gain, strip silence and vocal edits | V-VOCAL | [WORKFLOW_UI_REMOTE.md](WORKFLOW_UI_REMOTE.md) |
| R11 | 2 | All eight Studio One audio mouse tools and eight Paint variants | V-TOOLBAR | [WORKFLOW_UI_REMOTE.md](WORKFLOW_UI_REMOTE.md) |
| R12 | 2 | Separate Pro Tools-style Nudge section with exact sample arithmetic | V-NUDGE | [WORKFLOW_UI_REMOTE.md](WORKFLOW_UI_REMOTE.md) |
| R13 | 2 | Complete versioned Pro Tools and Studio One shortcut inventories and separate profiles | V-SHORTCUTS | [WORKFLOW_UI_REMOTE.md](WORKFLOW_UI_REMOTE.md) |
| R14 | 2 | Simple buses, sends, independent send pan and spill | V-ROUTING | [WORKFLOW_UI_REMOTE.md](WORKFLOW_UI_REMOTE.md) |
| R15 | 2 | Professional Console mixer and Visual mixer on one shared engine | V-MIXERS | [WORKFLOW_UI_REMOTE.md](WORKFLOW_UI_REMOTE.md) |
| R16 | 2 | Defined Logic-style clean profile, clean f64 summing and legacy compatibility | V-PROFILES | [WORKFLOW_UI_REMOTE.md](WORKFLOW_UI_REMOTE.md) |
| R17 | 2 | Dedicated LUNA-style Summing/Tape slots, order, PDC and personal plugin providers | V-SLOTS | [WORKFLOW_UI_REMOTE.md](WORKFLOW_UI_REMOTE.md) |
| R18 | 2 | Record/Edit/Mix/Master UI layouts and compatible SML Studio branding | V-LAYOUTS | [WORKFLOW_UI_REMOTE.md](WORKFLOW_UI_REMOTE.md) |
| R19 | 2 | FL Studio-inspired menu responsiveness measured under load | V-MENUS | [WORKFLOW_UI_REMOTE.md](WORKFLOW_UI_REMOTE.md) |
| R20 | 2 | Custom data-only skins, scaling and accessible fallback themes | V-SKINS | [WORKFLOW_UI_REMOTE.md](WORKFLOW_UI_REMOTE.md) |
| R21 | 2 | Authenticated phone browser remote with native transport ownership | V-REMOTE | [WORKFLOW_UI_REMOTE.md](WORKFLOW_UI_REMOTE.md) |
| R22 | 2 | Versioned searchable knowledge base covering real tools and limitations | V-KNOWLEDGE | [WORKFLOW_UI_REMOTE.md](WORKFLOW_UI_REMOTE.md) |
| R23 | 3 | Channel EQ: host integration target; actual binary compatibility conditional on supplied artifact | V-EXTERNAL_PLUGIN | [PROCESSOR_SPEC.md](PROCESSOR_SPEC.md) |
| R24 | 3 | Gate/Expander: host integration target; actual binary compatibility conditional on supplied artifact | V-EXTERNAL_PLUGIN | [PROCESSOR_SPEC.md](PROCESSOR_SPEC.md) |
| R25 | 3 | Flexible Compressor: host integration target; actual binary compatibility conditional on supplied artifact | V-EXTERNAL_PLUGIN | [PROCESSOR_SPEC.md](PROCESSOR_SPEC.md) |
| R26 | 3 | Optical Leveler: host integration target; actual binary compatibility conditional on supplied artifact | V-EXTERNAL_PLUGIN | [PROCESSOR_SPEC.md](PROCESSOR_SPEC.md) |
| R27 | 3 | Tune: host integration target; actual binary compatibility conditional on supplied artifact | V-EXTERNAL_PLUGIN | [PROCESSOR_SPEC.md](PROCESSOR_SPEC.md) |
| R28 | 3 | Vocal Editor: host integration target; actual binary compatibility conditional on supplied artifact | V-EXTERNAL_PLUGIN | [PROCESSOR_SPEC.md](PROCESSOR_SPEC.md) |
| R29 | 3 | Align: host integration target; actual binary compatibility conditional on supplied artifact | V-EXTERNAL_PLUGIN | [PROCESSOR_SPEC.md](PROCESSOR_SPEC.md) |
| R30 | 3 | Non-destructive native timing edits and bounded plugin/render integration | V-EDIT_RENDER | [PROCESSOR_SPEC.md](PROCESSOR_SPEC.md) |
| R31 | 3 | Host compatibility, automation/state, latency and offline evidence | V-HOST_EVIDENCE | [PROCESSOR_SPEC.md](PROCESSOR_SPEC.md) |
| R32 | 4 | Expert assistant Explain/Operate/Engineer modes with grounded knowledge | V-ASSISTANT | [ASSISTANT_MASTERING.md](ASSISTANT_MASTERING.md) |
| R33 | 4 | Push-to-talk voice input/output, audio analysis and phone assistant | V-VOICE | [ASSISTANT_MASTERING.md](ASSISTANT_MASTERING.md) |
| R34 | 4 | Dedicated mastering workspace, references, loudness/true-peak and export | V-MASTERING | [ASSISTANT_MASTERING.md](ASSISTANT_MASTERING.md) |
| R35 | 4 | Cross-system soak, failure recovery and measured reliability release gates | V-RELEASE | [ASSISTANT_MASTERING.md](ASSISTANT_MASTERING.md) |
| R36 | 4 | Practical manuals, migration, installation and transparent feature coverage | V-HANDOFF | [ASSISTANT_MASTERING.md](ASSISTANT_MASTERING.md) |
| R37 | 2 | Monitor-only volume/dim/mono and real cue-route control where supported | V-MONITOR | [WORKFLOW_UI_REMOTE.md](WORKFLOW_UI_REMOTE.md) |
| R38 | 4 | Exactly four resumable autonomous prompts with evidence and no routine approvals | V-AUTONOMY | [ASSISTANT_MASTERING.md](ASSISTANT_MASTERING.md) |

Maintain a repository ledger with: ID, actual source paths, command/UI entry, implementation status, evidence path, commit, environment, native/reference gaps, and next action. Statuses: UNVERIFIED, MISSING, IMPLEMENTED_UNTESTED, PASS_PORTABLE, PASS_NATIVE, PARTIAL, BLOCKED, DEFERRED_BY_OWNER. Deferred custom summing/tape models are tracked separately; no mandatory row can be labeled deferred without an explicit scope decision.

A requirement may have portable PASS and pending native evidence simultaneously. Do not collapse those into unconditional PASS. The seven future plugins have separate conditional integration records. AWAITING_EXTERNAL_PLUGIN does not block DAW completion; a compatibility PASS requires the actual tested binary. The DAW has no obligation to create these plugins.
