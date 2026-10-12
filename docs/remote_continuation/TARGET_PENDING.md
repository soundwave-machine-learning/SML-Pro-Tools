# Target-pending checks (Remote Continuation 3.0)

Checks that cannot run in environment P (Linux container, no device/display/Windows/keyboard).
Each names what to do, on which environment, and what passes. Nothing here has been observed.

| # | Env | Check | Pass criterion | From |
|---|---|---|---|---|
| T01 | H (owner machine) | 64-frame playback + 2-track record with the realtime audio thread, 10 min | no xrun counted; meter/latency alarm quiet | RC02 / B02 |
| T02 | W/H | Physical keypad: Num+/Num− vs top-row +/− in the Edit window, text field focused and not | keypad nudges; top-row with text focus types; behaviour matches nudge_keys docs | RC03 / B03 |
| T03 | H | Real third-party VST3 with a sidechain input (e.g. a gate/compressor), keyed from a bus | gain reduction follows the key; capability record says "routed into the first aux input bus" | C03 / R24 |
| T04 | L | Listen to Bend renders with anchors inside sustained notes | no clicks at anchors; any 10 ms level dip judged acceptable or reported | C04 / R30 |
| T05 | W | Log file on Windows contains `~\` instead of `C:\Users\<name>\` and no tokens | open `%APPDATA%\…\logs\soundcraft.log` after opening a session | R05 |
