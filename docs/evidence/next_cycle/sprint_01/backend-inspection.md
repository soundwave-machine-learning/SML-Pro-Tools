# B06 — audio backend / ASIO / packaging inspection (environment P)

| Item | Finding | Source |
|---|---|---|
| Audio library | cpal 0.16.0 (`Cargo.toml` workspace dependency) | `Cargo.lock` |
| Windows host compiled | WASAPI (cpal default on Windows). ASIO needs cpal feature `asio`, which requires the Steinberg ASIO SDK (`CPAL_ASIO_DIR`) and LLVM/clang (bindgen) on the build host; the SDK is not in this container and its licence must be accepted by the owner. Not enabled. | `/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/cpal-0.16.0/Cargo.toml` features |
| Linux host compiled | ALSA (no devices in the container); JACK feature not enabled | same |
| Windows build here | `x86_64-pc-windows-gnu` cross-build (mingw), unsigned, never executed on Windows; native MSVC packaging `packaging/windows/package.ps1` needs PowerShell + MSVC | `docs/evidence/sprint_04/artifact-manifest.json` (prior cycle) |
| Signing | outside authorization | — |

Status: B06 stays OPEN. Portable preparation possible here: none beyond keeping the WASAPI label
honest in the Playback Engine window (it already reports the host name from cpal). The ASIO
build/test is a target-machine task with the SDK present (TARGET_PENDING).
