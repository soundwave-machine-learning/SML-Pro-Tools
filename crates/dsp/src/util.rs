//! Small realtime building blocks shared by the plugins.

/// Default sample rate used when a host passes something unusable.
pub(crate) const FALLBACK_SR: f32 = 48_000.0;

/// Sanitizes a host sample rate into 1 kHz ..= 1.536 MHz.
pub(crate) fn sane_sr(sr: f32) -> f32 {
    if sr.is_finite() && sr >= 1000.0 { sr.min(1_536_000.0) } else { FALLBACK_SR }
}

/// Upper bound on channels any plugin will allocate state for.
pub(crate) const MAX_CHANNELS: usize = 64;

/// Converts milliseconds to (fractional) samples, never negative or non-finite.
#[inline]
pub(crate) fn ms_to_samples(ms: f32, sr: f32) -> f32 {
    let s = ms * 0.001 * sr;
    if s.is_finite() { s.max(0.0) } else { 0.0 }
}

/// One-pole coefficient for a time constant in milliseconds.
#[inline]
pub(crate) fn time_coef(ms: f32, sr: f32) -> f32 {
    let n = ms_to_samples(ms, sr);
    if n < 1.0e-3 { 0.0 } else { (-1.0 / n).exp() }
}

/// Flushes denormals and non-finite values in feedback paths to zero.
#[inline]
pub(crate) fn flush(x: f32) -> f32 {
    if x.is_finite() && x.abs() > 1.0e-25 { x } else { 0.0 }
}

/// Xorshift32 noise source (deterministic, allocation free).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Rng(u32);

impl Rng {
    pub(crate) fn new(seed: u32) -> Self {
        Self(if seed == 0 { 0x9E37_79B9 } else { seed })
    }

    #[inline]
    pub(crate) fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform white noise in -1..1.
    #[inline]
    pub(crate) fn bipolar(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (2.0 / 16_777_216.0) - 1.0
    }
}

/// Pink noise via a sum of first-order filtered white noise (Kellet-style economy filter).
#[derive(Debug, Clone, Copy)]
pub(crate) struct PinkNoise {
    b: [f32; 7],
}

impl PinkNoise {
    pub(crate) fn new() -> Self {
        Self { b: [0.0; 7] }
    }

    #[inline]
    pub(crate) fn next(&mut self, white: f32) -> f32 {
        let b = &mut self.b;
        b[0] = 0.99886 * b[0] + white * 0.0555179;
        b[1] = 0.99332 * b[1] + white * 0.0750759;
        b[2] = 0.96900 * b[2] + white * 0.153_852;
        b[3] = 0.86650 * b[3] + white * 0.3104856;
        b[4] = 0.55000 * b[4] + white * 0.5329522;
        b[5] = -0.7616 * b[5] - white * 0.016_898;
        let out = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + white * 0.5362;
        b[6] = white * 0.115926;
        out * 0.11
    }
}
