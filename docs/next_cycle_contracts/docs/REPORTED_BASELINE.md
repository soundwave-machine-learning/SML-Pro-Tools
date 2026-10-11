# Baseline from the owner's supplied Claude report
Revision 2.2 • 2026-10-11. Evidence class: REPORTED, not INSPECTED or RERUN. The owner pasted the prior-cycle final report. No source checkout, raw log, artifact or manifest was inspected while authoring this update.

## Reported identity and artifacts
- Repository: soundwave-machine-learning/SML-Pro-Tools.
- Branch: fable/recording-core.
- Final HEAD: 396fd0a7a7aa5b62d8dae0e49b3413c7e2e3432b.
- Report/handoff commit: c74ac7a; final follow-up reportedly adds the checkpoint line.
- Artifact source commit: 7838032. Verify the full commit identity and source diff to final HEAD; do not assume later changes are documentation-only.
- main reportedly remains at 7414719, with a clean working tree; nothing pushed or merged.
- cargo xtask ci reportedly passes 616 tests, formatting, lint, asset, layer and wasm checks. Confirm exact command against current xtask and inspect docs/evidence/sprint_04/regression.log before treating it as inspected evidence. A current rerun is separate evidence.
- Windows artifact: sml-recording-0.5.0-windows-x64-gnu-unsigned.zip, WASAPI only, no Windows execution, no ASIO.
- Linux artifact: soundcraft-0.5.0-linux-x86_64.tar.gz; extracted CLI smoke reportedly passed; GUI unavailable without display.
- Checksum manifest reportedly at docs/evidence/sprint_04/artifact-manifest.json. Actual checksum values were not supplied here. Dist artifacts reportedly not tracked in git and cloud workspace is temporary; do not claim their attachments are available in a later session.

## Reported fixes to verify and preserve
Worker scheduling removed from light tracks; mixer structural hashing removed from steady playback; steady-state allocation test; replaced-session destruction off audio thread; device input picker; slot input/output meters; Visual list route badges/clip warning/rename; loop takes placed using audio timestamps; snapshot/automation test; invalid input refuses recording and remains silent while monitoring; many-channel takes load only armed channels; input/output device-loss handling stops capture visibly and retains captured media; real musical Count Off behind existing toggle/bar count; Count Off cleared on stop/consumed by next play and excluded from offline render; small-screen mixer/toolbar scrolling; Count Off accessible in Recording window.

Reported relevant commits: 17ecf26 device loss; b183c02 tempo-map click test; 81c5f54 count-in documentation; 1e3f16d working Count Off; 03e1911 scale/render/scroll fixes; 7838032 Recording-window count-off access. Read the real diffs, test changes and final docs before accepting these claims. Corrected test defects are acceptable only with an independent expected-result rationale; no acceptance weakening.

## Open carry-forward ledger
| ID | Reported gap | Priority / requirement | Required closure evidence |
|---|---|---|---|
| B01 | Structural track/plugin/route edits allocate on the audio thread during playback | Critical CL05 software gap | Off-thread preparation and safe lifetime/publication; targeted mutation stress with allocation/deallocation guards, no unsafe locks or teardown. If full live mutation is not yet safe, explicitly require stopped transport and enforce in all clients; document that limitation, never claim live-edit support |
| B02 | 11–18 overruns per 270,000 callbacks at 64 frames in the VM | Open performance result; not proven to be VM-only | Retain measurements and analyze callback compute versus scheduling jitter/deadline misses; real native buffer matrix later. No zero-xrun PASS or silent target relaxation |
| B03 | Toolkit cannot distinguish numpad/main-row; Win+Num± unavailable | Partial Nudge/profile compatibility | Inspect actual native events/backend version; preserve key-location semantics where supported; accessible remappable fallback; real keyboard/focus tests pending until available |
| B04 | QuickPunch, TrackPunch, Destructive Record/Punch toggles have no behavior | Misleading UI, not authorization to implement destructive modes | Disable/label unavailable; no tooltips/assistant/remote claims that they work; retain legacy data. Add actual behavior only under a later approved recording-mode contract |
| B05 | H1–H4, CL-T6/7, V14/listening and personal-plugin/native editor checks not executed | Native release gates | Target-machine gauntlet with real device/driver/plugins and recording continuity/latency results; cannot be substituted by cloud CI |
| B06 | ASIO absent; GNU Windows cross-build only | Backend/package gap | Inspect current CPAL/build setup and SDK availability; build/test supported ASIO when legitimate dependencies/host exist; retain WASAPI and honest backend labels. Signing remains outside current authorization |
| B07 | Faders and toolbar reachability restored via scrolling | Portable fix reported; usability still pending | Native screen/scale/focus/scroll review; keep Stop/Play/Record and active Nudge discoverable on small windows, not only hidden at far end of a long row |
| B08 | V12 requires an external pitch plugin | External-product dependency | Personal supplied-plugin integration only. The seven SML plugin developments are separate owner projects; their absence must not block generic DAW completion |
| B09 | Autosave/media durability and runtime plugin containment not established by this report | Unverified next-cycle requirements | Inspect actual code first; complete revision 2.2 recovery and plugin-host contracts with fault evidence rather than infer these from 616 tests |

## Working while the owner is away
Do not ask for hardware, native UI or listening approval simply to continue independent portable work. Maintain a TARGET_PENDING ledger and a repeatable target-machine gauntlet. Missing native access blocks native readiness claims, not all later implementation. Never install on the owner's machine remotely without separate authorization. Do not fabricate measurements or promote a cross-build into an installed product.

## Baseline access rule
Locate the actual branch/checkpoint in the existing Claude workspace or a non-destructively retrievable owner-supplied checkout/bundle. Verify HEAD and preserve later user changes. A public clone may lack this unpushed branch. If the reported source is unavailable, record BASELINE_UNAVAILABLE and prepare reviewable contract/evidence work; do not silently restart from main or invent the missing changes. No push is authorized by this planning package.
