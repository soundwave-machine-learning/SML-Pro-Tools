//! Rule-based formant speech synthesis (original implementation of the textbook cascade/parallel
//! formant synthesizer; formant targets are standard published averages for adult speakers).
//!
//! Text → words → phonemes (a short lexicon, else letter-to-sound rules) → timed segments with
//! formant/amplitude targets → 5 ms parameter frames smoothed into each other (coarticulation) →
//! per-sample synthesis: an impulse train shaped into a glottal pulse plus aspiration noise
//! through five cascaded resonators (F1–F3 follow the targets, F4/F5 fixed), and a parallel
//! noise resonator for frication. Output is mono f32 at [`crate::SPEECH_RATE`], peak 0.8.

use crate::SPEECH_RATE;

/// Voice settings.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Voice {
    /// Mean fundamental frequency (Hz).
    pub pitch_hz: f32,
    /// Speaking rate (1 = normal; 1.25 is faster).
    pub rate: f32,
    /// Scales every formant (vocal-tract length; 1 = the reference adult).
    pub formant_scale: f32,
}

impl Default for Voice {
    fn default() -> Self {
        Voice { pitch_hz: 110.0, rate: 1.0, formant_scale: 1.0 }
    }
}

impl Voice {
    fn sane(self) -> Voice {
        let ok = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        Voice { pitch_hz: ok(self.pitch_hz, 50.0, 400.0, 110.0), rate: ok(self.rate, 0.5, 2.0, 1.0), formant_scale: ok(self.formant_scale, 0.7, 1.4, 1.0) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Vowel,
    Glide,
    Nasal,
    Fric,
    Stop,
    Aspirate,
    Pause,
}

/// One phoneme: `dur` ms; formants (Hz); `av` voicing, `af` frication (centre `ff` Hz).
#[derive(Debug, Clone, Copy)]
struct Ph {
    name: &'static str,
    kind: Kind,
    dur: f32,
    f: [f32; 3],
    av: f32,
    af: f32,
    ff: f32,
    voiced: bool,
}

const fn v(name: &'static str, dur: f32, f1: f32, f2: f32, f3: f32) -> Ph {
    Ph { name, kind: Kind::Vowel, dur, f: [f1, f2, f3], av: 1.0, af: 0.0, ff: 0.0, voiced: true }
}
const fn g(name: &'static str, kind: Kind, dur: f32, f1: f32, f2: f32, f3: f32, av: f32) -> Ph {
    Ph { name, kind, dur, f: [f1, f2, f3], av, af: 0.0, ff: 0.0, voiced: true }
}
const fn fr(name: &'static str, dur: f32, ff: f32, af: f32, voiced: bool, f2: f32) -> Ph {
    Ph { name, kind: Kind::Fric, dur, f: [400.0, f2, 2600.0], av: if voiced { 0.35 } else { 0.0 }, af, ff, voiced }
}
const fn st(name: &'static str, ff: f32, voiced: bool, f2: f32) -> Ph {
    Ph { name, kind: Kind::Stop, dur: 70.0, f: [300.0, f2, 2600.0], av: 0.0, af: 0.9, ff, voiced }
}

/// The phoneme inventory (ARPAbet names).
const PHONEMES: &[Ph] = &[
    v("IY", 120.0, 270.0, 2290.0, 3010.0),
    v("IH", 90.0, 390.0, 1990.0, 2550.0),
    v("EH", 100.0, 530.0, 1840.0, 2480.0),
    v("AE", 130.0, 660.0, 1720.0, 2410.0),
    v("AA", 130.0, 730.0, 1090.0, 2440.0),
    v("AO", 130.0, 570.0, 840.0, 2410.0),
    v("UH", 90.0, 440.0, 1020.0, 2240.0),
    v("UW", 120.0, 300.0, 870.0, 2240.0),
    v("AH", 90.0, 520.0, 1190.0, 2390.0),
    v("ER", 120.0, 490.0, 1350.0, 1690.0),
    v("AX", 60.0, 500.0, 1500.0, 2500.0),
    g("L", Kind::Glide, 70.0, 310.0, 1050.0, 2880.0, 0.7),
    g("R", Kind::Glide, 70.0, 310.0, 1060.0, 1380.0, 0.7),
    g("W", Kind::Glide, 60.0, 290.0, 610.0, 2150.0, 0.7),
    g("Y", Kind::Glide, 60.0, 260.0, 2070.0, 3020.0, 0.7),
    g("M", Kind::Nasal, 70.0, 280.0, 900.0, 2200.0, 0.45),
    g("N", Kind::Nasal, 70.0, 280.0, 1700.0, 2600.0, 0.45),
    g("NG", Kind::Nasal, 80.0, 280.0, 2300.0, 2750.0, 0.45),
    fr("S", 110.0, 5500.0, 0.6, false, 1700.0),
    fr("Z", 90.0, 5500.0, 0.4, true, 1700.0),
    fr("SH", 110.0, 2800.0, 0.6, false, 1800.0),
    fr("ZH", 90.0, 2800.0, 0.4, true, 1800.0),
    fr("F", 100.0, 6500.0, 0.25, false, 1100.0),
    fr("V", 70.0, 6500.0, 0.2, true, 1100.0),
    fr("TH", 100.0, 6000.0, 0.2, false, 1400.0),
    fr("DH", 60.0, 6000.0, 0.15, true, 1400.0),
    Ph { name: "HH", kind: Kind::Aspirate, dur: 60.0, f: [500.0, 1500.0, 2500.0], av: 0.0, af: 0.0, ff: 0.0, voiced: false },
    st("P", 1000.0, false, 800.0),
    st("B", 1000.0, true, 800.0),
    st("T", 4000.0, false, 1800.0),
    st("D", 4000.0, true, 1800.0),
    st("K", 2000.0, false, 2300.0),
    st("G", 2000.0, true, 2300.0),
    Ph { name: "_", kind: Kind::Pause, dur: 150.0, f: [500.0, 1500.0, 2500.0], av: 0.0, af: 0.0, ff: 0.0, voiced: false },
    Ph { name: "-", kind: Kind::Pause, dur: 25.0, f: [500.0, 1500.0, 2500.0], av: 0.0, af: 0.0, ff: 0.0, voiced: false },
];

/// Two-part phonemes (diphthongs and affricates) as their parts.
const COMPOUND: &[(&str, &[&str])] = &[("EY", &["EH", "IY"]), ("AY", &["AA", "IY"]), ("OW", &["AO", "UW"]), ("AW", &["AA", "UW"]), ("OY", &["AO", "IY"]), ("CH", &["T", "SH"]), ("JH", &["D", "ZH"])];

fn phoneme(name: &str) -> Option<&'static Ph> {
    PHONEMES.iter().find(|p| p.name == name)
}

/// Hand-written pronunciations for words the rules get wrong or the assistant says often.
const LEXICON: &[(&str, &str)] = &[
    ("a", "AX"),
    ("the", "DH AX"),
    ("to", "T UW"),
    ("do", "D UW"),
    ("you", "Y UW"),
    ("is", "IH Z"),
    ("was", "W AA Z"),
    ("of", "AH V"),
    ("one", "W AH N"),
    ("two", "T UW"),
    ("four", "F AO R"),
    ("eight", "EY T"),
    ("zero", "Z IY R OW"),
    ("play", "P L EY"),
    ("stop", "S T AA P"),
    ("record", "R IH K AO R D"),
    ("undo", "AH N D UW"),
    ("save", "S EY V"),
    ("mute", "M Y UW T"),
    ("muted", "M Y UW T IH D"),
    ("kick", "K IH K"),
    ("louder", "L AW D ER"),
    ("quieter", "K W AY AX T ER"),
    ("next", "N EH K S T"),
    ("marker", "M AA R K ER"),
    ("track", "T R AE K"),
    ("plan", "P L AE N"),
    ("volume", "V AA L Y UW M"),
    ("not", "N AA T"),
    ("available", "AX V EY L AX B AX L"),
    ("there", "DH EH R"),
    ("they", "DH EY"),
    ("this", "DH IH S"),
    ("that", "DH AE T"),
    ("with", "W IH TH"),
    ("what", "W AH T"),
    ("are", "AA R"),
    ("have", "HH AE V"),
    ("done", "D AH N"),
    ("banana", "B AX N AE N AX"),
    ("delete", "D IH L IY T"),
    ("everything", "EH V R IY TH IH NG"),
    ("hello", "HH EH L OW"),
];

const DIGITS: [&str; 10] = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine"];

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')
}

