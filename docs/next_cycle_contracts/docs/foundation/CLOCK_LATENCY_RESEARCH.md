# Clock, latency and responsiveness research — revision 1.7
Date: 2026-10-11. Documentation and pinned source inspection; no native measurements or listening tests.

## Findings
Pro Tools native operation trades smaller hardware buffers for shorter monitoring delay and less CPU margin [A1]. HDX Hybrid Engine uses dedicated DSP for selected monitoring chains [A2]. Its hardware-backed latency claims cannot be transferred to our native Rust engine. The comparison target for this project is Pro Tools Native on the SAME device, driver, rate and buffer, not HDX/Carbon hardware. Avid also discusses reduced system delay improving control-surface response, which should not be mistaken for a universal GUI benchmark.

Reason 10.3 documentation explains that matching internal processing batches to the selected audio buffer improves some plugin throughput but can alter feedback-path delay [R1]. Reason 12.2.9 describes analyzing the song to use an appropriate number of rendering threads rather than always starting many workers [R2]. These support deliberate scheduling and block-size decisions. They do not prove Reason has lower latency than every other DAW. Our specified graph rejects feedback cycles; do not add Reason-style feedback routing merely to mimic that behavior.

Microsoft documents a device position paired with a performance-counter timestamp for WASAPI [W1]. Device-clock units must be interpreted using the API's frequency, not assumed to equal samples. This is a useful example for backend clock abstraction; an ASIO implementation must use its own correctly defined time/sample-position API.

## Four different problems
1. Hardware sample clock: the interface controls conversion cadence. App timing work cannot improve the physical oscillator or promise lower converter jitter.
2. Engine/transport clock: schedule audio and edits against integer sample positions, independent of UI timers. Tempo and visual playhead are mapped onto that timebase.
3. Monitoring delay: input conversion/driver queues, native processing/plugin delay, output queues/conversion. Compensation aligns parallel paths by delaying earlier ones; it cannot make a live signal arrive before it was captured.
4. Response and recording placement: a UI can redraw quickly yet control queued audio late; a recording can be placed correctly even when its monitored sound was delayed. Measure these separately.

## Pinned source observations
Repository https://github.com/soundwave-machine-learning/SML-Pro-Tools at 7414719f3760de1c9359ec9723351a7232442e61:
- crates/playback/src/lib.rs lines 1–150: BLOCK=512; engine prepared with that block; pending output buffers/read index; session sample rate separate from device rate; mpsc commands; meter snapshot mutex; plugin state capture initiated between blocks; silent transport fallback described.
- crates/playback/Cargo.toml: CPAL dependency with no ASIO feature enabled in that manifest. Workspace/features still require inspection; this does NOT prove ASIO is unavailable in every build.
Audit full callback, resampling, command draining and input/output synchronization before changing them. A fixed 512 render block can contribute pending audio/control delay depending on implementation, but this inspection is not a measurement of round-trip latency. A try_lock comment alone does not prove callback allocation and scheduling safety.

## Primary references
- A1 Avid native buffer guidance: https://www.avid.com/pro-tools/user-guide/change-buffer-size
- A2 Avid HDX Hybrid Engine explanation, 2021-06-24: https://www.avid.com/resource-center/hdx-hybrid-engine
- R1 Reason 10.3 batching/feedback timing, updated 2019-05-23: https://help.reasonstudios.com/hc/en-us/articles/360019449434-Some-sounds-are-out-of-sync-in-Reason-10-3-and-later-what-s-up
- R2 Reason 12.2.9 threading, updated 2023-01-11: https://help.reasonstudios.com/hc/en-us/articles/7847406117266-Max-Audio-Threads-in-Reason-12-2-9-and-later-a-new-function-for-optimizing-performance
- W1 Microsoft IAudioClock::GetPosition: https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclock-getposition

Proposed targets and architecture are in CLOCK_LATENCY_SPEC.md. They are original engineering requirements, not measurements or commercial-DAW guarantees.
