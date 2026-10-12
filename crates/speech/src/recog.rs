//! Enrolled-command speech recognition (original implementation of textbook techniques).
//!
//! Utterance → 16 kHz → energy endpointing → 25 ms frames every 10 ms → pre-emphasis, Hamming
//! window, power spectrum → 26 mel bands → log → DCT → cepstra c1…c12 (c0, the level, is
//! dropped), liftered and mean-normalised over the utterance (removes level and channel
//! colour). An utterance is compared with every enrolled template by dynamic time warping
//! (Sakoe–Chiba band), the distance normalised by path length.
//!
//! Rejection is decided from the enrollment alone, never tuned on test speech: phrase *i*
//! is accepted when its best distance is below half of (a) its distance to the nearest *other*
//! enrolled phrase and (b) its distance to its own time-reversed template (an utterance with
//! the same sounds in a meaningless order), and clearly below the runner-up phrase (≤ 0.9×).

use crate::SPEECH_RATE;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;

const FRAME: usize = 400;
const HOP: usize = 160;
const NFFT: usize = 512;
const MELS: usize = 26;
const CEPS: usize = 12;
/// Shortest utterance worth matching (frames of 10 ms).
const MIN_FRAMES: usize = 15;
/// Band-energy floor under the utterance's strongest band (dB).
const FLOOR_DB: f32 = -30.0;
/// Longest utterance kept (frames): 8 s.
const MAX_FRAMES: usize = 800;

/// Cepstral frames of one utterance.
pub type Features = Vec<[f32; CEPS]>;

fn hz_to_mel(f: f32) -> f32 {
    2595.0 * (1.0 + f / 700.0).log10()
}
fn mel_to_hz(m: f32) -> f32 {
    700.0 * (10f32.powf(m / 2595.0) - 1.0)
}

/// Triangular mel filters over the FFT bins: (first bin, weights).
fn mel_bank() -> Vec<(usize, Vec<f32>)> {
    let (lo, hi) = (hz_to_mel(100.0), hz_to_mel(7000.0));
    let pts: Vec<f32> = (0..MELS + 2).map(|i| mel_to_hz(lo + (hi - lo) * i as f32 / (MELS + 1) as f32)).collect();
    let bin = |f: f32| f * NFFT as f32 / SPEECH_RATE as f32;
    (0..MELS)
        .map(|m| {
            let (a, b, c) = (bin(pts[m]), bin(pts[m + 1]), bin(pts[m + 2]));
            let first = a.floor().max(0.0) as usize;
            let last = (c.ceil() as usize).min(NFFT / 2);
            let w = (first..=last)
                .map(|k| {
                    let k = k as f32;
                    if k < a || k > c {
                        0.0
                    } else if k <= b {
                        (k - a) / (b - a).max(1e-6)
                    } else {
                        (c - k) / (c - b).max(1e-6)
                    }
                })
                .collect();
            (first, w)
        })
        .collect()
}

/// Frame energies in dB (20 ms frames, 10 ms hop) and the kept range (start, end samples) of the
/// speech in `x`, or `None` when there is no clear speech (silence or steady noise).
pub fn endpoint(x: &[f32]) -> Option<(usize, usize)> {
    let win = 320;
    if x.len() < win {
        return None;
    }
    let db: Vec<f32> = (0..=(x.len() - win) / HOP)
        .map(|i| {
            let s = x.get(i * HOP..i * HOP + win).unwrap_or(&[]);
            let e = s.iter().map(|v| v * v).sum::<f32>() / win as f32;
            10.0 * (e + 1e-12).log10()
        })
        .collect();
    // Noise floor: the quietest 30 ms stretch (speech has pauses and closures; steady noise has
    // no stretch much quieter than the rest).
    let smooth: Vec<f32> = db.windows(3).map(|w| w.iter().sum::<f32>() / 3.0).collect();
    let floor = smooth.iter().copied().fold(f32::INFINITY, f32::min).min(db.iter().copied().fold(f32::INFINITY, f32::max));
    let peak = db.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if !floor.is_finite() || !peak.is_finite() {
        return None;
    }
    // Speech must stand at least 12 dB above the noise floor somewhere, and be audible at all.
    if peak - floor < 12.0 || peak < -60.0 {
        return None;
    }
    let thr = floor + (peak - floor) * 0.3;
    let first = db.iter().position(|d| *d > thr)?;
    let last = db.iter().rposition(|d| *d > thr)?;
    let pad = 5; // 50 ms
    let start = first.saturating_sub(pad) * HOP;
    let end = ((last + pad) * HOP + win).min(x.len());
    (end > start).then_some((start, end))
}

