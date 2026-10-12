//! RC10 (Remote Continuation Sprint 4): decoded exports match the intended graph and format —
//! the track and Master chains are applied exactly once, float exports are the offline render
//! sample for sample, the requested rate/depth/tail are what the file holds, and dither is
//! applied once (TPDF ±1 LSB: about 0.5 LSB RMS on digital silence after rounding). Monitor,
//! reference and assistant speech never reach a bounce (covered in playback and e08 tests).

use serde_json::{Value, json};
use soundcraft_audio_io::{AudioBuffer, SampleFormat};
use soundcraft_engine::Engine;
use soundcraft_model::{ChannelFormat, Clip, Insert, SourceAudio, SourceId, TrackKind};
use std::sync::Arc;

const N: usize = 48_000;

fn engine(track_insert: Option<&str>, master_insert: Option<&str>, level: f32) -> Engine {
    let mut e = Engine::default();
    let s = e.session_mut();
    let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Tone"));
    let tone: Vec<f32> = (0..N).map(|i| (2.0 * std::f32::consts::PI * 441.0 * i as f32 / 48_000.0).sin() * level).collect();
    s.pool.insert(SourceId(9), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![tone] })));
    let id = s.new_clip_id();
    s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "tone", SourceId(9), 0, 0, N as i64));
    if let Some(p) = track_insert {
        s.track_mut(t).unwrap().mixer.inserts[0] = Some(Insert::new(p));
    }
    let m = s.add_track(TrackKind::Master, ChannelFormat::Stereo, None);
    if let Some(p) = master_insert {
        s.track_mut(m).unwrap().mixer.inserts[0] = Some(Insert::new(p));
    }
    e
}

fn bounce(e: &mut Engine, name: &str, extra: Value) -> (soundcraft_audio_io::AudioInfo, AudioBuffer, Value) {
    let dir = std::env::temp_dir().join(format!("sml-rc10-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    let mut p = json!({"path": path.to_string_lossy(), "start": 0, "end": N});
    for (k, v) in extra.as_object().unwrap() {
        p[k] = v.clone();
    }
    let r = e.execute("file.bounce_mix", &p).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let (info, buf) = soundcraft_audio_io::decode(&bytes, Some("wav")).unwrap();
    (info, buf, r)
}

fn peak(b: &AudioBuffer) -> f32 {
    b.channels.iter().flatten().fold(0.0f32, |m, x| m.max(x.abs()))
}

#[test]
fn exports_match_the_intended_graph_and_format() {
    // Chains applied once: −6 dB on the track and −6 dB on Master → exactly −12 dB overall.
    let (_, plain, _) = bounce(&mut engine(None, None, 0.5), "plain.wav", json!({"bit_depth": "32f"}));
    let (_, chained, r) = bounce(&mut engine(Some("test:gain:-6"), Some("test:gain:-6"), 0.5), "chained.wav", json!({"bit_depth": "32f"}));
    let ratio = peak(&chained) / peak(&plain);
    assert!((ratio - 10f32.powf(-12.0 / 20.0)).abs() < 0.002, "track + Master chains applied exactly once: ratio {ratio}");
    assert_eq!(r["verified"]["header_ok"], true, "{r}");
    // Float export = the offline render, sample for sample.
    let e = engine(Some("test:gain:-6"), Some("test:gain:-6"), 0.5);
    let render = soundcraft_mix::render_range_report(e.session(), soundcraft_time::Range::new(0, N as i64), 1024, soundcraft_mix::Tail::None).audio;
    let diff = render.iter().zip(&chained.channels).flat_map(|(a, b)| a.iter().zip(b).map(|(x, y)| (x - y).abs())).fold(0.0f32, f32::max);
    assert!(diff < 1e-6, "float export equals the render (max diff {diff})");
    // Rate conversion and depth.
    let (info, buf, _) = bounce(&mut engine(None, None, 0.5), "r441.wav", json!({"rate": 44_100, "bit_depth": "16"}));
    assert_eq!((info.sample_rate, info.sample_format), (44_100, SampleFormat::Int16));
    let want = N * 441 / 480;
    assert!((info.frames as usize).abs_diff(want) <= 2, "{} frames vs {want}", info.frames);
    assert!((peak(&buf) - peak(&plain)).abs() < 0.01, "level kept through the conversion");
    let (info, _, _) = bounce(&mut engine(None, None, 0.5), "d24.wav", json!({"bit_depth": "24"}));
    assert_eq!(info.sample_format, SampleFormat::Int24);
    // Tail: a plugin declaring a 4 800-sample tail adds exactly that with tail auto, none without.
    let (info, _, r) = bounce(&mut engine(Some("test:tail:4800"), None, 0.5), "tail.wav", json!({"tail": "auto", "bit_depth": "32f"}));
    assert_eq!(info.frames as usize, N + 4_800, "{r}");
    let (info, _, _) = bounce(&mut engine(Some("test:tail:4800"), None, 0.5), "notail.wav", json!({"bit_depth": "32f"}));
    assert_eq!(info.frames as usize, N);
    // Dither once: digital silence at 16 bits with dither shows TPDF noise of about 0.5 LSB RMS;
    // without dither it stays exactly zero.
    let (_, dithered, _) = bounce(&mut engine(None, None, 0.0), "dith.wav", json!({"bit_depth": "16", "dither": true}));
    let lsb = 1.0 / 32_768.0;
    let n = dithered.channels.iter().map(Vec::len).sum::<usize>() as f32;
    let rms = (dithered.channels.iter().flatten().map(|x| (x / lsb) * (x / lsb)).sum::<f32>() / n).sqrt();
    println!("dither on silence: {rms:.3} LSB RMS");
    assert!((0.40..0.60).contains(&rms), "one TPDF pass ≈ 0.5 LSB, two would be ≈ 0.7: {rms}");
    let (_, plain16, _) = bounce(&mut engine(None, None, 0.0), "nodith.wav", json!({"bit_depth": "16", "dither": false}));
    assert_eq!(peak(&plain16), 0.0);
}
