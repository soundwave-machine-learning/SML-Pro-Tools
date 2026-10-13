# Hotfix Pass 03 report

**Branch:** `fable/recording-core`, local commits only. Nothing was pushed, merged, published or
installed.

**Start:** `d75158f`, the Hotfix 02 docs checkpoint. It was clean, 8 commits ahead of
`origin` and 0 behind. Toolchain: rustc 1.97.0, Linux x86_64 container.

**Candidate:** built from `cb08e7e` (clean tree). The update script comes from `147cb25`.

**Platform:** all execution was in the Linux container. The Windows code is cross-compiled and
clippy-clean for `x86_64-pc-windows-gnu`, but **has not been run on Windows**. No audio
interface, display, plugin, phone or listener was available.

Evidence labels used below:
- **RERUN**: run during this pass, with a log.
- **INSPECTED**: source or diff read.
- **REPORTED**: taken from an earlier report without re-running.

## 1. Baseline audit (Hotfix 02 checkpoint)

| Check | Result | Label |
|---|---|---|
| Package | All 18 files match `PACKAGE_MANIFEST.json`: revision 1.0, tickets HF15–HF22, carry-forward HF07 and HF13. | RERUN |
| Checkout | HEAD `d75158f`, clean, 8 ahead / 0 behind `origin/fable/recording-core`. Not reset; every later commit builds on it. | RERUN |
| Docs-only after the artifact | `f6deab2..d75158f` changes only `DOCS_MANIFEST.json`, `HOTFIX02_REPORT.md` and the update-script log. | RERUN |
| Hotfix 02 artifact | `dist/hotfix02-windows/SHA256SUMS.txt` verifies (ZIP, MANIFEST, script). ZIP `026632fe…759d`, soundcraft.exe `21e4e463…f781`, as the package states. | RERUN |
| Hotfix 02 gate log | `docs/hotfix_pass_02/evidence/ci.log`: 766 passed, 0 failed. | RERUN (log read) |
| The rewritten toolbar test (`compact_record_toolbar_keeps_nudge_and_transport_reachable`) | Intent kept at normal widths: transport reachable, no overlap with Nudge, tools in the top row. **Weakness found:** at the minimum window it only checked that the "More controls"/"More tools" buttons exist. It now opens "More controls" and finds NUDGE there and not in the bar (`f98af69`). The clip-drag tests added in Hotfix 02 are new, not rewritten. | INSPECTED + fixed |
| Scanner job lifecycle (`crates/mix/src/scan_jobs.rs`) | One running job plus at most one waiting (coalesced). Cancel kills the running workers and keeps finished results. Retry touches only the chosen bundles. Job history is bounded (16). A recovery point is written before any scan (`hf12_a_scan_request_writes_a_recovery_point_first`). | INSPECTED; tests RERUN in the gate |
| Package correction: "480 frames is not an OS-wide law" | Accepted. The manual no longer states it as a law: plain WASAPI uses "the Windows audio engine's period (often 10 ms…)", and IAudioClient3 is implemented (HF07). | INSPECTED |
| HF11 (Hotfix 02) | **Defect found.** The report said Detach/Dock Mixer were in the Window menu, but they were registered commands with no menu-bar entry. Fixed in HF19. | INSPECTED + fixed |

## 2. Per-ticket status

