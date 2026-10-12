//! Development probe: level of sustained phonemes (dB RMS after the synthesizer's normalisation is
//! undone by synthesizing them together with a fixed reference).
use soundcraft_speech::synth::{Voice, synthesize_phonemes};
fn main() {
    for p in ["AA", "IY", "M", "S", "SH", "F", "T", "K", "HH", "Z"] {
        // Each phoneme between two long AA vowels: report its level relative to the vowels.
        let ph: Vec<String> = ["AA", "AA", "AA", p, p, p, "AA", "AA", "AA"].iter().map(|s| s.to_string()).collect();
        let x = synthesize_phonemes(&ph, Voice::default());
        let n = x.len();
        let rms = |a: &[f32]| 10.0 * (a.iter().map(|v| v * v).sum::<f32>() / a.len().max(1) as f32 + 1e-12).log10();
        println!(
            "{p}: vowel {:.1} dB, middle {:.1} dB, peak {:.2}",
            rms(&x[n / 12..n / 4]),
            rms(&x[n * 5 / 12..n * 7 / 12]),
            x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
        );
    }
}
