# Sprint 01 report — Recording, clock and clean baseline (gate A1)

Package revision 1.6. Branch `fable/recording-core` from `main` = `7414719f3760de1c9359ec9723351a7232442e61`
(equal to the planning SHA). Execution host: Linux x86_64 cloud container, no audio devices, no
Windows target (see `docs/evidence/sprint_01/environment.json`). Budget: wall-time proxy, started
15:16:41Z; Sprint 1 accepted at ~16:00Z.

## Verdict
**A1 PORTABLE: PASS** (all A1 software rows below executed and passing; full `cargo xtask ci` exit 0).
**H1 native recording/device: BLOCKED FOR TARGET VALIDATION** (no Windows host, interface, driver
or ASIO SDK in this container). A mock/fixture input does not prove physical recording.

## Baseline
| Step | Command | Result | Log |
|---|---|---|---|
| First baseline | `cargo xtask ci` | FAIL at clippy: `alsa-sys` build script — missing `libasound2-dev` (environment) | `baseline-attempt1-missing-alsa.log` |
| Fix | `apt-get install libasound2-dev pkg-config libudev-dev`; `rustup target add wasm32-unknown-unknown` | ordinary build deps | — |
| Second baseline | `cargo xtask ci` | fmt ok, clippy ok, test FAIL: `ui-egui tests/theme_docking.rs` needs a wgpu adapter ("No adapter found") — environment | `baseline.log` |
| Fix | `apt-get install mesa-vulkan-drivers libvulkan1` (software Vulkan) | — | — |
| Baseline rerun | `cargo test --workspace --no-fail-fast`; `cargo xtask wasm`; `cargo xtask layers && cargo xtask assets` | all exit 0 (541 tests passed, 0 failed) | `baseline-tests-rerun.log`, `baseline-wasm.log`, `baseline-layers-assets.log` |

Pre-existing source failures: none. All baseline failures were container environment gaps.
During the sprint the container disk allowance filled (linker bus error); fixed by deleting
`target/debug/incremental`, examples and stale test executables, then building with
`CARGO_INCREMENTAL=0` (`ci-sprint1-attempt3-disk-full.log`).

## Work done (contract items 1–8)
1. Baseline: above. Toolchain rustc 1.97.0 vs workspace `rust-version = 1.90` (newer toolchain, no issue observed).
2. **Catalogue removal (R04)**: all 26 upstream bundled effects/instruments removed from every
   product route (insert/instrument menus, `mix.insert`, `mix.instrument`, default instrument on
   new instrument tracks and MIDI import, `engine.plugins`, CLI `plugins`, `audiosuite.process`
   plugin ids, Clip Effects EQ/comp, demo session). Mix engine instantiates only external hosts.
   Retained internal services listed in `plugin-removal-inventory.md`. Window title is now
   "SML Recording" (licenses/notices untouched; crates not renamed). PARTIAL item: the upstream
   DSP implementation modules are still compiled as internal code (unreachable from product
   routes); deleting them is deferred (shared with offline services).
3. **Old sessions (R05)**: removed ids stay as named inactive missing placeholders (id, params,
   state, slot retained; audio passes through); `session.missing_plugins`; exports require
   `acknowledge_missing: true`. New graph strategy: `Session.mix_profile.graph_version`
   (absent in legacy files = 1, new sessions = 2) — audited/introduced now, used in Sprint 3.
4. **Device/buffer negotiation (CL02)**: `playback::device` — pure `negotiate()` over real
   capabilities (`enumerate()` via cpal) with explicit errors (no substitution), fixed-buffer
   stream config, ClockLock always Unknown (cpal reports none). `Recorder::open_with(request)`.
   ASIO: cpal's `asio` feature is NOT compiled in this workspace; enabling it needs the Steinberg
   ASIO SDK on a Windows build host — BLOCKED here. WASAPI exclusive mode is not claimed.
   UI device-selection dialog wiring to `open_with` is not done (PARTIAL; default device used by
   the UI, requested-device path available through the API).
5. **Bounded dry disk capture (R03)**: `playback::capture` — preallocated lock-free SPSC ring of
   atomics (whole block or nothing), writer thread to 32-bit float WAV created with
   `create_new` (never overwrites), unique take names, journal written atomically *after* each
   durable media flush, explicit `Overflow`/`WriteFailed` stopped-partial states, crash
   `recover()` keeping exactly the journaled frames. The recorder no longer grows a Vec in the
   callback; the UI records each take to `<session>/Audio Files/` (or temp for unsaved sessions)
   and shows partial-capture status.