/// Mean-normalised mel-cepstra of the speech in `pcm` (any rate; resampled to 16 kHz).
/// `None`: no speech found or too short.
pub fn features(pcm: &[f32], rate: u32) -> Option<Features> {
    let x: Vec<f32> =
        crate::resample_linear(pcm, rate, SPEECH_RATE).into_iter().map(|v| if v.is_finite() { v.clamp(-4.0, 4.0) } else { 0.0 }).collect();
    let (s, e) = endpoint(&x)?;
    let x = x.get(s..e)?;
    let bank = mel_bank();
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(NFFT);
    let hamming: Vec<f32> = (0..FRAME).map(|i| 0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / (FRAME - 1) as f32).cos()).collect();
    let lifter: Vec<f32> = (1..=CEPS).map(|n| 1.0 + 11.0 * (std::f32::consts::PI * n as f32 / 22.0).sin()).collect();
    let mut out: Features = Vec::new();
    let mut mel: Vec<[f32; MELS]> = Vec::new();
    let mut buf = vec![Complex::new(0.0f32, 0.0); NFFT];
    let mut start = 0;
    while start + FRAME <= x.len() && mel.len() < MAX_FRAMES {
        for (k, b) in buf.iter_mut().enumerate() {
            *b = if k < FRAME {
                let cur = x.get(start + k).copied().unwrap_or(0.0);
                let prev = if start + k > 0 { x.get(start + k - 1).copied().unwrap_or(0.0) } else { 0.0 };
                Complex::new((cur - 0.97 * prev) * hamming.get(k).copied().unwrap_or(0.0), 0.0)
            } else {
                Complex::new(0.0, 0.0)
            };
        }
        fft.process(&mut buf);
        let mut e = [0.0f32; MELS];
        for (em, (first, w)) in e.iter_mut().zip(&bank) {
            *em = w.iter().enumerate().map(|(i, wt)| buf.get(first + i).map_or(0.0, |c| c.norm_sqr()) * wt).sum();
        }
        mel.push(e);
        start += HOP;
    }
    if mel.len() < MIN_FRAMES {
        return None;
    }
    // Noise masking: every band is floored FLOOR_DB under the utterance's strongest band, so
    // digital silence and a real room's noise floor look alike.
    let strongest = mel.iter().flat_map(|f| f.iter().copied()).fold(0.0f32, f32::max);
    let floor = (strongest * 10f32.powf(FLOOR_DB / 10.0)).max(1e-12);
    for e in &mel {
        let mut c = [0.0f32; CEPS];
        for (n, cn) in c.iter_mut().enumerate() {
            let q = (n + 1) as f32;
            *cn =
                e.iter().enumerate().map(|(m, v)| (v + floor).ln() * (std::f32::consts::PI * q * (m as f32 + 0.5) / MELS as f32).cos()).sum::<f32>()
                    * lifter.get(n).copied().unwrap_or(1.0);
        }
        out.push(c);
    }
    let mut mean = [0.0f32; CEPS];
    for f in &out {
        for (m, v) in mean.iter_mut().zip(f) {
            *m += v / out.len() as f32;
        }
    }
    for f in &mut out {
        for (v, m) in f.iter_mut().zip(mean) {
            *v -= m;
        }
    }
    Some(out)
}

fn dist(a: &[f32; CEPS], b: &[f32; CEPS]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f32>().sqrt()
}

