//! SoundCraft native WASAPI output (HF07, continued): output modes the audio library (cpal 0.16, shared mode only)
//! does not offer.
//!
//! - **WASAPI Exclusive** (`IAudioClient::Initialize` with `AUDCLNT_SHAREMODE_EXCLUSIVE`,
//!   event-driven): the app owns the device; the buffer is the requested period, aligned to what
//!   the driver accepts (`AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED` → the driver's size). Formats are
//!   tried in order 32-bit float, 32-bit integer, 24-bit packed, 16-bit integer, because many
//!   drivers accept only integer formats in exclusive mode. Other apps cannot play while it is open.
//! - **WASAPI Low Latency** (`IAudioClient3::InitializeSharedAudioStream`, shared mode, event
//!   driven): smaller shared periods when the driver reports them
//!   (`GetSharedModeEnginePeriod`: default, fundamental, min, max). Runs at the device's mix
//!   format (rate and channels); other apps keep playing.
//!
//! Output only; the input (recording) stays on the shared cpal stream. All COM work, buffer
//! management and format conversion run on the backend's own thread, with "Pro Audio" MMCSS
//! priority; the render closure is the same `AudioState::fill` the cpal path calls.
//!
//! Status: compiled for Windows (cross-build); **not yet run on Windows hardware**. The pure
//! period and format helpers below are unit-tested on every platform.
//!
//! Like `soundcraft-vst3-host`, an isolated `unsafe` crate (craftrules never-crash): `unsafe` is
//! denied crate-wide and allowed only in the private, Windows-only `ffi` module, where every block
//! carries a `// SAFETY:` comment. The public API is safe and returns `Result`.

/// A native WASAPI output mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Exclusive,
    LowLatencyShared,
}

/// Host names these modes appear under in Audio Setup and device requests.
pub const EXCLUSIVE_HOST: &str = "WASAPI Exclusive";
pub const LOW_LATENCY_HOST: &str = "WASAPI Low Latency";

/// The mode a host name selects, if it is one of these.
pub fn mode_for_host(host: &str) -> Option<Mode> {
    if host.eq_ignore_ascii_case(EXCLUSIVE_HOST) {
        Some(Mode::Exclusive)
    } else if host.eq_ignore_ascii_case(LOW_LATENCY_HOST) {
        Some(Mode::LowLatencyShared)
    } else {
        None
    }
}

/// A frame count as a WASAPI `REFERENCE_TIME` (100 ns units), rounded to nearest — the period
/// Microsoft's exclusive-mode alignment procedure computes from the aligned buffer size.
pub fn frames_to_hns(frames: u32, rate: u32) -> i64 {
    ((10_000_000.0 * f64::from(frames) / f64::from(rate.max(1))) + 0.5) as i64
}

/// A `REFERENCE_TIME` as frames at `rate`, rounded to nearest.
pub fn hns_to_frames(hns: i64, rate: u32) -> u32 {
    ((hns as f64 * f64::from(rate) / 10_000_000.0) + 0.5).max(0.0) as u32
}

/// Shared low-latency period: the smallest multiple of `fundamental` that is at least the request
/// (default: the minimum), clamped to `min..=max`. All in frames, as `GetSharedModeEnginePeriod`
/// reports them.
pub fn shared_period(request: Option<u32>, fundamental: u32, min: u32, max: u32) -> u32 {
    let f = fundamental.max(1);
    let want = request.unwrap_or(min).clamp(min, max.max(min));
    let k = want.div_ceil(f);
    (k * f).clamp(min, max.max(min))
}

/// Sample formats tried for exclusive mode, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleKind {
    F32,
    I32,
    I24,
    I16,
}

impl SampleKind {
    pub const EXCLUSIVE_ORDER: [SampleKind; 4] = [SampleKind::F32, SampleKind::I32, SampleKind::I24, SampleKind::I16];
    pub fn bytes(self) -> usize {
        match self {
            SampleKind::F32 | SampleKind::I32 => 4,
            SampleKind::I24 => 3,
            SampleKind::I16 => 2,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            SampleKind::F32 => "32-bit float",
            SampleKind::I32 => "32-bit integer",
            SampleKind::I24 => "24-bit integer",
            SampleKind::I16 => "16-bit integer",
        }
    }
}

/// Writes interleaved float samples into a device buffer of `kind` (little endian), clamping
/// to full scale. `out` holds `src.len() * kind.bytes()` bytes.
pub fn convert_into(src: &[f32], kind: SampleKind, out: &mut [u8]) {
    let b = kind.bytes();
    for (x, o) in src.iter().zip(out.chunks_exact_mut(b)) {
        let v = x.clamp(-1.0, 1.0);
        match kind {
            SampleKind::F32 => o.copy_from_slice(&v.to_le_bytes()),
            SampleKind::I32 => o.copy_from_slice(&((f64::from(v) * 2_147_483_647.0).round() as i32).to_le_bytes()),
            SampleKind::I24 => {
                let i = (f64::from(v) * 8_388_607.0).round() as i32;
                o.copy_from_slice(&i.to_le_bytes()[..3]);
            }
            SampleKind::I16 => o.copy_from_slice(&((v * 32_767.0).round() as i16).to_le_bytes()),
        }
    }
}