6. **Clock (CL01/CL05)**: `playback::clock` — stream generations, integer frame counter,
   transport map with stale-generation rejection, exact in-block offsets/splitting, punch window
   clipping, pre-roll, latency ledger with signs/single application/Unknown flagging, callback
   load with N/Fs denominator. Live output path: renders exactly each callback's frames when
   rates match (removes up to 448 frames of queued audio at 64-frame callbacks; BLOCK=512 kept
   as max block and for the resampling path). Stream-clock telemetry exposed by `Player::stream_clock`.
7. Recording view: existing transport/arm/monitor UI retained; capture tap is the raw callback
   input (dry, before playback/mixer processing). Software monitoring default unchanged
   (follows arm/monitor flags; not forced on). A dedicated simplified "recording view" panel was
   not added (PARTIAL for R08 UI).
8. Later-feature audit: see table below; command index established.

## A1 checks (all RERUN in this container)
| Check | Command | Result | Evidence |
|---|---|---|---|
| Capture identity, frame order/count/channel map, 44.1/48 k, mono/stereo, irregular callbacks (32…1024, 7), unique files | `cargo test -p soundcraft-playback capture::` | 7 passed | `capture-tests.log` |
| Overflow, disk-full, crash recovery, bounded ring, disarmed | `cargo test -p soundcraft-playback -- overflow writer_error crash_recovery ring_memory disarmed` | 5 passed | `failure-recovery.log` |
| CL-T1 (30 min simulated, variable callbacks, generations), CL-T2 (negotiation fixtures, unsupported values), CL-T3 primitives, live path | `cargo test -p soundcraft-playback -- clock:: device:: live_path` | 11 passed | `clock-tests.log` |
| Real capability audit | `cargo run -p soundcraft-playback --example device_audit` | hosts: ALSA; 0 input / 0 output devices (headless) | `device-negotiation.json` |
| Catalogue/migration | engine + mix filtered suites | 7 + 2 passed | `migration-test.log`, `migration-test-mix.log`, `catalogue.json` |
| Full CI | `cargo xtask ci` | exit 0; 581 tests passed, 0 failed | `ci-sprint1.log` |

Integer-PCM conversion ≤1 LSB: not applicable — capture is written as 32-bit float (bit-exact).
Quantified uncommitted loss: at most the frames after the last checkpoint (default flush every
48 000 frames = 1 s at 48 kHz) plus ring contents; recovery reports them as discarded.

## Requirements
| ID | Status |
|---|---|
| R01 device/rate/buffer | IMPLEMENTED (API + tests); UI device picker not wired → PARTIAL; H1 pending |
| R02 independent arm/monitor/transport, dry recording | Existing behaviour retained; dry capture tap verified at ring level; H1 pending |
| R03 bounded disk capture/finalize/recovery | PASS (A1); H1 pending |
| R04 remove bundled catalogue | PASS for product routes; module deletion PARTIAL |
| R05 preserve removed state/media | PASS (A1) |
| R08 simple recording view | PARTIAL (no new view) |
| R24 audio-owned clock, negotiation, provenance | PASS (A1) for primitives/live path; capture→transport placement wiring in UI uses existing record_start (PARTIAL) |
| R25 monitoring/compensation/placement | Ledger/sign primitives PASS (CL-T3 part); modes are Sprint 3 |
| R26 bounded realtime work | Capture path FIXED; remaining callback findings documented in `realtime-audit.md` (PARTIAL) |

## Later-feature audit (source inspection, start of Sprint 2/3)
| Area | State found |
|---|---|
| V01–V05, V09, V10 | Existing commands: select/separate/heal/trim/duplicate/consolidate/fades_create/strip_silence/clip.gain/clip.group/locks — working, tests to add in Sprint 2 |
| V06 comp | `track.playlist_promote` exists (range copy from alternate playlist) |
| V07 punch/loop | UI punch + loop record exist (`start_recording`); scheduler primitives now tested |
| V11 stretch | `dsp::offline::time_stretch` (WSOLA) retained as service |
| V12 pitch | external host only; no owner plugin → pending |
| N01–N10 | Old `edit.nudge` = one undifferentiated step; dedicated engine added (Sprint 2) |
| B01–B06 | Busses/aux/sends existed; one-step workflow missing → added (Sprint 3 module) |
| M01–M05, SL01–SL06 | No profile/slots/width; existing pan law (-3/-4.5/-6/linear 0) |
| VM01–VM05 | No Visual view |
| CL03/CL04 | Existing PDC over inserts; no Tracking/Mixing mode |

## Next
Sprint 2 (A2): Nudge engine already landed with N01–N06/N08 tests in this commit; add N07 key
map, vocal V-row tests and staged V-E2E.
