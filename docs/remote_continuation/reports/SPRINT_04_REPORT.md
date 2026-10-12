# Sprint 04 report — hardened candidate, artifacts and handoff (RC10–RC12)

Revision 3.0, executed 2026-10-12 in environment **P** only (Linux x86_64 container, 4 vCPU,
15 GiB RAM; no audio device, display, Windows host, phone or listener). This report is also the
cycle's final handoff. Nothing here is a release; nothing was pushed, merged or published.

## Source
Candidate (verified source of the packages): **`b641b5a…`** = `test: harden integrated SML Studio
remote candidate` (full SHA in the package manifests). Branch `fable/recording-core`, local
commits only, on top of `9f9b4d3` (the verified start). Later commits change documents only.

## RC10 — meters, exports, durability (PASS_PORTABLE; X05 partial)
- **X05 independent meter check.** Frozen first (`59aa76b`, `meter-x05-frozen.json`): 14 cases,
  tolerances, and the independent implementation — FFmpeg 6.1.1 `ebur128` (system package
  already present; no download). Signals generated from the published case descriptions
  (`examples/meter_vectors.rs`), compared by `tools/compare_meters.py` → `meter.csv`:
  **56 comparisons, 54 within tolerance.** Integrated loudness and gating (X01–X05, X13), max
  momentary, loudness range (X09/X10 exactly 10 and 20 LU in both) and K-weighting across the band
  (sweeps X12/X14, noise bursts X11) agree. The two outside:
  - **X06 (5.1 channel weights):** the two meters agree to 0.006 LU (−25.354 vs −25.360), so the
    weights match an independent implementation; the frozen analytic value (−23.0) was **my
    error** in the frozen file (the right value is ≈ −25.36). Kept as written and reported.
  - **X08 (inter-sample true peak, fs/4 at 45°):** SoundCraft −0.165 dBTP, FFmpeg +0.514 dBTP,
    analytic −0.087 dBTP. SoundCraft is within 0.08 dB of the analytic value; FFmpeg's
    interpolator overshoots by 0.6 dB on this signal. A disagreement, reported as found.
  Not achieved: official EBU test files or a certified meter (not reachable here) — X05 stays
  partial; the meters remain "checked", not "certified" (help text unchanged).
- **Decoded exports** (`tests/rc10_export.rs`): track + Master chains applied exactly once
  (−12.00 dB overall), 32-bit float export equals the offline render sample for sample,
  44.1 kHz / 16-bit and 24-bit files hold what was asked, a declared 4 800-sample tail is exactly
  added with `tail: auto` and absent without it, dither applied once (0.501 LSB RMS on silence;
  none when off). Earlier: missing plugin / mid-render worker fault refuse the export (Sprint 1),
  monitor/reference/speech never in a bounce (e08, Sprint 2 routing test).
- **Durability — defect found and fixed:** a session save wrote the new file and renamed it over
  the old one without flushing it to disk first (an empty file could survive a power cut) and
  never flushed the audio files it references. Now every file is flushed before the rename and
  the folder entry after it; a failed save removes its temporary file, leaves the previous
  session byte-identical and keeps the document unsaved
  (`a_failed_save_keeps_the_previous_file_and_stays_unsaved`). Autosave generations, capture
  disk-full/writer errors, crash recovery and unfinished-take discovery were already covered
  (RELIABILITY_AUDIT.md). Power loss itself: H pending; Windows has no directory flush (stated).

## RC11 — shared services under load (PASS_PORTABLE; H pending)
- **Defect found, reproduced, fixed:** replacing the session (open/close/new) did not touch the
  Bend renderer, so a render still running for the old song was applied to the new song's clip
  with the same id and revision. With the fix disabled the stress test shows **5 stale renders
  landing in the new song** (`stale-render-repro-without-fix.log`); with it, none
  (`Renderer::forget_all`, called by `replace_session`).
- `tests/rc11_stress.rs` (3 rounds, `stress.json`): a 10 000-step fader gesture → last value, one
  undo step (≈ 0.1 s); 5 000 memory locations whose names are injection text → quoted, never a
  plan; Bend renders on every clip cut off by session replacement → bounded queue (5 queued, 3
  refused), stale ones dropped; a model server that hangs, drops connections, sends garbage, a
  `file.delete` proposal or truncated JSON → every request ends within its deadline, second
  submissions refused as busy, cancels honoured, only allowlisted `mix.mute` plans, document
  never changed by a reply; original media byte-identical; resident memory 31 → 35 MB over the
  rounds (bound 64 MB).
- Full-song portable scenario (record → edit → mix → assist → master → export → reopen) rerun:
  `full-song-scenario.json` (all steps ok). Callback compute was measured in Sprint 1
  (64-frame p99 ≈ 6 % of the budget; misses are preemption); 30-min/2 h/8 h runs and native
  timing remain H.

## RC12 — candidate, artifacts, documentation (PASS_PORTABLE; native checklist open)
Both packages were rebuilt from the verified candidate **`b641b5ad938d47a9387f611cf717f9933c7cdb19`**
with a clean tree (`dirty: false` in both manifests) and **attached to this session** (delivery
confirmed by the attachment tool for all five files). Not published anywhere.