| Ticket | Status | Commit | What changed | Evidence (all portable) | Not covered |
|---|---|---|---|---|---|
| **HF18** recovery guard | **Done (portable)** | `e26026d` | The engine refuses `session.new/open/close/revert` and `open_recovered`/`open_session` while recording, for every caller, before anything runs. The app also guards Recover Autosave's two buttons. | `hf17_hf18_recording_safety.rs::session_replacement_is_refused_while_recording`; `hotfix03.rs::hf18_recording_blocks_every_session_replacement_entry` (session, undo, dirty flag and recorder unchanged; recovery works after Stop) | A real device. Start-vs-recover race: both run on the UI thread (control requests are dispatched there too), so they are serialized. INSPECTED, no stress test. |
| **HF17** non-destructive punch | **Done (portable)** | `e26026d` | The take is trimmed to the punch range (or to the record point onwards) **before** it is placed. Pre-roll, count-off and post-roll are timing only. Replaced material is kept on "<track>.NN before take NN". One undo step. A capture that never reaches the range restores the document. | Punch matrix (start, middle and end of a clip; pre/post-roll on and off): outside the range the mix is bit-identical and sources are untouched. Also tested: overlaps, stereo track fed mono, early stop, empty capture, save and reopen. `k8_reproduction_with_the_old_placement_path` shows the old path losing material. | Writer/device failure injected *during a punch*, tempo/rate boundaries, loop+punch. The loop-record path is unchanged and was not re-tested here. Native recording review. |
| **HF16** Master mute | **Done (portable)** | `44c7ed9` | The Master's `mixer.mute` (and mute automation in Read) gates Main after the fader and Master Tape, with a 5 ms ramp. The meter is post-mute. The Master strip has an M button. Bounce results carry `master_muted` and a warning. | `master_mute_gates_the_program_with_a_ramp`; the cookbook re-run gives a silent export (−144 dB) with the warning | Device queue latency (the mute starts from the next block plus device buffering; not measured). By ear. |
| **HF15** reference A/B | **Done (portable)** | `44c7ed9` | The reference plays on the device output only: pre-decoded from the pool, level-match gain, frame 0 = session 0, rate conversion, 10 ms crossfade. Live states: Playing, Missing, Ended. Never in renders or capture. | `reference_replaces_the_program_on_the_output_only`, crossfade bound, end/missing/44.1 kHz, `reference_never_enters_a_render` (offline render bit-identical with A/B on or off), UI test on the Master card | Listening check. Device latency. |
| **HF07** low-latency backend | **Implemented, not run** | `65e4620` | New isolated FFI crate `soundcraft-wasapi-native`: WASAPI **Exclusive** (alignment retry) and **Low Latency** shared (IAudioClient3), event-driven with MMCSS "Pro Audio", output only. Offered in Audio Setup with honest notes. The ledger shows "Driver output latency". | `period_math`, `sample_conversion_is_exact_and_clamped`, the driver-type list test; Windows clippy | **Everything native:** SSL 2 open, delivered period, xruns, device loss (TARGET_PENDING A1–A5). **ASIO: BLOCKED** (Steinberg SDK licence acceptance + MSVC/LLVM build machine). |
| **HF13** FabFilter | **PENDING_NATIVE** | — | No change this pass. The Hotfix 02 diagnostics and Retry stand. The module-exit change is still a hypothesis. | — | Retry of the 13 plugins on the owner's computer (P1). |
| **HF21** authenticated control | **Done (portable)** | `fd87ab3` | Protocol 2: auth first (token `smlc_`+64 hex from OS randomness, file `control-token`, mode 0600 on Unix, constant-time compare, redaction). Limits: 1 MiB, 10 s to authenticate, 30 min idle, 8 connections. Typed codes. Rotate and revoke. Setup › External Control…. `soundcraft-cli token`. Migration message for protocol 1 clients. Clients never replay. | `admission_rotation_and_revocation` (none, wrong, protocol-1, malformed and HTTP requests all refused before dispatch; the token is never echoed); `bounds`; `tokens_are_random_well_formed_and_rotatable` | Browser-origin attempts: an HTTP request is refused as non-auth (tested); no CORS surface. The owner's Grok connector (C2). Windows ACL of the settings folder (the per-user `%APPDATA%` is relied on). |
| **HF19** menus | **Done (portable)** | `fd87ab3` | File › Recover Autosave…, Window › Recording / SML Remote / Detach Mixer / Dock Mixer, Setup › External Control…, Help › Command Help / Assistant. Disabled entries explain why. Menus taller than the window scroll. | `hf19_core_tools_are_in_the_menus`: real pointer clicks and wheel scrolling at the default size, 911×512 (1366×768 at 150 %) and 960×540 at 200 % | The owner's display (C3). |
| **HF20** MP3/AAC export | **Implemented, not run** | `9d0d761`, `9baf4de` | New isolated FFI crate `soundcraft-mf-encode`, using the Windows Media Foundation MP3 and AAC encoders through IMFSinkWriter. CBR only. TPDF dither to 16-bit. ID3v2 tag. Written to a hidden temporary file, decoded and verified (rate, channels, length within 4608 frames), then renamed. Refusals: VBR, loudness/limiter options, wrong extension, unsupported settings, non-Windows. **Media Foundation is loaded at run time** (see §6). | `pcm16_and_id3`, `verification_rules`, `real_lossy_files_decode_and_verify` (ffmpeg 6.1.1 fixtures, decode + truncation detection), `unavailable_encoder_leaves_nothing`, `hf20_compressed_export_requests` | **Every real encode:** the Windows encoders have not been run (E1–E3). |
| **HF22** SDKs | **Done (portable)** | `40d7494`, `f98af69` | `sdk/python/sml_studio.py` and `sdk/js/sml-studio.mjs` (stdlib only), typed errors, no replay, scan-job helpers, nothing runs on import. Examples are read-only unless `--temporary-session` (which refuses a dirty session). Schema `sdk/schema/sml-control-v2.schema.json`. `soundcraft-cli serve` provides a headless authenticated server. A protocol 1 (Hotfix 02) app is named as such. | `apps/soundcraft-cli/tests/sdk_e2e.rs`: both SDK suites and both examples against a real `serve` (6 Python, 5 JS cases: wrong/missing token, version mismatch, timeout without replay, protocol-1 app, typed command errors, temporary-session refusal) | Against the desktop app on Windows (C4). Scan jobs need the desktop app (`serve` answers with a typed error). |