/// Letter-to-sound rules for one lower-case word (letters only).
fn letters_to_phonemes(w: &str) -> Vec<&'static str> {
    let c: Vec<char> = w.chars().collect();
    let n = c.len();
    let at = |i: usize| c.get(i).copied().unwrap_or(' ');
    let starts = |i: usize, s: &str| s.chars().enumerate().all(|(k, ch)| at(i + k) == ch);
    // Magic e: vowel + one consonant + final e → long vowel, silent e.
    let magic = n >= 3 && at(n - 1) == 'e' && !is_vowel(at(n - 2)) && is_vowel(at(n - 3));
    let mut out: Vec<&'static str> = Vec::new();
    let mut i = 0;
    const DIGRAPHS: &[(&str, &[&str])] = &[
        ("tion", &["SH", "AX", "N"]),
        ("sion", &["ZH", "AX", "N"]),
        ("tch", &["CH"]),
        ("igh", &["AY"]),
        ("th", &["TH"]),
        ("sh", &["SH"]),
        ("ch", &["CH"]),
        ("ph", &["F"]),
        ("wh", &["W"]),
        ("ck", &["K"]),
        ("ng", &["NG"]),
        ("qu", &["K", "W"]),
        ("ee", &["IY"]),
        ("ea", &["IY"]),
        ("oo", &["UW"]),
        ("ou", &["AW"]),
        ("ai", &["EY"]),
        ("ay", &["EY"]),
        ("oa", &["OW"]),
        ("oi", &["OY"]),
        ("oy", &["OY"]),
        ("au", &["AO"]),
        ("aw", &["AO"]),
        ("ew", &["UW"]),
        ("ie", &["IY"]),
        ("ei", &["EY"]),
        ("er", &["ER"]),
        ("ir", &["ER"]),
        ("ur", &["ER"]),
        ("ar", &["AA", "R"]),
        ("or", &["AO", "R"]),
    ];
    while i < n {
        let ch = at(i);
        if i == 0 && (starts(0, "kn") || starts(0, "wr")) {
            out.push(if ch == 'k' { "N" } else { "R" });
            i += 2;
            continue;
        }
        if i + 2 == n && ch == 'l' && at(i + 1) == 'e' && i > 0 && !is_vowel(at(i - 1)) {
            out.extend(["AX", "L"]);
            break;
        }
        if starts(i, "ow") {
            out.push(if i + 2 == n { "OW" } else { "AW" });
            i += 2;
            continue;
        }
        if let Some((d, ps)) = DIGRAPHS.iter().find(|(d, _)| starts(i, d)) {
            out.extend(ps.iter().copied());
            i += d.len();
            continue;
        }
        // Doubled consonants sound once.
        if i + 1 < n && ch == at(i + 1) && !is_vowel(ch) {
            i += 1;
            continue;
        }
        let next = at(i + 1);
        let soft = matches!(next, 'e' | 'i' | 'y');
        let p: &[&'static str] = match ch {
            'a' | 'e' | 'i' | 'o' | 'u' => {
                let long = (magic && i == n - 3) || (i + 1 == n && n <= 3);
                if ch == 'e' && i + 1 == n && n > 2 {
                    &[]
                } else if long {
                    match ch {
                        'a' => &["EY"],
                        'e' => &["IY"],
                        'i' => &["AY"],
                        'o' => &["OW"],
                        _ => &["Y", "UW"],
                    }
                } else {
                    match ch {
                        'a' => &["AE"],
                        'e' => &["EH"],
                        'i' => &["IH"],
                        'o' => &["AA"],
                        _ => &["AH"],
                    }
                }
            }
            'y' => {
                if i == 0 {
                    &["Y"]
                } else if i + 1 == n {
                    if n <= 3 { &["AY"] } else { &["IY"] }
                } else {
                    &["IH"]
                }
            }
            'b' => &["B"],
            'c' => {
                if soft {
                    &["S"]
                } else {
                    &["K"]
                }
            }
            'd' => &["D"],
            'f' => &["F"],
            'g' => {
                if soft && next != 'e' {
                    &["JH"]
                } else {
                    &["G"]
                }
            }
            'h' => &["HH"],
            'j' => &["JH"],
            'k' => &["K"],
            'l' => &["L"],
            'm' => &["M"],
            'n' => &["N"],
            'p' => &["P"],
            'q' => &["K"],
            'r' => &["R"],
            's' => {
                if i + 1 == n && i > 0 && matches!(at(i - 1), 'b' | 'd' | 'g' | 'l' | 'm' | 'n' | 'r' | 'v') {
                    &["Z"]
                } else {
                    &["S"]
                }
            }
            't' => &["T"],
            'v' => &["V"],
            'w' => &["W"],
            'x' => &["K", "S"],
            'z' => &["Z"],
            _ => &[],
        };
        out.extend(p.iter().copied());
        i += 1;
    }
    out
}

