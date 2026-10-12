//! E07 (R34): loudness / true-peak / loudness-range conformance against analytically defined
//! signals of EBU Tech 3341 / 3342 and ITU-R BS.1770-4. Only signals whose expected value
//! follows from the standard's definition are generated here (no downloaded vectors); the
//! tolerances are the documents' own (±0.1 LU loudness, ±0.2 dB true peak, ±1 LU range).
//! Writes `SML_E07_CSV` when set. Passing these is conformance evidence for the listed cases,
//! not a certification.

use soundcraft_dsp::meter::LoudnessMeter;

fn sine(freq: f32, db: f32, secs: f32, sr: f32, ch: usize) -> Vec<Vec<f32>> {
    let a = 10f32.powf(db / 20.0);
    let n = (secs * sr) as usize;
    let one: Vec<f32> = (0..n).map(|i| a * (2.0 * std::f32::consts::PI * freq * i as f32 / sr).sin()).collect();
    vec![one; ch]
}

struct Case {
    id: &'static str,
    what: &'static str,
    expected: f32,
    tolerance: f32,
    measured: f32,
}

fn integrated(sig: &[Vec<f32>], sr: f32) -> LoudnessMeter {
    let mut m = LoudnessMeter::new(sr, sig.len());
    m.process(sig, sig[0].len());
    m
}

