# Sprint 04 report — Target performance, owner workflow and delivery (gate A4)

Entered after A1–A3 portable PASS (with named PARTIAL rows) and the carry-over commits. Host:
Linux x86_64 container — no Windows machine, audio interface, driver, owner plugins or owner.

## Verdict
**A4 PORTABLE REGRESSION: PASS** — `cargo xtask ci` exit 0 on the final source (594 tests in 56
test binaries, fmt, clippy -D warnings, assets, layers, wasm), `docs/evidence/sprint_04/regression.log`.
**H1–H4 / CL-T6 / CL-T7 / owner review: BLOCKED FOR TARGET VALIDATION** (no target access). The
exact pending procedure is `docs/TARGET_MACHINE_GAUNTLET.md`. RELEASE READY: **NO**.

## Work done (contract items 1–5)
1. **Profiling (headless, release build)**: `crates/engine/examples/sml_profile.rs` — 16 playback +
   2 armed live-input tracks, Vocal Bus, FX return, Master, graph v2, width ≠ 100 %; 60 s of
   consecutive callbacks at 64/128/256 frames, 48 kHz; load = compute/(N/Fs).

   | Frames | budget | p50 | p99 | max | p99 load | overruns |
   |---|---|---|---|---|---|---|
   | 64 | 1333 µs | 92.6 µs | 289.2 µs | 3602 µs | 0.217 | **13 / 45 000** |
   | 128 | 2667 µs | 99.0 µs | 310.1 µs | 1654 µs | 0.116 | 0 / 22 500 |
   | 256 | 5333 µs | 123.3 µs | 245.5 µs | 2113 µs | 0.046 | 0 / 11 250 |

   Fader command acknowledgment (event → model revision, engine only): p50 16.6 µs, p95 25.5 µs,
   max 415 µs (1000 samples). These are container measurements with a shared, noisy scheduler —
   **not** native results and not a CL-T7 PASS. The 64-frame overruns are a real finding to check
   on the target (candidate causes: rayon strip scheduling, session fingerprint hashing per block,
   allocation on structure change). No optimization was made without a native measurement.
2. **Native H1–H3, CL-T6/7**: not executable here; gauntlet written with topologies, counts,
   thresholds and evidence file names.
3. **Native vocal workflow (VM-T8/V-E2E native, N07/N09/VM-T7/SL-T8, screen/scale matrix)**:
   not executable; the software V-E2E (Sprints 2–3) passes; UI renders exist for Console/Visual/
   Split and the Recording panel at 1600×1000 @2x only.
4. **Packaging** (inspected `packaging/linux/package.sh`, `packaging/windows/package.ps1`):
   - Linux x86_64 tarball built with the repo script (`--skip-build --formats tar` after
     `cargo build --release --locked -p soundcraft -p soundcraft-cli`); clean-extraction CLI smoke
     PASS (headless workflow + bounce); GUI cannot start without a display here.
   - Windows: `package.ps1` needs PowerShell + MSVC on Windows — not runnable. A **cross-built**
     unsigned portable zip (`x86_64-pc-windows-gnu`, mingw-w64; only Windows system DLL imports)
     was produced for owner testing. Cross-compiling is **not** a Windows runtime pass. WASAPI only
     (ASIO not compiled; needs the ASIO SDK on a Windows build host).
   - Artifacts (gitignored, local): see `artifact-manifest.json` for paths, sizes, sha256.
5. **Docs**: `docs/USER_GUIDE.md` (recording, Nudge, vocal comping, buses/FX returns, slots,
   profile/pan/width, Console/Visual/Split, monitoring/exports, device/latency/tracking, plugin
   risks — see §10), `docs/TARGET_MACHINE_GAUNTLET.md`.

## Evidence
`regression.log`, `release-build.log`, `profile-run.log`, `callback-profile.csv`,
`response-profile.csv`, `package-linux.log`, `package-smoke.log`, `package-smoke.md`,
`windows-cross-build.log`, `artifact-manifest.json`.

## Requirements touched
R09 measured responsiveness: headless measurements only (PARTIAL; native pending). R10 regression
PASS, truthful candidate artifacts produced, manual written; native delivery checks pending.
R14 complete vocal walkthrough: software stages PASS; native/owner walkthrough pending.
