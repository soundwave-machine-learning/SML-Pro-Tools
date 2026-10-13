# Hotfix Pass 03: checks pending on the target machine

Everything in Hotfix Pass 03 was built and tested in a Linux container. That container had no
Windows host, audio interface, display, plugins, phone or listener. The Windows code was
cross-compiled and passes clippy for `x86_64-pc-windows-gnu`, but none of it has been **run on
Windows**.

The checks below need the owner's computer. Each one says what to do and what counts as a pass.
Use a copied or disposable project for every check that records, exports or uses the control
channel, never real unsaved work. Close SML Studio before updating it.

## Audio (HF07)

| # | Check | Pass when |
|---|---|---|
| A1 | Audio Setup › Driver type lists WASAPI, WASAPI Low Latency, WASAPI Exclusive and ASIO (greyed out). | ASIO's hover text gives the reason it is unavailable. |
| A2 | **WASAPI Low Latency** on the SSL 2 at 48 kHz: Apply, play for 30 minutes. | "Buffer (delivered)" shows the driver's small period, the ledger shows "Driver output latency", and there are no dropouts. Record the period. If the driver offers no smaller period, Audio Setup says so. |
| A3 | **WASAPI Exclusive** on the SSL 2 at 128 frames (then 64), 30 minutes of playback. Another program tries to play at the same time. | The delivered size is the aligned request. Playback has no dropouts. The other program cannot use the device, which is expected. |
| A4 | Unplug the SSL 2 during Exclusive playback. | "Playback stopped…" appears, the app does not crash, and Apply reopens the device. |
| A5 | Record a take while the output is in Exclusive or Low Latency mode. | The input still opens in shared mode, and the take is aligned. Use loopback or a clap to check. |
| A6 | ASIO | **BLOCKED.** It needs Steinberg's ASIO SDK under a licence the owner accepts, plus an MSVC/LLVM build. Not attempted. |

## Plugins (HF13, carried forward)

| # | Check | Pass when |
|---|---|---|
| P1 | Retry the 13 quarantined FabFilter VST3s (Quarantined (n) › Retry, or `engine.plugin_scan_retry {all: true}`). | Each one either loads, or is quarantined again with the stage it reached and the exit code. **The cause is unknown until this runs.** The module-exit change from Hotfix 02 is a hypothesis, not a fix. |
| P2 | Full rescan of the 384-VST3 library over the control channel, now with a token. | The request returns a job at once, the app survives, and the job completes or is cancelled. |

## Recording safety (HF17, HF18)

| # | Check | Pass when |
|---|---|---|
| R1 | Use a copy of a project with a clip on each side of a gap. Pre/post-roll on; punch-record a selection that overlaps both clips. | Material outside the selection is unchanged (play it back). A "<track>.NN before take NN" playlist holds the old material. One undo restores everything. |
| R2 | While recording, try File › New, Open, Close, Revert, Recover Autosave… › Open / Recover, and the same from the control channel. | Each one is refused with "a take is being recorded…". Recording continues, and the take is placed when you stop. |

## Monitoring (HF15, HF16)

| # | Check | Pass when |
|---|---|---|
| M1 | Master layout: set a reference, then Level match, then A/B on while playing. | You hear the reference at matched loudness. Switching is a smooth 10 ms crossfade. An export made with A/B on does not contain the reference. |
| M2 | Click M on the Master strip while playing. | Main fades out with no click. The Master meter drops. The metronome click is still heard if it is on. Exporting warns that the Master is muted. |

## Export (HF20)

| # | Check | Pass when |
|---|---|---|
| E1 | Bounce Mix to MP3 at 320 kbps, 48 kHz stereo. | The file plays in Windows Media Player or another player. The result's `verified` block is present. The length is within one codec frame. Title and artist tags are visible. |
| E2 | Bounce Mix to M4A at 192 kbps. | Same as E1, and the file opens in another player. |
| E3 | Cancel a long MP3 export. | No file is left under the target name, and no `.sml-partial` file remains. |

## Control and menus (HF19, HF21, HF22)

| # | Check | Pass when |
|---|---|---|
| C1 | Start SML Studio with `--control 7979`, then open Setup › External Control…. | It shows the port and a redacted token. Copy, Rotate and Revoke behave as the manual describes (§25.2). |
| C2 | Update the Grok connector with the token, without pasting the token into the chat. | The connector authenticates. A connector without the token gets `auth_required`. |
| C3 | Menus at the owner's display scale. | Window › Recording, SML Remote, Detach/Dock Mixer; File › Recover Autosave…; Help › Assistant and Command Help all open their windows. Long menus scroll. |
| C4 | `python examples\example.py 7979` (read-only), run from `sdk\`. | It prints the protocol, the app version and the session. |

## Update and rollback

| # | Check | Pass when |
|---|---|---|
| U1 | Close SML Studio, then run `Update-SMLStudioCandidate.ps1 -WhatIf` and then the real update (DELIVERY steps). | The settings backup includes `control-token`, and the token is not printed. |
| U2 | Return to Hotfix 02 with the script's rollback. | The old `soundcraft.exe` hash matches again. Sessions saved by Hotfix 03 open there (see the downgrade notes in the report). |