/// Text to the phoneme sequence the synthesizer speaks (`_` pause, `-` word gap).
pub fn phonemes(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut Vec<String>| {
        if word.is_empty() {
            return;
        }
        let w = std::mem::take(word);
        let ps: Vec<String> = match LEXICON.iter().find(|(k, _)| *k == w) {
            Some((_, p)) => p.split_whitespace().map(str::to_string).collect(),
            None => letters_to_phonemes(&w).into_iter().map(str::to_string).collect(),
        };
        if !ps.is_empty() {
            if out.last().is_some_and(|l| l != "_") {
                out.push("-".into());
            }
            out.extend(ps);
        }
    };
    for ch in text.chars().take(2_000) {
        if ch.is_ascii_alphabetic() || ch == '\'' {
            if ch != '\'' {
                word.push(ch.to_ascii_lowercase());
            }
        } else if let Some(d) = ch.to_digit(10) {
            flush(&mut word, &mut out);
            word.push_str(DIGITS.get(d as usize).copied().unwrap_or(""));
            flush(&mut word, &mut out);
        } else {
            flush(&mut word, &mut out);
            if matches!(ch, '.' | ',' | ';' | ':' | '?' | '!') && out.last().is_some_and(|l| l != "_") {
                out.push("_".into());
            }
        }
    }
    flush(&mut word, &mut out);
    while out.last().is_some_and(|l| l == "_" || l == "-") {
        out.pop();
    }
    out
}

