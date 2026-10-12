//! Development probe (NOT the held-out set): recognition of a separate dev vocabulary under
//! other voices and white noise, used to try noise handling after the held-out first run showed
//! 0/8 at 20 and 10 dB SNR. Results are recorded in SPRINT_02_REPORT; no variant tried
//! (relative spectral floors at -35/-45/-55/-60 dB with spectral subtraction, a fixed-margin
//! endpoint) improved noise without breaking tempo tolerance, so the recognizer was left as it
//! was first scored. `cargo run -p soundcraft-speech --example dev_noise_probe`
use soundcraft_speech::recog::{dtw, endpoint, features};
use soundcraft_speech::synth::{Voice, synthesize};

fn ptt(text: &str, v: Voice) -> Vec<f32> {
    let mut x = vec![0.0f32; 3_200];
    x.extend(synthesize(text, v));
    x.extend(vec![0.0f32; 3_200]);
    x
}
fn noisy(mut x: Vec<f32>, snr_db: f32, seed: u32) -> Vec<f32> {
    let sp: Vec<f32> = x.iter().copied().filter(|v| v.abs() > 1e-6).collect();
    let rms = (sp.iter().map(|v| v * v).sum::<f32>() / sp.len().max(1) as f32).sqrt();
    let t = rms / 10f32.powf(snr_db / 20.0) * 3f32.sqrt();
    let mut s = seed.max(1);
    for v in &mut x {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        *v += (s as f32 / u32::MAX as f32 * 2.0 - 1.0) * t;
    }
    x
}
fn main() {
    use soundcraft_speech::recog::Recognizer;
    let devw = ["solo", "rewind", "bounce", "zoom in", "go to start", "loop", "metronome", "redo"];
    let mut r = Recognizer::default();
    for w in devw {
        r.enroll(w, &ptt(w, Voice::default()), 16_000).unwrap();
    }
    let conds: [(&str, Voice, Option<f32>); 9] = [
        ("snr40", Voice::default(), Some(40.0)),
        ("snr35", Voice::default(), Some(35.0)),
        ("snr30", Voice::default(), Some(30.0)),
        ("clean", Voice::default(), None),
        ("pitch150", Voice { pitch_hz: 150.0, ..Voice::default() }, None),
        ("rate0.9", Voice { rate: 0.9, ..Voice::default() }, None),
        ("snr25", Voice::default(), Some(25.0)),
        ("snr15", Voice::default(), Some(15.0)),
        ("snr8", Voice::default(), Some(8.0)),
    ];
    for (name, v, snr) in conds {
        let mut ok = 0;
        let mut misses = Vec::new();
        for (k, w) in devw.iter().enumerate() {
            let mut x = ptt(w, v);
            if let Some(s) = snr {
                x = noisy(x, s, 5000 + k as u32);
            }
            let rec = r.recognize(&x, 16_000);
            if rec.phrase.as_deref() == Some(*w) {
                ok += 1;
            } else {
                misses.push(format!("{w}->{:?}({}; best {:?})", rec.phrase, rec.reason, rec.ranking.first()));
            }
        }
        println!("DEV {name}: {ok}/8 {}", misses.join(" | "));
    }
    // Matched conditions: enroll in the same kind of noise (another seed) as the test.
    for snr in [20.0f32, 10.0] {
        let mut m = Recognizer::default();
        for (k, w) in devw.iter().enumerate() {
            m.enroll(w, &noisy(ptt(w, Voice::default()), snr, 9000 + k as u32), 16_000)
                .unwrap_or_else(|e| println!("DEV enroll {w} at {snr} dB: {e}"));
        }
        let ok = devw
            .iter()
            .enumerate()
            .filter(|(k, w)| m.recognize(&noisy(ptt(w, Voice::default()), snr, 7000 + *k as u32), 16_000).phrase.as_deref() == Some(**w))
            .count();
        let oov = ["apple", "window", "seven", "turn it up"]
            .iter()
            .filter(|w| m.recognize(&noisy(ptt(w, Voice::default()), snr, 11), 16_000).phrase.is_some())
            .count();
        println!("DEV matched-noise enroll+test at {snr} dB: {ok}/8 recognised, {oov}/4 OOV wrongly accepted");
    }
    for w in ["apple", "window", "seven", "turn it up"] {
        let rec = r.recognize(&ptt(w, Voice::default()), 16_000);
        println!("DEV oov {w}: {:?} best {:?} thr {:.1}", rec.phrase, rec.ranking.first(), rec.threshold);
    }
    {
        let x = noisy(ptt("rewind", Voice::default()), 30.0, 5001);
        let c = ptt("rewind", Voice::default());
        let e = |x: &[f32]| -> Vec<i32> {
            x.chunks(320).map(|s| (10.0 * (s.iter().map(|v| v * v).sum::<f32>() / 320.0 + 1e-12).log10()) as i32).collect()
        };
        println!("DBG clean dB {:?}", e(&c));
        println!("DBG noisy dB {:?}", e(&x));
        println!(
            "DBG endpoints {:?} {:?} features {:?} {:?}",
            endpoint(&c),
            endpoint(&x),
            features(&c, 16_000).map(|f| f.len()),
            features(&x, 16_000).map(|f| f.len())
        );
    }
    let dev = ["solo", "rewind", "bounce", "zoom in", "go to start", "loop", "metronome", "redo"];
    for w in dev {
        let clean = ptt(w, Voice::default());
        let n = noisy(clean.clone(), 20.0, 1234);
        println!("{w}: clean endpoint {:?} noisy {:?} len {}", endpoint(&clean), endpoint(&n), clean.len());
        if let (Some(a), Some(b)) = (features(&clean, 16_000), features(&n, 16_000)) {
            println!("   frames {} {} dtw clean-noisy {:.1}", a.len(), b.len(), dtw(&a, &b));
        }
    }
}
