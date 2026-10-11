# Requirement and evidence matrix — revision 1.6

All current rows mandatory within the stated scope; native/claim/deferred qualifiers apply exactly as shown. Status of every software/target test: NOT EXECUTED. Source inspection is not a test.

| ID | Requirement | Sprint owner | Evidence/gate |
|---|---|---|---|
| R01 | Device/input selection and actual rate/buffer | 1 | CL01–02, CL-T1/2, A1 capture; H1 |
| R02 | Independent arm/monitor/transport and dry recording | 1 | A1 capture/workflow, CL-T3/5; H1 |
| R03 | Bounded disk capture, finalization and crash recovery | 1 | A1 failure/recovery and CL-T5; H1 |
| R04 | Remove upstream bundled effects/instruments, retain internal services | 1 | A1 catalogue and callsite inventory |
| R05 | Preserve removed/missing processor state and source media | 1 | A1 migration; SL-T4 |
| R06 | Own VST3 plugins, retained CLAP, native editors/state/export | 3 | A3 host fixture/lifecycle, SL-T6; H2 |
| R07 | Professional stereo mixer and correct routing/compensation | 3 | B-T1..6, M-T1/2/5, CL-T4 |
| R08 | Simple recording view and usable mixer controls | 1,3 | UI checks; H4/UX |
| R09 | Measured audio/UI responsiveness | 1,3,4 | CL-T5/6/7, VM-T7, SL-T8; H3 |
| R10 | Regression, truthful portable candidate, manual and delivery | 4 | A4; H4 |
| R11 | Vocal editing tool families V01–V14 | 2,3 | V-E2E staged; H2/UX |
| R12 | Dedicated sample-precise Nudge | 2 | N01..06/08 software, N07/09 native; N10 claim-only |
| R13 | Easy subgroup and FX-return bus creation | 3 | B01–06, B-T1..6; B-T7 UX |
| R14 | Complete vocal record/edit/mix walkthrough | 4 | V-E2E full, VM-T8; H4/UX |
| R15 | Defined Logic-style clean mixer profile | 3 | M01–02, M-T1/2; Logic comparison claim-only |
| R16 | Dedicated selectable Summing slot on bus/Main | 3 | M03–05, SL01–03, SL-T1..6 |
| R17 | Slot signal integrity and evidence; native sound models deferred | 3,4 | M-T1/2/5/6 software; M-T7 native; M-T3/4/8 deferred |
| R18 | Console/Visual/Split views over one mixer | 3 | VM01/04, VM-T1/2/6 |
| R19 | Spatial gain/pan/width/meters/list/A-B-C snapshots | 3 | VM02/03/05, VM-T3..5 |
| R20 | Bidirectional automation/undo/persistence and shared routing | 3,4 | VM-T1..6 software; VM-T7/8 native |
| R21 | Track Tape and bus/Main Master Tape with position control | 3 | SL01–02/04, SL-T1..6 |
| R22 | Provider lifecycle/PDC/state/category bypass/group-ready schema | 3 | SL03–05, SL-T3..6, CL-T4 |
| R23 | Spill, independent send pan and visible gain staging | 3 | SL06, SL-T7; B-T7 UX |
| R24 | Audio-owned clock, device negotiation and timestamp provenance | 1 | CL01–02, CL-T1/2/3 |
| R25 | Monitoring modes, graph compensation and calibrated placement | 1,3,4 | CL03–04, CL-T3/4 software; CL-T6/7 native |
| R26 | Bounded realtime work and separate response metrics | 1,3,4 | CL05–06, CL-T5 software; CL-T6/7 native |

Track V01–V14, N01–N10, B01–B06/B-T1–B-T7, M01–M05/M-T1–M-T8, VM01–VM05/VM-T1–VM-T8, SL01–SL06/SL-T1–SL-T8 and CL01–CL06/CL-T1–CL-T8 within reports. Do not replace feature coverage with a passing test count. Current M-T3/4/8 are DEFERRED BY OWNER. Commercial-product native comparisons are claim-only, not baseline delivery prerequisites. H1–H4/owner UX remain target-readiness gates.