/// One synthesis segment: duration (samples) and targets.
#[derive(Debug, Clone, Copy)]
struct Seg {
    len: usize,
    f: [f32; 3],
    av: f32,
    af: f32,
    ff: f32,
    ah: f32,
}

fn segments(ph: &[String], voice: Voice) -> Vec<Seg> {
    let sr = SPEECH_RATE as f32;
    let ms = |d: f32| ((d / voice.rate) * sr / 1000.0).round().max(1.0) as usize;
    let mut segs = Vec::new();
    let mut names: Vec<&str> = Vec::new();
    for p in ph {
        match COMPOUND.iter().find(|(c, _)| c == p) {
            Some((_, parts)) => names.extend(parts.iter().copied()),
            None => names.push(p.as_str()),
        }
    }
    for (k, name) in names.iter().enumerate() {
        let Some(p) = phoneme(name) else { continue };
        let next_f = names.get(k + 1..).and_then(|r| r.iter().filter_map(|n| phoneme(n)).find(|q| q.kind == Kind::Vowel)).map_or(p.f, |q| q.f);
        let sc = |f: [f32; 3]| f.map(|x| x * voice.formant_scale);
        match p.kind {
            Kind::Stop => {
                // Closure (a voice bar for voiced stops), burst, then aspiration for voiceless.
                segs.push(Seg { len: ms(p.dur * 0.6), f: sc(p.f), av: if p.voiced { 0.12 } else { 0.0 }, af: 0.0, ff: p.ff, ah: 0.0 });
                segs.push(Seg { len: ms(12.0), f: sc(p.f), av: 0.0, af: p.af, ff: p.ff * voice.formant_scale, ah: 0.0 });
                if !p.voiced {
                    segs.push(Seg { len: ms(35.0), f: sc(next_f), av: 0.0, af: 0.0, ff: p.ff, ah: 0.45 });
                }
            }
            Kind::Aspirate => segs.push(Seg { len: ms(p.dur), f: sc(next_f), av: 0.0, af: 0.0, ff: 0.0, ah: 0.5 }),
            Kind::Pause => segs.push(Seg { len: ms(p.dur), f: sc(p.f), av: 0.0, af: 0.0, ff: 0.0, ah: 0.0 }),
            Kind::Fric => segs.push(Seg { len: ms(p.dur), f: sc(p.f), av: p.av, af: p.af, ff: p.ff * voice.formant_scale, ah: 0.0 }),
            Kind::Vowel | Kind::Glide | Kind::Nasal => segs.push(Seg { len: ms(p.dur), f: sc(p.f), av: p.av, af: 0.0, ff: 0.0, ah: 0.0 }),
        }
    }
    segs
}