**Also fixed during the gate** (`360acf3`):
- The browser (wasm) build broke because the token code needed `getrandom`. That dependency is now native-only.
- An old bounce test assumed MP3 was never writable.
- A pre-existing race in a capture test: a forgotten writer could checkpoint while `recover()` ran.

## 3. Audio backends and codecs

| Backend | Platform | Direction | Buffer | Status |
|---|---|---|---|---|
| WASAPI shared (cpal 0.16) | Windows | out + in | the engine period, set by Windows | built (as in Hotfix 02); not observed on Windows |
| **WASAPI Low Latency** (IAudioClient3, native) | Windows | out | a multiple of the driver's period step | built; **not run** |
| **WASAPI Exclusive** (native) | Windows | out | the request, aligned by the driver | built; **not run** |
| ASIO | Windows | — | — | **BLOCKED** (SDK licence, MSVC/LLVM) |
| ALSA (cpal) | Linux | out + in | settable | built; tests use no device |

| Format | Import | Export | Where |
|---|---|---|---|
| WAV / AIFF | yes | 16/24/32f, dither, rate conversion | all platforms |
| FLAC | yes | 16/24 | all platforms |
| MP3 | yes (symphonia) | **CBR 128–320 kbps; 32/44.1/48 kHz; mono/stereo**, ID3v2 | export on Windows with Media Foundation only |
| AAC-LC in M4A | yes (symphonia) | **96–192 kbps; 44.1/48 kHz; mono/stereo** | export on Windows with Media Foundation only |
| OGG Vorbis, ALAC, CAF | yes | no | — |

Codec licensing: no encoder ships in the package. The encoders that come with Windows are used,
and no LGPL code (LAME, shine) was taken. ffmpeg is used only by a test, to make fixture files,
and is not linked.

## 4. Control protocol 2 and SDKs

- Protocol 2: [../control-protocol.md](../control-protocol.md) and `sdk/schema/sml-control-v2.schema.json`.
- Migration: a protocol 1 client gets `auth_required` with the token file's location.
- Tokens: Setup › External Control… or `soundcraft-cli token path|show|rotate|revoke`.
- Grok: [../grok/GROK_OPERATOR_GUIDE.md](../grok/GROK_OPERATOR_GUIDE.md) §2 (never print the token, never replay).
- SDK: `sdk/README.md`. Version 1.0.0. Python 3.8+ and Node.js 18+, no dependencies. Local only, and also inside the candidate ZIP under `sdk/`.
- The Grok cookbook was re-run through the authenticated path, on a disposable session in a temporary settings folder: `evidence/cookbook-serve.log`, 53 calls OK, 0 failed. The MP3 export is refused off Windows, as documented.

## 5. Gate and platform

| What | Built | Executed | Log |
|---|---|---|---|
| `cargo xtask ci` at `cb08e7e`: fmt, clippy -D warnings, test, assets, layers, docs, wasm | yes | Linux: **791 tests passed, 0 failed** | `evidence/ci.log` |
| Windows clippy (`--workspace --exclude soundcraft-web --target x86_64-pc-windows-gnu -D warnings`) | yes | compile only | `evidence/windows-clippy.log` |
| Windows candidate (GNU cross-build) | yes | **not executed** | `evidence/cross-build.log` |
| SDK end to end (Python 3.13, Node 22) | yes | Linux | in `ci.log` (`sdk_e2e`) |
| Update script (PowerShell 7.4.6) | — | Linux, fake installs (**not Windows**) | `evidence/update-script-test.log` |
| Downgrade (Hotfix 02 CLI built from `d75158f`) | yes | Linux | `evidence/downgrade.log` |