/// What a native mode reports for one device at one rate.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeCaps {
    pub rate: u32,
    pub channels: u16,
    /// Buffer range in frames (exclusive: minimum and default device period; low latency: the
    /// shared engine's minimum and maximum period).
    pub min_frames: u32,
    pub max_frames: u32,
    /// Low latency: the period granularity (frames); exclusive: 1 (aligned by the driver).
    pub step_frames: u32,
    pub sample_kind: SampleKind,
}

/// A running native stream; dropping it stops and joins its thread.
pub struct NativeStream {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// Frames per device period actually granted.
    pub period_frames: u32,
    pub rate: u32,
    pub channels: u16,
    pub sample_kind: SampleKind,
    /// The driver's stream latency (`GetStreamLatency`), frames, when reported.
    pub stream_latency_frames: Option<u32>,
}

impl Drop for NativeStream {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod ffi;
#[cfg(windows)]
pub use ffi::{caps, open};

/// Not available off Windows.
#[cfg(not(windows))]
pub fn caps(_device: &str, _mode: Mode, _rate: u32) -> Result<NativeCaps, String> {
    Err("native WASAPI modes exist only on Windows".into())
}

/// Not available off Windows.
#[cfg(not(windows))]
pub fn open(
    _device: &str,
    _mode: Mode,
    _rate: u32,
    _channels: u16,
    _frames: Option<u32>,
    _on_lost: Box<dyn Fn(u8) + Send>,
    _render: Box<dyn FnMut(&mut [f32], usize) + Send>,
) -> Result<NativeStream, String> {
    Err("native WASAPI modes exist only on Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_math() {
        assert_eq!(frames_to_hns(480, 48_000), 100_000, "10 ms");
        assert_eq!(frames_to_hns(64, 48_000), 13_333);
        assert_eq!(frames_to_hns(128, 48_000), 26_667);
        assert_eq!(hns_to_frames(26_667, 48_000), 128);
        assert_eq!(hns_to_frames(30_000, 44_100), 132);
        // Shared low-latency: multiples of the fundamental within min..max.
        assert_eq!(shared_period(None, 48, 144, 480), 144, "default: the minimum");
        assert_eq!(shared_period(Some(128), 48, 144, 480), 144, "below the minimum: the minimum");
        assert_eq!(shared_period(Some(200), 48, 144, 480), 240, "rounded up to the fundamental");
        assert_eq!(shared_period(Some(4096), 48, 144, 480), 480, "above the maximum: the maximum");
        assert_eq!(shared_period(Some(64), 0, 32, 480), 64, "a zero fundamental is treated as 1");
        assert_eq!(mode_for_host("wasapi exclusive"), Some(Mode::Exclusive));
        assert_eq!(mode_for_host(LOW_LATENCY_HOST), Some(Mode::LowLatencyShared));
        assert_eq!(mode_for_host("WASAPI"), None);
    }

    #[test]
    fn sample_conversion_is_exact_and_clamped() {
        let src = [0.0f32, 1.0, -1.0, 0.5, 2.0];
        let mut o16 = vec![0u8; 10];
        convert_into(&src, SampleKind::I16, &mut o16);
        let v: Vec<i16> = o16.chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        assert_eq!(v, [0, 32_767, -32_767, 16_384, 32_767]);
        let mut o24 = vec![0u8; 15];
        convert_into(&src, SampleKind::I24, &mut o24);
        let v: Vec<i32> = o24.chunks(3).map(|c| i32::from_le_bytes([c[0], c[1], c[2], if c[2] & 0x80 != 0 { 0xff } else { 0 }])).collect();
        assert_eq!(v, [0, 8_388_607, -8_388_607, 4_194_304, 8_388_607]);
        let mut o32 = vec![0u8; 20];
        convert_into(&src, SampleKind::I32, &mut o32);
        let v: Vec<i32> = o32.chunks(4).map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        assert_eq!(v[1], i32::MAX);
        let mut of = vec![0u8; 20];
        convert_into(&src, SampleKind::F32, &mut of);
        assert_eq!(f32::from_le_bytes([of[12], of[13], of[14], of[15]]), 0.5);
        assert_eq!(f32::from_le_bytes([of[16], of[17], of[18], of[19]]), 1.0, "clamped");
    }
}