/// Dynamic-time-warping distance of two utterances, normalised by path length; `f32::INFINITY`
/// when their lengths differ by more than the band allows (over 2.5×).
pub fn dtw(a: &Features, b: &Features) -> f32 {
    let (n, m) = (a.len(), b.len());
    if n == 0 || m == 0 || n > m * 5 / 2 || m > n * 5 / 2 {
        return f32::INFINITY;
    }
    let band = n.max(m) / 3 + 4;
    let inf = f32::INFINITY;
    let mut prev = vec![(inf, 0u32); m + 1];
    let mut cur = vec![(inf, 0u32); m + 1];
    prev[0] = (0.0, 0);
    for i in 1..=n {
        cur.iter_mut().for_each(|c| *c = (inf, 0));
        let centre = i * m / n;
        let lo = centre.saturating_sub(band).max(1);
        let hi = (centre + band).min(m);
        for j in lo..=hi {
            let d = dist(&a[i - 1], &b[j - 1]);
            // Diagonal steps count twice (symmetric form), so path weight = n + m.
            let cands = [(prev[j - 1].0 + 2.0 * d, prev[j - 1].1 + 2), (prev[j].0 + d, prev[j].1 + 1), (cur[j - 1].0 + d, cur[j - 1].1 + 1)];
            let best = cands.iter().copied().fold((inf, 0), |acc, c| if c.0 < acc.0 { c } else { acc });
            cur[j] = best;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    let (cost, w) = prev[m];
    if cost.is_finite() && w > 0 { cost / w as f32 } else { inf }
}

/// One enrolled phrase: its templates and the rejection threshold derived from enrollment.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Phrase {
    pub text: String,
    pub templates: Vec<Features>,
}

/// The recogniser's answer for one utterance.
#[derive(Debug, Clone, PartialEq)]
pub struct Recognition {
    /// The recognised phrase, or `None` (rejected).
    pub phrase: Option<String>,
    /// Phrases by distance (best first).
    pub ranking: Vec<(String, f32)>,
    /// The acceptance threshold of the best phrase.
    pub threshold: f32,
    /// Why it was rejected (empty when accepted).
    pub reason: String,
}

/// Enrolled command vocabulary.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Recognizer {
    pub phrases: Vec<Phrase>,
}

/// Most phrases and templates kept (bounded memory and matching time).
pub const MAX_PHRASES: usize = 64;
pub const MAX_TEMPLATES: usize = 4;

impl Recognizer {
    /// Adds one recording of `text` (trimmed, case kept). Errors on no speech, too short, or
    /// the vocabulary/template limits.
    pub fn enroll(&mut self, text: &str, pcm: &[f32], rate: u32) -> Result<(), String> {
        let text = text.trim();
        if text.is_empty() || text.len() > 80 {
            return Err("a phrase needs 1-80 characters".into());
        }
        let f = features(pcm, rate).ok_or("no speech found in the recording (too quiet, too short or only noise)")?;
        let full = self.phrases.len() >= MAX_PHRASES;
        match self.phrases.iter_mut().find(|p| p.text.eq_ignore_ascii_case(text)) {
            Some(p) if p.templates.len() >= MAX_TEMPLATES => Err(format!("`{text}` already has {MAX_TEMPLATES} recordings")),
            Some(p) => {
                p.templates.push(f);
                Ok(())
            }
            None if full => Err(format!("at most {MAX_PHRASES} phrases")),
            None => {
                self.phrases.push(Phrase { text: text.to_string(), templates: vec![f] });
                Ok(())
            }
        }
    }

    pub fn remove(&mut self, text: &str) -> bool {
        let n = self.phrases.len();
        self.phrases.retain(|p| !p.text.eq_ignore_ascii_case(text));
        self.phrases.len() != n
    }

    fn phrase_distance(p: &Phrase, f: &Features) -> f32 {
        p.templates.iter().map(|t| dtw(t, f)).fold(f32::INFINITY, f32::min)
    }