/// Two-pole resonator (gain-normalised at DC for the cascade).
#[derive(Debug, Clone, Copy, Default)]
struct Reso {
    a: f32,
    b: f32,
    c: f32,
    y1: f32,
    y2: f32,
}

impl Reso {
    fn set(&mut self, f: f32, bw: f32) {
        let t = 1.0 / SPEECH_RATE as f32;
        let r = (-std::f32::consts::PI * bw * t).exp();
        self.c = -r * r;
        self.b = 2.0 * r * (2.0 * std::f32::consts::PI * f * t).cos();
        self.a = 1.0 - self.b - self.c;
    }
    fn tick(&mut self, x: f32) -> f32 {
        let y = self.a * x + self.b * self.y1 + self.c * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Speaks `text` with `voice`: mono f32 at [`SPEECH_RATE`], peak 0.8 (empty for no speakable text).
pub fn synthesize(text: &str, voice: Voice) -> Vec<f32> {
    synthesize_phonemes(&phonemes(text), voice)
}

/// Like [`synthesize`] from a phoneme sequence (ARPAbet names, `_` pause, `-` word gap).
pub fn synthesize_phonemes(ph: &[String], voice: Voice) -> Vec<f32> {
    let voice = voice.sane();
    let segs = segments(ph, voice);
    let total: usize = segs.iter().map(|s| s.len).sum();
    if total == 0 {
        return Vec::new();
    }
    // 5 ms frames of targets, smoothed so formants glide between phonemes.
    let frame = (SPEECH_RATE / 200) as usize;
    let mut targets: Vec<Seg> = Vec::with_capacity(total / frame + 2);
    for s in &segs {
        for _ in 0..s.len.div_ceil(frame) {
            targets.push(*s);
        }
    }
    let mut cur = targets.first().copied().unwrap_or(Seg { len: 0, f: [500.0, 1500.0, 2500.0], av: 0.0, af: 0.0, ff: 0.0, ah: 0.0 });
    let kf = 0.35f32; // formant glide per frame (~12 ms time constant)
    let ka = 0.6f32; // amplitude follow per frame
    let frames: Vec<Seg> = targets
        .iter()
        .map(|t| {
            for (c, x) in cur.f.iter_mut().zip(t.f) {
                *c += kf * (x - *c);
            }
            cur.av += ka * (t.av - cur.av);
            cur.af += ka * (t.af - cur.af);
            cur.ah += ka * (t.ah - cur.ah);
            cur.ff = t.ff;
            cur
        })
        .collect();
    let mut out = Vec::with_capacity(frames.len() * frame);
    let (mut r1, mut r2, mut r3, mut rf) = (Reso::default(), Reso::default(), Reso::default(), Reso::default());
    let (mut r4, mut r5, mut glot) = (Reso::default(), Reso::default(), Reso::default());
    r4.set(3500.0 * voice.formant_scale, 250.0);
    r5.set(4500.0 * voice.formant_scale, 300.0);
    // Glottal pulse shaping: a resonator at 0 Hz is a smooth low-pass.
    glot.set(0.0, 100.0);
    let mut phase = 0.0f32;
    let mut seed: u32 = 0x1234_5678;
    let mut noise = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let mut prev_g = 0.0f32;
    let n_frames = frames.len().max(1) as f32;
    for (k, fp) in frames.iter().enumerate() {
        r1.set(fp.f[0], 60.0 + fp.f[0] * 0.05);
        r2.set(fp.f[1], 90.0 + fp.f[1] * 0.03);
        r3.set(fp.f[2], 150.0);
        if fp.ff > 0.0 {
            rf.set(fp.ff, (fp.ff * 0.35).max(400.0));
        }
        // Declination: the pitch falls through the utterance.
        let f0 = voice.pitch_hz * (1.1 - 0.25 * k as f32 / n_frames);
        for _ in 0..frame {
            phase += f0 / SPEECH_RATE as f32;
            let pulse = if phase >= 1.0 {
                phase -= 1.0;
                1.0
            } else {
                0.0
            };
            let g = glot.tick(pulse);
            // Lip radiation: first difference.
            let src = (g - prev_g) * 40.0 * fp.av + noise() * 0.08 * fp.ah;
            prev_g = g;
            let voiced = r5.tick(r4.tick(r3.tick(r2.tick(r1.tick(src)))));
            let fric = if fp.af > 0.0 { rf.tick(noise()) * fp.af * 0.6 } else { 0.0 };
            out.push(voiced + fric);
        }
    }
    // DC block and normalise.
    let mut x1 = 0.0f32;
    let mut y1 = 0.0f32;
    for s in &mut out {
        let y = *s - x1 + 0.995 * y1;
        x1 = *s;
        y1 = y;
        *s = if y.is_finite() { y } else { 0.0 };
    }
    let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    if peak > 1e-9 {
        for s in &mut out {
            *s *= 0.8 / peak;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_becomes_phonemes() {
        assert_eq!(phonemes("Play"), ["P", "L", "EY"]);
        assert_eq!(phonemes("mute kick"), ["M", "Y", "UW", "T", "-", "K", "IH", "K"]);
        assert_eq!(phonemes("make"), ["M", "EY", "K"], "magic e");
        assert_eq!(phonemes("shop"), ["SH", "AA", "P"]);
        assert_eq!(phonemes("bus 2."), ["B", "AH", "S", "-", "T", "UW"]);
        assert_eq!(phonemes("a, b"), ["AX", "_", "B"]);
        assert!(phonemes("").is_empty() && phonemes("!!! ...").is_empty());
        // Every rule output is a known phoneme.
        for w in ["through", "knight", "whistle", "question", "vision", "rhythm", "xylophone", "queue", "strengths"] {
            for p in phonemes(w) {
                assert!(phoneme(&p).is_some() || COMPOUND.iter().any(|(c, _)| *c == p) || p == "-", "{w}: {p}");
            }
        }
    }

    #[test]
    fn synthesis_is_finite_bounded_and_scales_with_rate() {
        let a = synthesize("mute kick", Voice::default());
        assert!(a.len() > 3_000 && a.len() < 48_000, "{}", a.len());
        assert!(a.iter().all(|x| x.is_finite()));
        let peak = a.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!((peak - 0.8).abs() < 1e-3);
        let slow = synthesize("mute kick", Voice { rate: 0.8, ..Voice::default() });
        assert!(slow.len() > a.len() * 11 / 10);
        assert!(synthesize("", Voice::default()).is_empty());
        // Hostile settings and text never panic.
        let _ = synthesize(&"é€😀 a".repeat(500), Voice { pitch_hz: f32::NAN, rate: 0.0, formant_scale: f32::INFINITY });
    }

    /// The vowel /AA/ has its energy near F1 ≈ 730 Hz and /IY/ near F1 ≈ 270 Hz: the low-band
    /// share of /IY/ sits lower, measured on a sustained synthesized vowel.
    #[test]
    fn vowels_carry_their_formants() {
        let spec = |ph: &str| {
            let segs = segments(&[ph.to_string(), ph.to_string(), ph.to_string()], Voice::default());
            let n: usize = segs.iter().map(|s| s.len).sum();
            assert!(n > 4000);
            let x = synthesize_segments_for_test(ph);
            // Energy centroid below 1 kHz via a crude DFT.
            let mut num = 0.0f64;
            let mut den = 0.0f64;
            for f in (100..1000).step_by(20) {
                let w = 2.0 * std::f64::consts::PI * f as f64 / SPEECH_RATE as f64;
                let (mut re, mut im) = (0.0f64, 0.0f64);
                for (i, s) in x.iter().enumerate() {
                    re += *s as f64 * (w * i as f64).cos();
                    im += *s as f64 * (w * i as f64).sin();
                }
                let p = re * re + im * im;
                num += p * f as f64;
                den += p;
            }
            num / den
        };
        let aa = spec("AA");
        let iy = spec("IY");
        assert!(aa > iy + 150.0, "AA centroid {aa:.0} Hz vs IY {iy:.0} Hz");
    }

    fn synthesize_segments_for_test(ph: &str) -> Vec<f32> {
        synthesize_phonemes(&vec![ph.to_string(); 4], Voice::default())
    }
}