| File | SHA-256 | Built by | Verified here |
|---|---|---|---|
| `soundcraft-0.5.0-linux-x86_64.tar.gz` (17.8 MB) | `fe1cd6949353fae37b2621ba0ed4abfbc473ac3565a3c58809b3ae046b46f963` | `packaging/linux/package.sh --formats tar` | clean extraction; all shared libraries resolve; CLI 0.5.0, 533 commands; headless demo script (provider report, speech queued, grounded answers, operate plan, non-loopback model refused); bounce with `tail: auto` + report + decoded verification; save, reopen, meters, bounce again; GUI binary prints its version (no display for a window) |
| `soundcraft-0.5.0-windows-x64-gnu-portable.zip` (18.8 MB) | `3ae863aab5339798afad94a3f5a3dedc175ec1ba962e485f48ff118166de27fd` | `packaging/windows/cross-build.sh` (mingw-w64, `x86_64-pc-windows-gnu`, unsigned) | clean extraction; every file matches its MANIFEST hash; PE32+ GUI/console x86-64; imports only Windows system DLLs. **Not executed** (no Windows here): T10 |

Manifests (`*.MANIFEST.json`, copied into this evidence folder with `SHA256SUMS`) record the
commit, dirty state, toolchain (rustc/cargo/mingw), imports or NEEDED libraries, and every file's
hash. Rebuild: check out `b641b5a` and run the same two scripts (`package-linux.log`,
`package-windows.log`, `package-smoke.log`). A first Windows run recorded three ledger CSVs I was
editing as uncommitted; that zip was discarded and the zip above rebuilt with a clean tree (both
runs are in `package-windows.log`). The signed MSVC MSI/zip come from
`packaging/windows/package.ps1` on a Windows host (release workflow; not run).

Documentation kept in step with the build: USER_GUIDE (Bend crossfades, assistant model and
speech, VST3 editors on Windows), help limits, RUNTIME_MANIFEST, WINDOWS_AUDIO_BACKENDS,
RELIABILITY_AUDIT, TARGET_PENDING (T01–T14), RESUME.

## Final ledgers
- **RC01–RC12:** `docs/remote_continuation/RC_STATUS.csv`
  - RC01 PASS · RC02 PASS_PORTABLE (H timing) · RC03 PASS_PORTABLE (keyboard W/H) ·
    RC04 **BLOCKED** (real model inference; client/validation PASS_PORTABLE) ·
    RC05 PASS_PORTABLE on synthetic speech (H/L/M) · RC06 PASS (P) ·
    RC07 PASS_PORTABLE (cross-built; W) · RC08 PASS_PORTABLE protocol (M) ·
    RC09 PASS_PORTABLE headless (W/M) · RC10 PASS_PORTABLE (X05 partial) ·
    RC11 PASS_PORTABLE (H) · RC12 PASS_PORTABLE (native checklist open).
- **R01–R38:** `docs/remote_continuation/GAP_MATRIX.csv` (baseline status at `9f9b4d3`, evidence
  class, current status, current evidence, what is still missing, owning RC). Changed this cycle:
  R04, R05, R06, R11, R21, R24, R30, R31, R33, R34, R35, R36. Still PARTIAL: R13, R19, R21, R31,
  R32, R35, R37, R38. AWAITING_EXTERNAL_PLUGIN: R23, R25–R29 (the SML plugins are separate
  projects; none was written here).
- **Carried audit gaps C01–C08:** `CARRIED_GAPS.csv` — all addressed in software; C02 (Windows
  editor) and C06 (packages) need W/H execution, C05's language model is BLOCKED.

## Runtime / model / licence state
`RUNTIME_MANIFEST.md`: no model ships (BLOCKED in P; owner can point the assistant at a local
OpenAI-compatible server, loopback only); speech synthesis and recognition are this project's
own code (MIT OR Apache-2.0), no weights; ffmpeg was used only as a test instrument, not shipped.

## Windows build / backend status
`WINDOWS_AUDIO_BACKENDS.md`, Sprint 3 report: WASAPI built in; ASIO is an off-by-default
feature needing the Steinberg SDK on a Windows MSVC host; the VST3 editor window (HWND) is
cross-built and cross-linted; the native test workflow exists and was not triggered.

## Measurements (all P)
Gates: baseline 678 passed; Sprint 1 683; Sprint 2 698; Sprint 3 700; Sprint 4 704 (0 failed
each time on the final attempt). Callback (Sprint 1, release): 64-frame p99 80 µs of 1333 µs.
Assistant (Sprint 2, release): rule answer p50 1.1 ms, recognition 1.5–8.5 ms, synthesis 2.7 ms,
peak RSS 42 MB, no measurable mixer impact. Meters: 54/56 agree with FFmpeg. Stress: memory
bounded, no stale actions.

## Known limitations (stated, not hidden)
Real language-model inference not observed (no model reachable). Speech recognition is an
enrolled-phrase recognizer evaluated on synthetic speech; it fails below ~20 dB SNR. The voice is
robotic and unjudged by listeners. No Windows, macOS, phone, interface, listener or long-run
evidence. Keypad minus is indistinguishable from main-row minus (toolkit). The phone page is
plain HTTP (no microphone, no installable app). Meter conformance is checked, not certified.

## Next native action
Run `TARGET_PENDING.md` in this order: T10 (Windows unzip + WASAPI), T12 (VST3 editor on
Windows, or trigger `windows-native-tests.yml`), T01 (64-frame realtime), T06 (local model),
T07–T09 (microphone, voice, speech never in a take), T13 (1366×768@150 %), T14 (phone), then the
remaining H/L items and the 30-min/2 h/8 h runs.
