//! Headless engine profile (Sprint 4, CL05/CL06 instrumentation; NOT a native measurement).
//!
//! Workload: 16 playback tracks (synthetic vocal-like audio), 2 record-armed tracks hearing a
//! live input buffer, a Vocal Bus (create-from-selection), an FX return, Main/Master, graph v2
//! (f64 sums), widths ≠ 100 %. For each callback size (64/128/256 frames at 48 kHz) it renders
//! 60 s of consecutive callbacks and records compute time per callback; load uses the frames
//! serviced as denominator (N/Fs). It also times engine command acknowledgment (UI event →
//! model updated) for 1000 fader moves. Writes CSVs to the paths given.
//!
//! `cargo run --release -p soundcraft-engine --example sml_profile -- callback.csv response.csv`
use serde_json::json;
use soundcraft_audio_io::{AudioBuffer, FileFormat};
use soundcraft_engine::Engine;
use soundcraft_model::{ChannelFormat, Clip, TrackKind};
use std::fmt::Write as _;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cb_path = args.first().cloned().unwrap_or_else(|| "callback-profile.csv".into());
    let rs_path = args.get(1).cloned().unwrap_or_else(|| "response-profile.csv".into());
    let mut e = Engine::default();
    let sr = 48_000u32;
    let len = sr as usize * 30;
    let mut ids = Vec::new();
    for k in 0..16u32 {
        let s = e.session_mut();
        let t = s.add_track(TrackKind::Audio, if k % 4 == 0 { ChannelFormat::Stereo } else { ChannelFormat::Mono }, Some(&format!("Track {k}")));
        let ch = if k % 4 == 0 { 2 } else { 1 };
        let data: Vec<Vec<f32>> =
            (0..ch).map(|c| (0..len).map(|i| ((i as f32 * (0.01 + k as f32 * 0.001 + c as f32 * 0.0003)).sin()) * 0.05).collect()).collect();
        let src = soundcraft_engine::io::add_source(s, &format!("t{k}"), AudioBuffer { sample_rate: sr, channels: data }, None, FileFormat::Wav);
        let id = s.new_clip_id();
        s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(id, "c", src, 0, 0, len as i64));
        ids.push(t);
    }
    for k in 0..2 {
        let s = e.session_mut();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some(&format!("Rec {k}")));
        s.track_mut(t).unwrap().mixer.record_arm = true;
    }
    let vocal: Vec<u64> = ids.iter().take(6).map(|t| t.0).collect();
    e.execute("bus.create_from_selection", &json!({"tracks": vocal, "name": "Vocal Bus"})).unwrap();
    e.execute("bus.create_fx_return", &json!({"tracks": [ids[0].0, ids[1].0], "name": "Verb"})).unwrap();
    e.session_mut().add_track(TrackKind::Master, ChannelFormat::Stereo, Some("Master"));
    for t in ids.iter().step_by(4) {
        e.execute("mix.width", &json!({"tracks": [t.0], "width": 1.3})).unwrap();
    }
    let session = e.session().clone();
    let mut csv =
        String::from("config,callback_frames,sample_rate,callbacks,budget_us,p50_us,p99_us,max_us,p99_load,max_load,deadline_misses,provenance\n");
    // B02 control: a thread that only keeps the callback cadence (spinning, no audio work) for
    // the same 60 s. Its lateness per period is pure scheduler preemption of this process; a
    // deadline miss here is a miss no DSP optimisation can remove. Compare with the rendering
    // configs below (same process, same minute).
    for n in [64usize, 128, 256] {
        let callbacks = (sr as usize * 60) / n;
        let period = std::time::Duration::from_secs_f64(n as f64 / f64::from(sr));
        let start = Instant::now();
        let mut late = Vec::with_capacity(callbacks);
        for k in 0..callbacks {
            let due = start + period * (k as u32 + 1);
            let mut now = Instant::now();
            while now < due {
                std::hint::spin_loop();
                now = Instant::now();
            }
            late.push((now - due).as_secs_f64() * 1e6);
        }
        late.sort_by(f64::total_cmp);
        let budget = n as f64 / f64::from(sr) * 1e6;
        let pct = |p: f64| late[((late.len() as f64 - 1.0) * p) as usize];
        let misses = late.iter().filter(|t| **t >= budget).count();
        let _ = writeln!(
            csv,
            "no-work control (paced spin; lateness = scheduler preemption only),{n},{sr},{callbacks},{budget:.1},{:.1},{:.1},{:.1},{:.3},{:.3},{misses},headless container release build (not native; no device; single process)",
            pct(0.5),
            pct(0.99),
            late[late.len() - 1],
            pct(0.99) / budget,
            late[late.len() - 1] / budget
        );
    }
    // Configs: playback's (adaptive strips, announced session changes), the same with the
    // per-block structure check (offline default), and forced-serial strips.
    for (n, parallel, signalled) in [64usize, 128, 256].into_iter().flat_map(|n| [(n, true, false), (n, false, false), (n, true, true)]) {
        let mut eng = soundcraft_mix::MixEngine::new(sr as f32, 512);
        eng.parallel_strips = parallel;
        eng.set_session_changes_signalled(signalled);
        eng.recording = true;
        eng.input = vec![vec![0.01; 512]];
        let mut out = vec![vec![0.0f32; n]; 2];
        let callbacks = (sr as usize * 60) / n;
        let mut times = Vec::with_capacity(callbacks);
        let mut pos = 0i64;
        for _ in 0..callbacks {
            let t0 = Instant::now();
            eng.render(&session, pos % (len as i64 - 1024), n, &mut out);
            times.push(t0.elapsed().as_secs_f64() * 1e6);
            pos += n as i64;
        }
        times.sort_by(f64::total_cmp);
        let budget = n as f64 / f64::from(sr) * 1e6;
        let pct = |p: f64| times[((times.len() as f64 - 1.0) * p) as usize];
        let misses = times.iter().filter(|t| **t >= budget).count();
        let _ = writeln!(
            csv,
            "16play+2rec+bus+fx v2 {},{n},{sr},{callbacks},{budget:.1},{:.1},{:.1},{:.1},{:.3},{:.3},{misses},headless container release build (not native; no device; single process)",
            match (parallel, signalled) {
                (true, true) => "adaptive strips + announced changes (playback config)",
                (true, false) => "adaptive strips + per-block structure check",
                _ => "serial strips + per-block structure check",
            },
            pct(0.5),
            pct(0.99),
            times[times.len() - 1],
            pct(0.99) / budget,
            times[times.len() - 1] / budget
        );
    }
    std::fs::write(&cb_path, &csv).unwrap();
    // Event → model acknowledgment for fader moves (the part both mixer views share).
    let mut ack = Vec::new();
    for i in 0..1000 {
        let t0 = Instant::now();
        e.execute_merged("mix.volume", &json!({"track": ids[i % 16].0, "db": -((i % 30) as f64)}), "profile").unwrap();
        ack.push(t0.elapsed().as_secs_f64() * 1e6);
    }
    e.end_merge();
    ack.sort_by(f64::total_cmp);
    let r = format!(
        "metric,samples,p50_us,p95_us,max_us,provenance\nfader command ack (event->model revision),{},{:.1},{:.1},{:.1},headless engine only; excludes UI frame/render and audible onset\n",
        ack.len(),
        ack[ack.len() / 2],
        ack[(ack.len() as f64 * 0.95) as usize],
        ack[ack.len() - 1]
    );
    std::fs::write(&rs_path, r).unwrap();
    print!("{csv}");
}
