//! RC05/RC06 measurements of the assistant runtime in the build environment (NOT a native or
//! listener measurement): grounded-rule answer time, prompt size, speech synthesis and
//! recognition cost, peak memory, and whether recognition running beside the mixer disturbs
//! the mixer's block compute time. The language model's own speed is not measured: no model is
//! reachable here (X03); the loopback fixture only times the client.
//!
//! `cargo run --release -p soundcraft-engine --example assistant_measure -- out.csv`

use serde_json::json;
use soundcraft_audio_io::{AudioBuffer, FileFormat};
use soundcraft_engine::Engine;
use soundcraft_engine::assistant::{Request, Unavailable};
use soundcraft_model::{ChannelFormat, Clip, TrackKind};
use soundcraft_speech::recog::Recognizer;
use soundcraft_speech::synth::{Voice, synthesize};
use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

fn pct(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(((v.len() as f64 - 1.0) * p).round() as usize).copied().unwrap_or(0.0)
}

fn peak_rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmHWM:")).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse().ok()))
        .unwrap_or(0)
}

fn ptt(text: &str, v: Voice) -> Vec<f32> {
    let mut x = vec![0.0f32; 3_200];
    x.extend(synthesize(text, v));
    x.extend(vec![0.0f32; 3_200]);
    x
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "assistant-measure.csv".into());
    let mut csv = String::from("measure,n,p50,p99,max,unit,note\n");
    let mut row = |name: &str, v: &mut Vec<f64>, unit: &str, note: &str| {
        let n = v.len();
        let (p50, p99) = (pct(v, 0.5), pct(v, 0.99));
        let max = v.iter().copied().fold(0.0, f64::max);
        let _ = writeln!(csv, "{name},{n},{p50:.3},{p99:.3},{max:.3},{unit},{note}");
        println!("{name}: n {n} p50 {p50:.3} p99 {p99:.3} max {max:.3} {unit}");
    };
    // A 16-track session like the callback profile's.
    let mut e = Engine::default();
    let sr = 48_000u32;
    let len = sr as usize * 10;
    for k in 0..16u32 {
        let s = e.session_mut();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some(&format!("Track {k}")));
        let data = vec![(0..len).map(|i| ((i as f32 * (0.01 + k as f32 * 0.001)).sin()) * 0.05).collect::<Vec<f32>>()];
        let src = soundcraft_engine::io::add_source(s, &format!("t{k}"), AudioBuffer { sample_rate: sr, channels: data }, None, FileFormat::Wav);
        let id = s.new_clip_id();
        if let Some(tr) = s.track_mut(t)
            && let Some(pl) = tr.playlist_mut()
        {
            pl.clips.push(Clip::audio(id, "c", src, 0, 0, len as i64));
        }
    }
    e.session_mut().add_track(TrackKind::Master, ChannelFormat::Stereo, Some("Master"));
    // 1. Grounded-rule answers (the typed and the heard path both end here).
    let questions = [
        "mute track 3",
        "set the track 2 volume to -6",
        "where is nudge",
        "what are the limits of the sandbox mode",
        "is track 1 clipping?",
        "how do I bounce the mix",
        "open the melodyne editor",
        "delete the session file",
    ];
    let mut t = Vec::new();
    for _ in 0..25 {
        for q in questions {
            let t0 = Instant::now();
            let _ = e.assistant_ask(&Request { text: q.into(), ..Request::default() }, &Unavailable);
            t.push(t0.elapsed().as_secs_f64() * 1e3);
        }
    }
    row("rule answer time", &mut t, "ms", "8 varied requests x 25 on a 16-track session (includes Engineer measurements)");
    // 2. Model prompt: build time and size (what a local model would have to read).
    let mut t = Vec::new();
    let mut chars = Vec::new();
    for q in questions {
        let t0 = Instant::now();
        let (s, u, _) = e.assistant_model_prompt(q);
        t.push(t0.elapsed().as_secs_f64() * 1e3);
        chars.push((s.len() + u.len()) as f64);
    }
    row("model prompt build time", &mut t, "ms", "system prompt with session summary and retrieved help");
    row("model prompt size", &mut chars, "chars", "about 4 characters per token for English text");
    // 3. Speech synthesis.
    let sentence = "Plan: mute track three, one undo step. Run it from the assistant window.";
    let mut t = Vec::new();
    let mut rtf = Vec::new();
    for _ in 0..20 {
        let t0 = Instant::now();
        let y = synthesize(sentence, Voice::default());
        let dt = t0.elapsed().as_secs_f64();
        t.push(dt * 1e3);
        rtf.push(dt / (y.len() as f64 / 16_000.0));
    }
    row("speech synthesis time", &mut t, "ms", "one 72-character sentence");
    row("speech synthesis real-time factor", &mut rtf, "x", "compute time / audio duration (lower is faster)");
    // 4. Recognition: 8 and 64 enrolled phrases.
    let base = ["play", "stop", "record", "undo", "save", "mute kick", "louder", "next marker"];
    for n in [8usize, 64] {
        let mut r = Recognizer::default();
        for k in 0..n {
            let phrase = if k < base.len() {
                base[k].to_string()
            } else {
                format!("{} {}", base[k % base.len()], ["one", "two", "three", "four", "five", "six", "seven", "eight"][k / base.len() % 8])
            };
            let _ = r.enroll(&phrase, &ptt(&phrase, Voice::default()), 16_000);
        }
        let test = ptt("next marker", Voice { pitch_hz: 150.0, ..Voice::default() });
        let mut t = Vec::new();
        for _ in 0..10 {
            let t0 = Instant::now();
            let _ = r.recognize(&test, 16_000);
            t.push(t0.elapsed().as_secs_f64() * 1e3);
        }
        row(
            &format!("recognition time ({} phrases)", r.phrases.len()),
            &mut t,
            "ms",
            "one 1.2 s utterance; runs on its own thread in the desktop app",
        );
    }
    // 5. Background impact: mixer block compute with and without recognition looping on another thread.
    let session = e.session().clone();
    let measure_blocks = |label: &str, csv_row: &mut dyn FnMut(&str, &mut Vec<f64>, &str, &str)| {
        let mut eng = soundcraft_mix::MixEngine::new(sr as f32, 256);
        let mut buf = vec![vec![0.0f32; 256]; 2];
        let mut t = Vec::new();
        for b in 0..4_000i64 {
            let t0 = Instant::now();
            eng.render(&session, (b * 256) % (len as i64 - 512), 256, &mut buf);
            t.push(t0.elapsed().as_secs_f64() * 1e6);
        }
        csv_row(label, &mut t, "us", "256-frame mixer block (budget 5333 us); same process");
    };
    measure_blocks("mixer block compute (idle assistant)", &mut row);
    let stop = Arc::new(AtomicBool::new(false));
    let s2 = Arc::clone(&stop);
    let mut r = Recognizer::default();
    for p in base {
        let _ = r.enroll(p, &ptt(p, Voice::default()), 16_000);
    }
    let bg = std::thread::spawn(move || {
        let x = ptt("mute kick", Voice { pitch_hz: 140.0, ..Voice::default() });
        let mut n = 0u64;
        while !s2.load(Ordering::Relaxed) {
            let _ = r.recognize(&x, 16_000);
            let _ = synthesize("kick muted", Voice::default());
            n += 1;
        }
        n
    });
    measure_blocks("mixer block compute (recognition + synthesis looping on another thread)", &mut row);
    stop.store(true, Ordering::Relaxed);
    let loops = bg.join().unwrap_or(0);
    let _ = writeln!(csv, "background speech loops during the run,{loops},,,,count,");
    let rss = peak_rss_kib();
    let _ = writeln!(csv, "peak resident memory,1,{rss},,,KiB,whole process incl. the 16-track session");
    println!("peak RSS {rss} KiB; background loops {loops}");
    let _ = std::fs::write(&out, csv);
    let _ = json!({});
}
