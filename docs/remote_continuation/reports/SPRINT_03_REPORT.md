# Sprint 03 report — Windows delivery and remote/studio usability (RC07–RC09)

Revision 3.0, executed 2026-10-12 in environment **P** only (Linux x86_64 container; mingw-w64
cross toolchain; no Windows host, no display, no phone). Windows target work is **cross-built
and cross-linted, never executed** here: every Windows runtime claim stays W pending.
Evidence: `docs/remote_continuation/evidence/sprint_03/`.

## Source
Base `710120c` (Sprint 2 closed). Local commits: `48e58b6` (cross-build script, manual Windows
workflow, backend audit), `d742ee6` (checkpoint), `219d9b5` (evidence), the milestone `f8163e9`
`build: improve Windows delivery and remote studio usability`, `bc9cacd` (compact-toolbar fix
after gate attempt 2) and a closing evidence commit. No push.

## RC07 — Windows build, package, backend (PASS_PORTABLE; W pending)
- **X04 discovery** (INSPECTED): `.github/workflows/release.yml` already builds, signs (when
  secrets exist) and packages MSVC x64/x86/arm64 via `packaging/windows/package.ps1`; it was not
  triggered (no push). Local toolchain: `x86_64-pc-windows-gnu` + mingw-w64 gcc 13; no Windows
  host, no wine, no runner.
- **Reproducible cross-build** `packaging/windows/cross-build.sh`: builds `soundcraft.exe` +
  `soundcraft-cli.exe` (`--locked`), stages licences and a README, writes `MANIFEST.json` (full
  commit, dirty state and the dirty paths, toolchain versions, target, audio backends, imports
  per .exe, SHA-256 per file) and a deterministic zip (fixed timestamps) with a `.sha256`. Run
  once at `d742ee6` (`windows-cross-build.log`, `artifact-manifest-interim.json`): the
  executables import only Windows system DLLs (KERNEL32, USER32, GDI32, OLE32, DXGI, OPENGL32,
  UIAutomationCore, msvcrt, …) — no mingw runtime DLLs to ship. The deliverable zip is rebuilt
  from the final source in Sprint 4.
- **Native test workflow** `.github/workflows/windows-native-tests.yml` (`workflow_dispatch`
  only): `cargo test --workspace` on windows-latest (runs the VST3 editor window natively) and
  an unsigned MSVC build as an artifact. Created, not triggered.
- **Windows VST3 editor window (carried gap C02)**: `crates/vst3-host/src/ffi.rs` `mod window`
  for `cfg(windows)`: a Win32 window (registered class, owned by the app's active window so it
  floats above it, client area = the plugin's `HWND` parent, `kPlatformTypeHWND`), close hides it
  so the editor link tears the view down, resizes via `AdjustWindowRectEx` + `SetWindowPos`,
  UI-thread only (`on_main_thread` = Rust's "main" thread). Every `unsafe` block carries a
  `// SAFETY:` comment; `windows-sys 0.61` (already in the lock file). Probe/capability now
  report "available" on Windows; `tests/editor_main_thread.rs` opens and closes the real window
  on Windows. Cross-checked: `cargo clippy --workspace --all-targets --target
  x86_64-pc-windows-gnu -D warnings` exit 0 (`windows-clippy.log`). Not run on Windows (T12).
- **Windows-only defects found by the cross-lint** (`windows-clippy-attempt1.log`): an unused
  variable in a vst3-host test, and `autosave::sync_dir` silently doing nothing on Windows (no
  directory flush there) — now explicit, with the NTFS-journal reliance stated; power-loss
  durability on Windows is W pending.
- **WASAPI / ASIO** (`WINDOWS_AUDIO_BACKENDS.md`): WASAPI is cpal's default and always built.
  ASIO wired as an **off-by-default** feature (`soundcraft --features asio` → playback →
  `cpal/asio`); not built here: it needs the Steinberg SDK (proprietary; cpal's build script
  would otherwise fetch it by itself), LLVM and MSVC (T11).
- **Paths** (INSPECTED): the desktop executable is its own plugin worker (`--plugin-worker`),
  so the portable zip needs no other program; preferences, logs, autosaves, `assistant.json`
  live in the per-user settings folder; no model or weights are bundled.

## RC08 — desktop/phone flows (PASS_PORTABLE protocol; M pending)
- Existing coverage rerun: pairing (wrong-code lockout, expiry), Origin check, duplicate
  request ids, stale revisions with two clients, record-nonce replay, revocation, logout,
  bounded queue/rate limit/meter coalescing.
- **New**: `capture_survives_disconnect_and_reconnect_sees_current_state` — while the desktop
  records, a dropped socket, a half-sent request and a revocation never stop capture or queue a
  transport request; re-pairing sees "recording" at the current revision with a fresh nonce; the
  pre-disconnect record nonce is refused.
- **Secure-origin reporting**: `/state.capabilities` states no HTTPS, not a secure origin, no
  phone microphone and that a disconnect never stops the desktop; the page also shows the
  browser's own `isSecureContext` result. (`remote.log`)

## RC09 — small screens and keyboard (PASS_PORTABLE headless; W/M pending)
- **Measured**: the Edit toolbar's full row is 2236 logical points wide, so it overflowed every
  common laptop/desktop width, leaving transport and Nudge at the far end of a sideways scroll.
- **Change**: when the row does not fit, the transport/record group and the Nudge panel are
  pinned on the right at their natural widths and only the other groups scroll; the full row is
  kept when it fits. Test `transport_and_nudge_are_reachable_on_small_screens` renders the real
  toolbar headless at 910.7×512 (1366×768 at 150 %), 1280×720, 1920×1080 and 2560×1440 in light
  and dark themes: both groups fully on screen, not overlapping, ≥ 250 points left for the
  scrolling groups (`layout.log`; e.g. 910.7: scroll 0–321, Nudge 329–719, transport 727–903).
  In the compact strip the edit and audio tools come first; from 1366 points wide they are
  fully in view, below that they scroll (edit modes also have F1–F4).
- **Keyboard**: the Nudge buttons state the fallback (keypad Num± where recognised; plain ± when
  no text field/plugin editor/dialog has focus; remappable `nudge.apply`). Modifier cleanup is
  egui-winit's (modifiers cleared on focus loss, INSPECTED) plus the app's existing gesture
  cancel on focus loss; no new code needed.

## Gate
`CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only cargo xtask ci`: attempt 1
failed rustfmt only (`sprint3-ci-attempt1-fmt.log`); attempt 2 caught a **regression from the
compact toolbar** — at 1500 points wide the Listen tool button was outside the visible strip,
so the existing kittest `listen_runs_while_held_and_cleans_up_on_release_or_escape` could not
select it (`sprint3-ci-attempt2-listen-regression.log`). Fixed by listing the edit and audio
tools first in the compact strip (the full row is unchanged), and the layout test now also
asserts the tools are fully in view from 1366 points up and top-aligned (`bc9cacd`).
**Attempt 3: fmt, clippy, test, assets, layers, wasm all ok; 700 passed / 0 failed / 1 ignored in
71 suites; EXIT 0** (`sprint3-ci.log`).

## Not done / external gates
T10 WASAPI run on Windows, T11 ASIO build, T12 VST3 editor on Windows, T13 real 1366×768@150 %
screen, T14 real phone reconnect. Signing, MSI and the release workflow were not run.

## Next safe action
Sprint 4 (RC10–RC12): independent meter check (X05), export/durability faults, stress and
backpressure, rebuild Linux and Windows packages from the final source with manifests and
hashes, attach them, final reports.
