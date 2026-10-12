//! RC06 speech part of the frozen held-out evaluation (`heldout-eval.json`, cases S01–S08, T01),
//! scored as written. Utterances are synthetic (the built-in synthesizer with other voice
//! settings and added noise): weaker evidence than human speech; microphones are H/M pending.
//! Rejection (S07, S08) and synthesis sanity (T01) are asserted; accuracy under other voices
//! and noise (S02–S06) is recorded. Writes `SML_SPEECH_CSV` when set.

use soundcraft_speech::recog::Recognizer;
use soundcraft_speech::synth::{Voice, synthesize};
use std::io::Write;
use std::path::Path;

const SR: u32 = 16_000;

fn ptt(text: &str, v: Voice) -> Vec<f32> {
    let mut x = vec![0.0f32; 3_200];
    x.extend(synthesize(text, v));
    x.extend(vec![0.0f32; 3_200]);
    x
}

/// Adds white noise at `snr_db` relative to the speech's RMS (over its non-silent samples).
fn noisy(mut x: Vec<f32>, snr_db: f32, seed: u32) -> Vec<f32> {
    let speech: Vec<f32> = x.iter().copied().filter(|v| v.abs() > 1e-6).collect();
    let rms = (speech.iter().map(|v| v * v).sum::<f32>() / speech.len().max(1) as f32).sqrt();
    let target = rms / 10f32.powf(snr_db / 20.0);
    let mut s = seed.max(1);
    // Uniform noise of RMS `target`: amplitude target·√3.
    for v in &mut x {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        *v += (s as f32 / u32::MAX as f32 * 2.0 - 1.0) * target * 3f32.sqrt();
    }
    x
}

#[test]
fn heldout_speech_evaluation() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/remote_continuation/evidence/sprint_02/heldout-eval.json");
    let ev: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    let vocab: Vec<String> = ev["speech"]["vocabulary"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    let voice_a = Voice { pitch_hz: 110.0, rate: 1.0, formant_scale: 1.0 };
    let mut r = Recognizer::default();
    for w in &vocab {
        r.enroll(w, &ptt(w, voice_a), SR).unwrap();
    }
    let thresholds: Vec<f32> = (0..vocab.len()).map(|i| r.threshold(i)).collect();
    println!("thresholds (enrollment-derived): {thresholds:?}");
    let mut rows: Vec<(String, String, String, String, bool)> = Vec::new();
    let conditions: [(&str, Voice, Option<f32>); 6] = [
        ("S01", voice_a, None),
        ("S02", Voice { pitch_hz: 180.0, ..voice_a }, None),
        ("S03", Voice { rate: 0.8, ..voice_a }, None),
        ("S04", Voice { rate: 1.25, ..voice_a }, None),
        ("S05", voice_a, Some(20.0)),
        ("S06", voice_a, Some(10.0)),
    ];
    for (id, v, snr) in conditions {
        for (k, w) in vocab.iter().enumerate() {
            let mut x = ptt(w, v);
            if let Some(snr) = snr {
                x = noisy(x, snr, 7 + k as u32);
            }
            let rec = r.recognize(&x, SR);
            let got = rec.phrase.clone().unwrap_or_else(|| format!("(rejected: {})", rec.reason));
            let best = rec.ranking.first().map(|(p, d)| format!("{p} {d:.2}")).unwrap_or_default();
            rows.push((id.into(), w.clone(), got.clone(), format!("best {best}; threshold {:.2}", rec.threshold), got == *w));
        }
    }
    for w in ["banana", "delete everything", "hello there"] {
        let rec = r.recognize(&ptt(w, voice_a), SR);
        let got = rec.phrase.clone().unwrap_or_else(|| format!("(rejected: {})", rec.reason));
        let best = rec.ranking.first().map(|(p, d)| format!("{p} {d:.2}")).unwrap_or_default();
        rows.push(("S07".into(), w.into(), got, format!("best {best}; threshold {:.2}", rec.threshold), rec.phrase.is_none()));
    }
    let silence = vec![0.0f32; 16_000];
    let noise = noisy(vec![0.0f32; 16_000], 0.0, 99);
    let noise = {
        // `noisy` scales to the speech RMS; for pure noise use a fixed level instead.
        let mut s = 5u32;
        noise
            .iter()
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                (s as f32 / u32::MAX as f32 * 2.0 - 1.0) * 0.2
            })
            .collect::<Vec<f32>>()
    };
    for (what, x) in [("silence", silence), ("noise", noise)] {
        let rec = r.recognize(&x, SR);
        rows.push((
            "S08".into(),
            what.into(),
            rec.phrase.clone().unwrap_or_else(|| format!("(rejected: {})", rec.reason)),
            String::new(),
            rec.phrase.is_none(),
        ));
    }
    for w in &vocab {
        let y = synthesize(w, voice_a);
        let secs = y.len() as f32 / SR as f32;
        let peak = y.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let rms = (y.iter().map(|v| v * v).sum::<f32>() / y.len().max(1) as f32).sqrt();
        let ok = (0.2..=3.0).contains(&secs) && y.iter().all(|v| v.is_finite()) && peak <= 0.9 && rms > 0.01;
        rows.push(("T01".into(), w.clone(), format!("{secs:.2} s, peak {peak:.2}, rms {rms:.3}"), String::new(), ok));
    }
    for (id, input, got, detail, ok) in &rows {
        println!("{id} {input:>18} -> {got} [{}] {detail}", if *ok { "PASS" } else { "FAIL" });
    }
    let mut summary = Vec::new();
    for id in ["S01", "S02", "S03", "S04", "S05", "S06", "S07", "S08", "T01"] {
        let of: Vec<_> = rows.iter().filter(|r| r.0 == id).collect();
        summary.push(format!("{id} {}/{}", of.iter().filter(|r| r.4).count(), of.len()));
    }
    println!("summary: {}", summary.join(", "));
    if let Some(path) = std::env::var_os("SML_SPEECH_CSV") {
        let mut f = std::fs::File::create(path).unwrap();
        writeln!(f, "case,input,output,result,detail").unwrap();
        for (id, input, got, detail, ok) in &rows {
            writeln!(f, "{id},{input},{},{},{}", got.replace(',', ";"), if *ok { "PASS" } else { "FAIL" }, detail.replace(',', ";")).unwrap();
        }
        writeln!(f, "summary,,{},,", summary.join("; ")).unwrap();
    }
    let gated: Vec<_> = rows
        .iter()
        .filter(|r| matches!(r.0.as_str(), "S01" | "S07" | "S08" | "T01") && !r.4)
        .map(|r| format!("{} {} -> {}", r.0, r.1, r.2))
        .collect();
    assert!(gated.is_empty(), "asserted speech cases failed:\n{}", gated.join("\n"));
}
