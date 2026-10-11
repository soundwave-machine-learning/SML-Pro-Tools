# Realtime callback audit (CL05) — Sprint 1

Provenance: INSPECTED = source read on branch `fable/recording-core` (base `7414719`);
RERUN = an executed test named below. No native profiling was performed (no audio hardware in the
execution container; see `environment.json`).

## Output callback (`crates/playback/src/lib.rs`, `AudioState::fill`)

| Item | Before (base 7414719, INSPECTED) | After this sprint | Status |
|---|---|---|---|
| Fixed `BLOCK = 512` render batching | Every render produced 512 frames into `pending`; a 64-frame device callback consumed a 512-frame render, leaving up to 448 frames of queued audio and delaying commands by up to one 512 block. | When session rate = device rate and speed = 1 the live path renders exactly the frames the callback still needs (up to the 512 max block), so no audio is queued beyond the current callback; commands drained at the start of each callback take effect in that callback. The resampling path (rate mismatch / varispeed) still renders 512-frame blocks (explicit, documented). | CHANGED; unit test `playback::tests::live_path_renders_exactly_the_callback_size` (RERUN) |
| Command channel | `std::sync::mpsc` drained with `try_recv` in a loop (unbounded queue; drain is non-blocking). | Unchanged. Unbounded queue remains a CL05 finding: bursts are drained in one callback (bounded by what the UI produced). | OPEN (documented) |
| Plugin destruction | Retired instances returned to the UI thread on a channel (`take_retired`). | Unchanged. | OK (INSPECTED) |
| Plugin state capture | `CaptureStates` answered between blocks *on the audio thread* (allocates, calls plugin `save_state`). | Unchanged. Documented as not realtime-safe; the host should capture while stopped. | OPEN (documented) |
| Meter snapshot | `try_lock` only; `HashMap::insert` on first sight of a track (allocation once per new track). | Unchanged. | MINOR (first-block allocation) |
| Session swap | `Arc<Session>` swapped; old Arc dropped on the audio thread (may free a whole session). | Sprint 4 follow-up: the replaced `Arc<Session>` is sent back on the garbage channel and freed on the UI thread (`replaced_sessions_are_dropped_off_the_audio_thread`). The std channel itself may allocate a block every few dozen messages (same class as the existing plugin return path). | FIXED (RERUN) |
| Stream clock | None: position = rendered-block end. | `StreamClock` (generation + integer frames serviced) advanced per callback; effective callback size range recorded. | NEW (`clock.rs`, tests RERUN) |

## Input callback (`crates/playback/src/record.rs`)

| Item | Before (INSPECTED) | After | Status |
|---|---|---|---|
| Take storage | `Mutex<Vec<f32>>` grown with `extend_from_slice` inside the callback (heap growth on the audio thread, whole take in RAM, `try_lock` failure buffered in a growing local backlog of up to 1 Mi samples). | Callback pushes into a preallocated lock-free `CaptureRing` (atomics, no allocation, whole block or none); a writer thread streams to a float WAV on disk with a durable journal. No take is held in RAM. | FIXED (`capture.rs`, tests RERUN) |
| Monitor ring | `Mutex<VecDeque>` with `try_lock`; `VecDeque::extend` may reallocate when above initial capacity (capacity preallocated to 1 s; cap 0.5 s so in practice no growth). | Unchanged. | OK (INSPECTED) |
| Device choice | `default_input_device()` + `default_input_config()` only. | `Recorder::open_with(request)` negotiates the requested host/device/rate/buffer (`device.rs`) and errors instead of substituting. | CHANGED |

## Mix engine (`crates/mix`)

Steady-state rendering: **0 heap allocations/reallocations over 200 blocks** in serial strip mode
(RERUN, Sprint 4: `cargo test -p soundcraft-mix --test realtime_alloc`, graph v2 with buses, sends,
FX return, width, a Tape-slot fixture, profile pans and live input). The per-block strip work list
was a fresh `Vec` before Sprint 4 and is now a reused buffer (FIXED). Strip parallelism is
adaptive: rayon workers only when ≥4 strips host processing (measured 3–5× scheduler overhead for
light strips; `docs/evidence/sprint_04/callback-profile*.csv`); in parallel mode rayon itself
allocated 3 times in 200 blocks (OPEN, third-party scheduler; documented).
`sync` allocates and runs inside `render` when the session structure changes (document edits);
in realtime use that happens on the audio thread after a session swap — a known CL05 gap
(graph construction should move off the callback). Recorded as OPEN, not fixed in this pass.

## Not measured

Callback p99 load, xruns, deadline misses and response distributions (CL-T5 native, CL-T6/7)
need the target machine. `CallbackStats` (load = compute / (N/Fs)) exists for that run.
