# Sprint 04 — Target performance, owner workflow and delivery
Revision 1.6 • PLANNED / NOT EXECUTED • gate A4.

## Objective and prerequisites
Validate real target behavior and deliver a truthful reviewable artifact/checkpoint. A1–A3 portable PASS is required for a complete product candidate. Report/checkpoint work is always allowed at an earlier stop. Actual Windows runtime/interface/driver and owner plugins are required for H claims; their absence does not authorize invented results.

## Read set and allowed scope
Read all current specifications/reports/evidence and VALIDATION_PLAN. Inspect actual apps/soundcraft main/graphics, recording/mix/UI snapshots/host lifecycle, packaging/windows/package.ps1 and release-playbook instructions before commands. Allowed: measured optimization/bug fixes, regression, local unsigned portable packaging, manual and reports. No new feature expansion, unrelated port, owner installation/signing/publishing.

## Concrete work
1. Profile release builds with declared workload. Preserve correct audio while fixing measured host bottlenecks; no rescans/waveform generation/full plugin rebuild per fader update. Separate UI/disk/DSP load and callback deadlines. Run A4 full regression after changes.
2. Execute available H1–H3 and CL-T6/7. Two separate 30-minute uninterrupted capture configurations plus calibration/reopen and separate short transport stress are required; these do not fit a 50-minute final sprint. If target/time is missing, produce exact pending gauntlet and blocked verdict. Record raw latency/offset/uncertainty, real driver settings, xruns and source/target details.
3. Complete native vocal workflow: three takes -> comp -> breath gain -> crossfade -> Nudge -> easy Vocal Bus -> external FX return -> spatial balance/width -> Console update reflected in Visual -> snapshots -> save/reopen -> named dry/processed/full export. Test actual focus/keys, N09/VM-T7/SL-T8, and stated screen/scale matrix. Owner usability/listening is recorded only if actually supplied. Reference comparisons are optional claim gates.
4. Build/package using actual inspected supported commands, record artifact SHA256/size/architecture/source. Native smoke requires clean extraction/start/setup/record/reopen/plugin/export. Cross-compiling an exe is not a Windows runtime pass. If no target artifact can be built, preserve source/patch/evidence/manual and identify the missing binary.
5. Finish docs/USER_GUIDE.md and docs/TARGET_MACHINE_GAUNTLET.md with dry recording/direct vs software monitoring, Nudge, slots/providers/sends/spill, Visual, state/automation, sample-rate/buffer/calibration, recovery and unsupported features. Describe in-process plugin risks factually and native model deferral. No owner media/credentials or third-party binaries in the repo.

## Validation, outputs and verdict
A4 passes only with applicable full regression; H1–H4/owner gates determine release readiness. Artifact checksums establish identity, not audio quality. Emit real regression.log, native CSV/log/image evidence, package-smoke.md and artifact manifest under docs/evidence/sprint_04/ only for executed actions. Write docs/sprint_reports/SPRINT_04_REPORT.md only if this sprint was actually entered, and always docs/AUTONOMOUS_BUILD_FINAL_REPORT.md with all statuses.

PORTABLE PASS plus missing H evidence is a reviewable candidate with RELEASE READY NO. Current slot infrastructure readiness is separate from deferred native sound-model readiness. Budget cap yields a resumable checkpoint; no fifth sprint, presumed owner approval or “all done” on missing rows.

## Execution, report and continuation rules
Follow docs/AUTONOMOUS_BUILD_PROMPT.md for authorization, worktree, six-hour cap and stop/checkpoint policy. Applicable repository instructions remain mandatory. Local ordinary edits/test fixes are authorized by launching the master; no push/merge/install/signing/purchase or delegated agents. No test weakening or fabricated results. Obsolete catalogue-only tests may be retired with explicit rationale and preserved shared coverage.

Run the full inspected `cargo xtask ci` and relevant real feature commands before an accepted milestone commit. Record unavailable native checks as H pending and portable failures as blocking. Write the sprint report only after actual work, update living handoff/command index, inspect diff and print commit SHA after commit. A partial/WIP checkpoint never claims acceptance. At any stop write the final report/next action even if no later sprint ran.
