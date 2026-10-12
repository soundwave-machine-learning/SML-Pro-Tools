#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
//! SoundCraft speech (Remote Continuation RC05): real, small, local speech for the assistant's
//! push-to-talk path, written for this project (no model weights, no downloads, no GPL code).
//!
//! - [`synth`]: English text → phonemes (a short hand-written lexicon, then letter-to-sound
//!   rules) → a cascade formant synthesizer (glottal pulse source, three variable formant
//!   resonators plus two fixed ones, a parallel frication branch). Intelligible-but-robotic by
//!   design; its quality has not been judged by listeners (L pending).
//! - [`recog`]: an enrolled-command recognizer. The user records each command phrase once (or
//!   more); an utterance is endpointed by energy, turned into mean-normalised mel-cepstra and
//!   matched by dynamic time warping. It recognises the enrolled vocabulary only (not
//!   dictation) and rejects what is not close enough to one phrase.
//!
//! Both run off the audio thread; they allocate freely and never panic on any input.

pub mod recog;
pub mod synth;

/// Sample rate of synthesized speech and of recognizer features.
pub const SPEECH_RATE: u32 = 16_000;

/// Linear-interpolation resampling of mono `x` from `from` Hz to `to` Hz (speech-band use only;
/// the device-rate conversion for playback happens in the host with its own resampler).
pub fn resample_linear(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || x.is_empty() || from == 0 || to == 0 {
        return x.to_vec();
    }
    let n = ((x.len() as u64 * u64::from(to)) / u64::from(from)) as usize;
    let step = f64::from(from) / f64::from(to);
    (0..n)
        .map(|i| {
            let p = i as f64 * step;
            let k = p.floor() as usize;
            let f = (p - k as f64) as f32;
            let a = x.get(k).copied().unwrap_or(0.0);
            let b = x.get(k + 1).copied().unwrap_or(a);
            a + (b - a) * f
        })
        .collect()
}