## 6. Candidate

`dist/hotfix03-windows/`:

| File | SHA-256 |
|---|---|
| `soundcraft-0.5.0-windows-x64-gnu-portable.zip` | `7ce82669e694f41e7291bcd72a4e55bc6741caac9c13d8b9996a43a86e6cfc44` |
| `soundcraft.exe` (in the ZIP) | `34b385287fcf7ae21850a78d2d8ad2a50868a968c066d81a523a49d932b8191d` |
| `soundcraft-cli.exe` (in the ZIP) | `cae23a75bbe6d067a6b51e41e02ee3cda313dddec180a9d60dcc8102c50b6d6b` |

- **Build:** source `cb08e7e`, clean tree, unsigned, GNU toolchain, not an installer.
- **Manifest:** lists the backends, codecs, protocol, signing, each file's hash and the DLLs it
  imports.
- **Docs:** inside the ZIP, revision 2026-10-13.6, with the SDK under `sdk/`.

**A build regression was caught here.** The first Hotfix 03 build (`bb96abd`) made both
executables import `mfplat.dll` and `mfreadwrite.dll`. Windows N/KN editions without the Media
Feature Pack lack those DLLs, so the app would not have started there at all. Media Foundation is
now loaded at run time (`9baf4de`), and `cross-build.sh` refuses any executable that imports it.

New imports compared with Hotfix 02: `avrt.dll` (MMCSS, for the native WASAPI thread) in
`soundcraft.exe`, and `ole32`/`oleaut32`/`ws2_32` in the CLI. All are present on every Windows
edition.

## 7. Update, rollback and downgrade

**Update script, on fake installs** (old install = the Hotfix 02 ZIP plus a user session folder;
fake `ui.json` and a fake `control-token`):

| Check | Result |
|---|---|
| Wrong checksum | refused; the old exe is unchanged |
| App running from the folder | refused; the old exe is unchanged |
| `-WhatIf` | nothing created |
| Update | the session folder is carried over; settings are copied with the originals unchanged; `control-token` is copied **mode 600** (protected per-user ACL on Windows, code path not run) |
| Rollback | the old `soundcraft.exe` is restored **byte for byte** |
| Token leak | the token appears **0 times** in all script output and the update log, while the same search finds it in the backup copy |

This was run on Linux PowerShell, **not** Windows (U1, U2).

**Downgrade** (a session saved by Hotfix 03, opened by a Hotfix 02 CLI built from `d75158f`):
- **Files:** the session files have identical field sets. Hotfix 02 opens, inspects, bounces and
  re-saves the session, and Master mute, playlists and the reference (with `listening`) survive
  the round trip. No session or preference format changed in this pass: the only new UI field is
  `#[serde(skip)]`, and the model is unchanged.
- **Master mute:** **behaviour differs.** Hotfix 02 ignores Master mute, so a session saved with
  Master muted exports **audibly** in Hotfix 02 (−15.1 LUFS) but silently, with a warning, in
  Hotfix 03.
- **Reference A/B:** stored, but silent in Hotfix 02, as before.
- **Control channel:** after a rollback the app speaks protocol 1 again, **without
  authentication**. The `control-token` file is ignored. The SDKs then report "protocol 1"; the
  CLI shows "unknown method `auth`".
- **"before take" playlists:** these are ordinary playlists to Hotfix 02.

## 8. Dependencies and licences

- **Third-party crates:** none added. `getrandom 0.3.4` and `windows 0.54.0` were already in
  `Cargo.lock` (via cpal and others).
- **New internal crates:** `soundcraft-wasapi-native` and `soundcraft-mf-encode`. Both are
  isolated unsafe FFI crates (unsafe denied crate-wide and allowed only in `ffi.rs`, with a
  SAFETY comment on every block), registered in the xtask layer table and in AGENTS.md.
- **Vendor documentation followed:** Microsoft Learn for IAudioClient / IAudioClient3 (low-latency
  audio), IMFSinkWriter, and the MP3 and AAC encoder attributes.
- **Not accepted:** no third-party licence terms, SDK downloads, plugin updates or virtual-device
  installs.

## 9. Pending on the target machine

See [TARGET_PENDING.md](TARGET_PENDING.md), which lists audio A1–A6, plugins P1–P2, recording
R1–R2, monitoring M1–M2, export E1–E3, control C1–C4 and update U1–U2.

Nothing in this pass is observed on Windows, so nothing should be described as working there
until those checks pass.