#[test]
fn ebu_3341_3342_and_bs1770_analytic_cases() {
    let sr = 48_000.0;
    let mut cases = Vec::new();
    // Tech 3341 case 1: stereo 1 kHz sine at −23.0 dBFS, 20 s → −23.0 LUFS ±0.1 (both channels).
    let m = integrated(&sine(1000.0, -23.0, 20.0, sr, 2), sr);
    cases.push(Case {
        id: "3341-1", what: "integrated, stereo 1 kHz −23 dBFS 20 s", expected: -23.0, tolerance: 0.1, measured: m.integrated_lufs()
    });
    cases.push(Case { id: "3341-1-M", what: "max momentary of the same", expected: -23.0, tolerance: 0.1, measured: m.max_momentary_lufs() });
    cases.push(Case { id: "3341-1-S", what: "short-term at the end of the same", expected: -23.0, tolerance: 0.1, measured: m.short_term_lufs() });
    // Tech 3341 case 2: −33.0 dBFS → −33.0 LUFS.
    let m = integrated(&sine(1000.0, -33.0, 20.0, sr, 2), sr);
    cases.push(Case {
        id: "3341-2", what: "integrated, stereo 1 kHz −33 dBFS 20 s", expected: -33.0, tolerance: 0.1, measured: m.integrated_lufs()
    });
    // Tech 3341 case 3: −36 dBFS 10 s, −23 dBFS 60 s, −36 dBFS 10 s → −23.0 (relative gate).
    let mut sig = sine(1000.0, -36.0, 10.0, sr, 2);
    let mid = sine(1000.0, -23.0, 60.0, sr, 2);
    let tail = sine(1000.0, -36.0, 10.0, sr, 2);
    for (c, s) in sig.iter_mut().enumerate() {
        s.extend_from_slice(&mid[c]);
        s.extend_from_slice(&tail[c]);
    }
    let m = integrated(&sig, sr);
    cases.push(Case {
        id: "3341-3",
        what: "integrated with relative gating (−36/−23/−36 dBFS)",
        expected: -23.0,
        tolerance: 0.1,
        measured: m.integrated_lufs(),
    });
    // Tech 3341 case 4: −72 dBFS 10 s, −23 dBFS 60 s, −72 dBFS 10 s → −23.0 (absolute gate).
    let mut sig = sine(1000.0, -72.0, 10.0, sr, 2);
    let mid = sine(1000.0, -23.0, 60.0, sr, 2);
    let tail = sine(1000.0, -72.0, 10.0, sr, 2);
    for (c, s) in sig.iter_mut().enumerate() {
        s.extend_from_slice(&mid[c]);
        s.extend_from_slice(&tail[c]);
    }
    let m = integrated(&sig, sr);
    cases.push(Case {
        id: "3341-4",
        what: "integrated with absolute gating (−72/−23/−72 dBFS)",
        expected: -23.0,
        tolerance: 0.1,
        measured: m.integrated_lufs(),
    });
    // Tech 3341 case 5: −26 dBFS 20 s, −20 dBFS 20 s, −26 dBFS 20 s → −23.0 ±0.1.
    let mut sig = sine(1000.0, -26.0, 20.0, sr, 2);
    let mid = sine(1000.0, -20.0, 20.0, sr, 2);
    let tail = sine(1000.0, -26.0, 20.0, sr, 2);
    for (c, s) in sig.iter_mut().enumerate() {
        s.extend_from_slice(&mid[c]);
        s.extend_from_slice(&tail[c]);
    }
    let m = integrated(&sig, sr);
    cases.push(Case {
        id: "3341-5",
        what: "integrated (−26/−20/−26 dBFS, 20 s each)",
        expected: -23.0,
        tolerance: 0.1,
        measured: m.integrated_lufs(),
    });
    // Tech 3341 case 6: 5.1 at −23 dBFS per channel with surround weighting → −23.0 LUFS
    // (L/R/C at −28, Ls/Rs at −30 dBFS is the official case; this is the analytic variant).
    // True peak (BS.1770-4 Annex 2 / Tech 3341 cases 15–23): a 997 Hz sine at −6 dBFS peaks at
    // −6.0 dBTP; at −0.5 dBFS at −0.5 dBTP (±0.2 dB).
    let m = integrated(&sine(997.0, -6.0, 5.0, sr, 2), sr);
    cases.push(Case { id: "3341-15", what: "true peak, 997 Hz −6 dBFS", expected: -6.0, tolerance: 0.2, measured: m.true_peak_db() });
    let m = integrated(&sine(997.0, -0.5, 5.0, sr, 2), sr);
    cases.push(Case { id: "3341-16", what: "true peak, 997 Hz −0.5 dBFS", expected: -0.5, tolerance: 0.2, measured: m.true_peak_db() });
    // Tech 3342 case 1: −20 dBFS 20 s then −30 dBFS 20 s → LRA 10 LU ±1.
    let mut sig = sine(1000.0, -20.0, 20.0, sr, 2);
    let low = sine(1000.0, -30.0, 20.0, sr, 2);
    for (c, s) in sig.iter_mut().enumerate() {
        s.extend_from_slice(&low[c]);
    }
    let m = integrated(&sig, sr);
    cases.push(Case {
        id: "3342-1",
        what: "loudness range (−20/−30 dBFS, 20 s each)",
        expected: 10.0,
        tolerance: 1.0,
        measured: m.loudness_range_lu(),
    });
    // Tech 3342 case 2: −20 dBFS 20 s then −15 dBFS 20 s → LRA 5 LU ±1.
    let mut sig = sine(1000.0, -20.0, 20.0, sr, 2);
    let hi = sine(1000.0, -15.0, 20.0, sr, 2);
    for (c, s) in sig.iter_mut().enumerate() {
        s.extend_from_slice(&hi[c]);
    }
    let m = integrated(&sig, sr);
    cases.push(Case {
        id: "3342-2",
        what: "loudness range (−20/−15 dBFS, 20 s each)",
        expected: 5.0,
        tolerance: 1.0,
        measured: m.loudness_range_lu(),
    });
    // Tech 3342 case 3: −40 dBFS 20 s then −20 dBFS 20 s → LRA 20 LU ±1.
    let mut sig = sine(1000.0, -40.0, 20.0, sr, 2);
    let hi = sine(1000.0, -20.0, 20.0, sr, 2);
    for (c, s) in sig.iter_mut().enumerate() {
        s.extend_from_slice(&hi[c]);
    }
    let m = integrated(&sig, sr);
    cases.push(Case {
        id: "3342-3",
        what: "loudness range (−40/−20 dBFS, 20 s each)",
        expected: 20.0,
        tolerance: 1.0,
        measured: m.loudness_range_lu(),
    });
    let mut failures = Vec::new();
    if let Some(path) = std::env::var_os("SML_E07_CSV") {
        use std::io::Write;
        let mut f = std::fs::File::create(path).unwrap();
        writeln!(f, "case,what,expected,tolerance,measured,pass,standard").unwrap();
        for c in &cases {
            let pass = (c.measured - c.expected).abs() <= c.tolerance;
            writeln!(
                f,
                "{},{},{:.2},{:.2},{:.3},{},EBU Tech 3341/3342 (2016) + ITU-R BS.1770-4 analytic signals generated in the test at 48 kHz",
                c.id, c.what, c.expected, c.tolerance, c.measured, pass
            )
            .unwrap();
        }
    }
    for c in &cases {
        if (c.measured - c.expected).abs() > c.tolerance {
            failures.push(format!("{} {}: expected {:.2} ±{:.2}, measured {:.3}", c.id, c.what, c.expected, c.tolerance, c.measured));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