    /// Acceptance threshold for phrase `i` (enrollment-derived, see the module docs).
    pub fn threshold(&self, i: usize) -> f32 {
        let Some(p) = self.phrases.get(i) else { return 0.0 };
        let reversed = p
            .templates
            .iter()
            .map(|t| t.iter().rev().copied().collect::<Features>())
            .map(|r| Self::phrase_distance(p, &r))
            .fold(f32::INFINITY, f32::min);
        let others = self
            .phrases
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .flat_map(|(_, q)| p.templates.iter().map(move |t| Self::phrase_distance(q, t)))
            .fold(f32::INFINITY, f32::min);
        0.5 * reversed.min(others)
    }

    pub fn recognize(&self, pcm: &[f32], rate: u32) -> Recognition {
        let reject =
            |reason: &str, ranking: Vec<(String, f32)>, threshold: f32| Recognition { phrase: None, ranking, threshold, reason: reason.into() };
        if self.phrases.is_empty() {
            return reject("no phrases are enrolled", Vec::new(), 0.0);
        }
        let Some(f) = features(pcm, rate) else { return reject("no speech found", Vec::new(), 0.0) };
        let mut ranking: Vec<(usize, f32)> = self.phrases.iter().enumerate().map(|(i, p)| (i, Self::phrase_distance(p, &f))).collect();
        ranking.sort_by(|a, b| a.1.total_cmp(&b.1));
        let named: Vec<(String, f32)> = ranking.iter().filter_map(|(i, d)| self.phrases.get(*i).map(|p| (p.text.clone(), *d))).collect();
        let Some(&(best, d1)) = ranking.first() else { return reject("no phrases", named, 0.0) };
        let t = self.threshold(best);
        if !d1.is_finite() || d1 > t {
            return reject("not close enough to any enrolled phrase", named, t);
        }
        if let Some(&(_, d2)) = ranking.get(1)
            && d1 > 0.9 * d2
        {
            return reject("too close to call between two phrases", named, t);
        }
        Recognition { phrase: self.phrases.get(best).map(|p| p.text.clone()), ranking: named, threshold: t, reason: String::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::{Voice, synthesize};

    /// A push-to-talk style recording: 200 ms of silence around the words.
    fn ptt(text: &str, v: Voice) -> Vec<f32> {
        let mut x = vec![0.0f32; 3_200];
        x.extend(synthesize(text, v));
        x.extend(vec![0.0f32; 3_200]);
        x
    }

    #[test]
    fn silence_noise_and_garbage_are_not_speech() {
        assert!(features(&[], 16_000).is_none());
        assert!(features(&vec![0.0; 16_000], 16_000).is_none());
        let mut seed = 1u32;
        let noise: Vec<f32> = (0..16_000)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5
            })
            .collect();
        assert!(features(&noise, 16_000).is_none(), "steady noise has no speech");
        assert!(features(&[f32::NAN; 20_000], 48_000).is_none());
        let r = Recognizer::default().recognize(&noise, 16_000);
        assert!(r.phrase.is_none());
    }

    #[test]
    fn dtw_is_zero_on_itself_and_tolerates_tempo() {
        let a = features(&ptt("next marker", Voice::default()), 16_000).unwrap();
        assert!(dtw(&a, &a) < 1e-6);
        let slow = features(&ptt("next marker", Voice { rate: 0.8, ..Voice::default() }), 16_000).unwrap();
        let other = features(&ptt("undo", Voice::default()), 16_000).unwrap();
        assert!(dtw(&a, &slow) < dtw(&a, &other), "same words at another tempo are closer than other words");
    }

    #[test]
    fn enrollment_limits_and_removal() {
        let mut r = Recognizer::default();
        let pcm = ptt("play", Voice::default());
        assert!(r.enroll("", &pcm, 16_000).is_err());
        assert!(r.enroll("play", &[0.0; 100], 16_000).is_err());
        for _ in 0..MAX_TEMPLATES {
            r.enroll("play", &pcm, 16_000).unwrap();
        }
        assert!(r.enroll("PLAY", &pcm, 16_000).is_err());
        assert!(r.remove("Play") && r.phrases.is_empty());
    }
}
