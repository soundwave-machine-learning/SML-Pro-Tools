# Recording, hosting and recovery
Requirements R01–R09. Core behavior also uses the preserved clock and mixer specifications.

## Establish the actual floor
Record HEAD, branch, dirty/untracked user files, upstream relation, build target and local instructions. Read REPORTED_BASELINE.md and actual prior sprint reports and map each original requirement to real code plus evidence. Identify UI-only controls, disabled implementations, unsound threads, panic/exception paths, unbounded buffers, file lifecycle races and missing migrations. Retain working implementations. Do not infer tests passed from a commit message or generated report.

Discover exact build/test/lint/package commands from checked-in scripts and CI; execute only applicable commands. Store exit status, toolchain versions and logs. Separate pre-existing failures from regressions. Verify license and dependency compatibility before incorporating new DSP/model code. Public documentation is not a license to redistribute proprietary implementation.

## Capture and clock
Re-run foundation clock/device/callback/capture tests on the actual branch before extending it. Support negotiated rather than requested-only device values. Preserve dry input data; one file per take/channel policy must be explicit. Check frame continuity using encoded test signals/counters, not silence. Never substitute a silent clock for successful device capture. Record driver-reported latency and measured residual calibration separately. Low-latency mode changes the actual graph through stopped, explicit deactivation/rebuild; ordinary bypass preserves PDC. Do not silently bypass the user's sound or alter exported signal.

## Project autosave
Proposed defaults: autosave dirty sessions every 60 seconds, retain 20 rolling recovery snapshots, and enforce a configurable disk budget while preserving the last known-good snapshot. These are original SML settings, not claims about any vendor. Autosave is independent from the explicit project Save operation; show last successful recovery time and any lag/failure.

Take an immutable model snapshot or safely versioned copy. Serialize on a worker to a unique temporary sibling, flush according to the platform's supported durability contract, validate schema/checksums, then atomically replace the snapshot index/commit marker on the same filesystem. Keep the previous valid generation until the new one is committed. Document platform/filesystem guarantees and power-loss limitations. One save job plus one replaceable pending revision; do not run concurrent project writers. Disk-full, permission and rename failure leave the current project and prior snapshot intact. Never block recording to finish autosave.

Third-party plugin state may be unsafe to fetch during processing. Maintain latest safe per-plugin checkpoints using supported host behavior. Recovery records checkpoint age and identifies stale state; defer unsafe capture until stopped. Autosave can preserve edits with older plugin state, but must state that limitation. No mandatory audio-thread serialization.

## Recoverable recording media
Project autosave alone does not protect a take. Write media through a bounded disk queue with reserved capacity and clear overrun errors. Use recoverable container/chunk metadata and a separately committed media journal. A journal commit may refer only to audio frames whose write/flush requirements have completed; store generation, file identity, channel/rate, frame count and integrity metadata. Flush batching and durable interval are measured in E02; proposed target <=1 second under supported local storage. Do not claim zero power-loss exposure between durable commits.

After forced termination, recover all verifiably committed complete frames into a new recovery session; surface partial tail/uncertain frames separately. Fix container headers from validated data without overwriting originals. Keep non-empty partial takes discoverable even when the project snapshot predates their creation. Test abrupt process termination separately from OS/power failure. Report the actual tested failure model and maximum observed lost tail. Graceful-stop recovery must preserve the full accepted take.

## Plugin lifecycle
VST3 scan in an external worker: user-selected paths, incremental cache keyed by identity/fingerprint, cancel, timeout, quarantine and explicit rescan/re-enable. Never scan DLLs on menu open or the audio thread. Retain working CLAP with equivalent safety coverage. No arbitrary plugin downloads or format expansion. Native UI open/close/focus, state chunks, presets, latency changes, sidechains, automation and missing-plugin reopen need real-host tests.

Runtime worker prototype E03 must measure process grouping, shared-buffer ownership, IPC deadline, added samples, editor attachment, state restore, worker crash/hang and graph continuation. No busy wait for a dead worker in the callback. Define a visible fault state with bounded ramp to a documented output policy; preserve dry recording and plugin state for recovery. Do not silently resume a crashed processor during recording or silently export a substituted sound. A faulted required processor makes offline export fail until the user chooses an explicit resolution. If reliable runtime isolation is infeasible on the current backend, ship no isolation claim; mark that capability BLOCKED and expose an honest in-process compatibility mode if already supported. Scan isolation can still pass separately.

Safe Open offers external-plugin-disabled loading and optional reset of UI/theme preferences, with original state retained and a reversible restoration path. Built-in DSP is still tested for finite/bounded output; it is not assumed infallible. Crash reports redact private paths/audio by default and are local unless the owner elects to share.

## Failure policy
Device removal stops or marks affected capture with explicit reason, preserves media and never switches input silently. Exhausted disk/queue errors are visible and timestamped. Invalid media/state/theme/remote input has size/range/depth limits. A GUI stall cannot own the recording writer or transport clock. Graph edits cannot destroy live plugin objects on the callback. Callback headroom, memory high-water, queue depth, xruns and failed recovery attempts are measured together.

Reference lessons are mechanisms, not borrowed reliability rankings: REAPER advertises plugin bridging/firewalling; Cubase documents Safe Mode and project autosaves; Ableton documents plugin-origin crashes and recovery. None proves SML reliable until its own fault tests pass. See SOURCES.md.
