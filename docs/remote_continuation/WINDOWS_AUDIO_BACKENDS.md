# Windows audio backends: WASAPI and ASIO (X04 / RC07 audit)

Inspected 2026-10-12 in environment P against the pinned sources (`Cargo.lock`: cpal 0.16.0,
registry copy `cpal-0.16.0/`); no Windows host, so nothing here was executed on Windows (W).

## What the build uses today
- **WASAPI** is cpal's default Windows host and is compiled into every Windows build (MSVC
  release workflow and the GNU cross-build alike). Shared mode; buffer size as negotiated by
  the device (the Playback Engine dialog shows what was granted).
- Device/host discovery: `crates/playback/src/device.rs` enumerates `cpal::available_hosts()`
  and every device of each host, so any host compiled in appears in the Playback Engine list
  with its devices; a request a device cannot honour is an error, never a silent substitute.
- **ASIO is not compiled in.** cpal only offers `HostId::Asio` with its `asio` cargo feature.

## What enabling ASIO needs (from cpal 0.16's README and build script)
1. The Steinberg ASIO SDK. Its licence is Steinberg's (proprietary; the owner must accept it;
   SoundCraft must not redistribute it). **Caution:** if `CPAL_ASIO_DIR` is not set, cpal's
   `asio-sys` build script downloads the SDK from steinberg.net into the temp directory by
   itself — set `CPAL_ASIO_DIR` to an SDK you downloaded and accepted deliberately.
2. LLVM/clang for bindgen (`LIBCLANG_PATH`), and Visual Studio's MSVC tools (the build script
   runs `vcvarsall.bat`). The MSVC target (`x86_64-pc-windows-msvc`), not the GNU cross-build.
3. Build with the feature: `cargo build --release -p soundcraft --features asio` (the feature is
   added in Sprint 3 and forwarded to `cpal/asio`; off by default).
4. An ASIO driver for the interface (the manufacturer's, or ASIO4ALL for testing).

With the feature on, ASIO appears as a host in Setup › Playback Engine next to WASAPI; nothing
else changes (same negotiation, same diagnostics). ASIO is exclusive per driver on most
interfaces: another app holding the driver makes opening fail with the driver's message.

## Status
| Item | Status |
|---|---|
| WASAPI build | cross-built here (GNU); MSVC build in `.github/workflows/release.yml` (not run by this cycle) |
| WASAPI at runtime | W pending (T10) |
| ASIO feature wiring | Sprint 3 (compile-checked without the feature only: the SDK is not present here and must not be fetched by this environment) |
| ASIO build + runtime | W pending: owner action with the SDK (T11) |
