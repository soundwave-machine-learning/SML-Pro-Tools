//! X05 (Remote Continuation Sprint 4): writes the frozen meter cases
//! (`docs/remote_continuation/evidence/sprint_04/meter-x05-frozen.json`) as 32-bit float WAV
//! files and SoundCraft's own measurements of them (`ours.csv`), for
//! `docs/remote_continuation/tools/compare_meters.py` to compare with FFmpeg's ebur128.
//! Signals are generated here from the published case descriptions (no test files are copied).
//!
//! `cargo run --release -p soundcraft-engine --example meter_vectors -- <out dir>`

use soundcraft_audio_io::{AudioBuffer, BitDepth, EncodeOptions, FileFormat};
use soundcraft_dsp::meter::LoudnessMeter;
use std::f64::consts::PI;
use std::fmt::Write as _;

fn db(x: f64) -> f64 {
    10f64.powf(x / 20.0)
}

/// A sine of `secs` at `level_db` (peak dBFS), frequency `hz`, starting phase `phase`.
fn sine(sr: u32, secs: f64, hz: f64, level_db: f64, phase: f64) -> Vec<f32> {
    let n = (secs * f64::from(sr)).round() as usize;
    (0..n).map(|i| (db(level_db) * (2.0 * PI * hz * i as f64 / f64::from(sr) + phase).sin()) as f32).collect()
}

/// 1 kHz segments of (level dBFS, seconds) back to back.
fn steps(sr: u32, segs: &[(f64, f64)]) -> Vec<f32> {
    let mut out = Vec::new();
    for &(l, s) in segs {
        // Continuous phase across the steps (no clicks at the joins).
        let start = out.len();
        let n = (s * f64::from(sr)).round() as usize;
        out.extend((0..n).map(|i| (db(l) * (2.0 * PI * 1000.0 * (start + i) as f64 / f64::from(sr)).sin()) as f32));
    }
    out
}

/// Log sine sweep from 20 Hz to 20 kHz.
fn sweep(sr: u32, secs: f64, level_db: f64) -> Vec<f32> {
    let n = (secs * f64::from(sr)).round() as usize;
    let (f1, f2) = (20.0f64, 20_000.0f64);
    let k = (f2 / f1).ln();
    (0..n)
        .map(|i| {
            let t = i as f64 / f64::from(sr);
            let phase = 2.0 * PI * f1 * secs / k * ((t / secs * k).exp() - 1.0);
            (db(level_db) * phase.sin()) as f32
        })
        .collect()
}

/// Low-passed noise bursts separated by silence (fixed seed).
fn bursts(sr: u32, secs: f64) -> Vec<f32> {
    let total = (secs * f64::from(sr)) as usize;
    let mut out = vec![0.0f32; total];
    let mut seed = 0x2545_F491_u64;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut at = 0usize;
    let mut y = 0.0f64;
    let a = (-2.0 * PI * 4000.0 / f64::from(sr)).exp();
    while at < total {
        let on = ((0.5 + 1.5 * rnd()) * f64::from(sr)) as usize;
        let gap = ((0.5 + 1.5 * rnd()) * f64::from(sr)) as usize;
        let level = db(-12.0 - 18.0 * rnd()) * 3f64.sqrt();
        for o in out.iter_mut().skip(at).take(on) {
            y = a * y + (1.0 - a) * (rnd() * 2.0 - 1.0);
            *o = (y * level * 2.5) as f32;
        }
        at += on + gap;
    }
    out
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "x05".into());
    let _ = std::fs::create_dir_all(&dir);
    let st = |x: Vec<f32>| vec![x.clone(), x];
    let k48 = 48_000u32;
    let cases: Vec<(&str, u32, Vec<Vec<f32>>)> = vec![
        ("X01", k48, st(sine(k48, 20.0, 1000.0, -23.0, 0.0))),
        ("X02", k48, st(sine(k48, 20.0, 1000.0, -33.0, 0.0))),
        ("X03", k48, st(steps(k48, &[(-36.0, 10.0), (-23.0, 60.0), (-36.0, 10.0)]))),
        ("X04", k48, st(steps(k48, &[(-72.0, 10.0), (-23.0, 20.0), (-72.0, 10.0)]))),
        ("X05", k48, st(steps(k48, &[(-26.0, 20.0), (-20.0, 20.0), (-26.0, 20.0)]))),
        ("X06", k48, vec![sine(k48, 20.0, 1000.0, -30.0, 0.0); 6]),
        ("X07", k48, st(sine(k48, 20.0, 997.0, -6.0, 0.0))),
        ("X08", k48, st((0..k48 as usize * 20).map(|i| (0.99 * (PI / 2.0 * i as f64 + PI / 4.0).sin()) as f32).collect())),
        ("X09", k48, st(steps(k48, &[(-20.0, 20.0), (-30.0, 20.0)]))),
        ("X10", k48, st(steps(k48, &[(-40.0, 20.0), (-20.0, 20.0)]))),
        ("X11", k48, st(bursts(k48, 60.0))),
        ("X12", k48, st(sweep(k48, 30.0, -12.0))),
        ("X13", 44_100, st(sine(44_100, 20.0, 1000.0, -23.0, 0.0))),
        ("X14", 44_100, st(sweep(44_100, 30.0, -12.0))),
    ];
    let mut csv = String::from("case,integrated_lufs,max_momentary_lufs,loudness_range_lu,true_peak_db\n");
    for (id, sr, ch) in cases {
        let mut m = LoudnessMeter::new(sr as f32, ch.len());
        let frames = ch.first().map_or(0, Vec::len);
        let mut done = 0;
        while done < frames {
            let n = (frames - done).min(1024);
            let block: Vec<Vec<f32>> = ch.iter().map(|c| c[done..done + n].to_vec()).collect();
            m.process(&block, n);
            done += n;
        }
        let _ = writeln!(csv, "{id},{:.4},{:.4},{:.4},{:.4}", m.integrated_lufs(), m.max_momentary_lufs(), m.loudness_range_lu(), m.true_peak_db());
        let bytes = soundcraft_audio_io::encode(
            &AudioBuffer { sample_rate: sr, channels: ch },
            &EncodeOptions { format: FileFormat::Wav, bit_depth: BitDepth::Float32, dither: false, bwf: None },
        );
        match bytes {
            Ok(b) => {
                if let Err(e) = std::fs::write(format!("{dir}/{id}.wav"), b) {
                    eprintln!("{id}: {e}");
                }
            }
            Err(e) => eprintln!("{id}: {e}"),
        }
    }
    if let Err(e) = std::fs::write(format!("{dir}/ours.csv"), &csv) {
        eprintln!("ours.csv: {e}");
    }
    print!("{csv}");
}
