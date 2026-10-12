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
| T06 | H | Real local model: run a local OpenAI-compatible server with a model you choose; ask 10 varied questions and 5 actions in Window › Assistant | answers arrive with elapsed time; every action shows as a plan and runs only on *Run plan*; note model, answer time, memory | RC04 |
| T07 | H | Push-to-talk with the real microphone: enroll 8 phrases, then say each 5 times; say 5 other phrases | ≥ 90 % recognised in a quiet room; others rejected; nothing runs without *Run plan* | RC05 |
| T08 | L | Listen to *Speak replies* for 10 answers | intelligible enough to confirm the action; note words that are not | RC05 |
| T09 | H | Speak replies while recording a take | the take contains no assistant speech; nothing heard while recording | RC05 |
| T10 | W | Unzip the Windows portable build on a clean Windows 10/11 x64 machine; start soundcraft.exe; open Setup › Playback Engine; play the demo session through WASAPI; record 30 s | starts without missing-DLL errors; WASAPI devices listed; playback and a take work; note the granted buffer size | RC07 |
| T11 | W | ASIO (optional): with the Steinberg SDK accepted and installed, `cargo build --release -p soundcraft --features asio` on Windows MSVC; select ASIO in Playback Engine | ASIO host and the interface's driver appear; playback/record at the driver's buffer size | RC07 |
| T12 | W | VST3 editor on Windows: insert an installed VST3 effect, open its editor, move a control, close the window, reopen | window floats over SoundCraft at the plugin's size; edits reach the session; closing tears the view down; no crash on reopen. Or run `.github/workflows/windows-native-tests.yml` (manual) for the fixture editor | RC07 / C02 |
| T13 | W | 1366×768 laptop at 150 % scaling: the Edit window toolbar | Play/Stop/Record and the Nudge panel visible without scrolling; other groups scroll; keypad and −/+ fallback behave as the hover text says | RC09 |
| T14 | M | Pair a real phone, start recording on the desktop, lock the phone / close the browser / walk out of Wi-Fi range, come back | recording never stops; the reopened page shows "recording" and the current state; the page says the microphone is unavailable (plain HTTP) | RC08 |
