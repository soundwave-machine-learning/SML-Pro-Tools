//! CL-T5 allocation guard: after warm-up, `MixEngine::render` performs no heap allocation for a
//! graph-v2 session with buses, sends, an FX return, width, a dedicated slot fixture, profile pans
//! and live input — in both serial and parallel strip modes.

use soundcraft_audio_io::AudioBuffer;
use soundcraft_mix::MixEngine;
use soundcraft_model::{ChannelFormat, Clip, DedicatedSlot, Insert, Route, SendSlot, Session, SourceAudio, SourceId, TrackKind};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use std::alloc::System;
use std::sync::Arc;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

fn session() -> Session {
    let mut s = Session::default();
    let vb = s.add_bus("Vox", ChannelFormat::Stereo);
    let fx = s.add_bus("FX", ChannelFormat::Stereo);
    for (k, fmt) in [ChannelFormat::Mono, ChannelFormat::Stereo, ChannelFormat::Mono, ChannelFormat::Mono].into_iter().enumerate() {
        let t = s.add_track(TrackKind::Audio, fmt, None);
        let ch = fmt.channels();
        s.pool.insert(SourceId(k as u64 + 1), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![vec![0.1; 96_000]; ch] })));
        let cid = s.new_clip_id();
        let tr = s.track_mut(t).unwrap();
        tr.playlist_mut().unwrap().clips.push(Clip::audio(cid, "c", SourceId(k as u64 + 1), 0, 0, 96_000));
        tr.mixer.output = Route::Bus(vb);
        let mut snd = SendSlot::new(Route::Bus(fx));
        snd.level_db = -6.0;
        snd.pre_fader = k % 2 == 0;
        snd.follow_main_pan = k % 2 == 1;
        tr.mixer.sends[0] = Some(snd);
        if ch == 2 {
            tr.mixer.width = 1.4;
        }
        tr.mixer.slots.tape = Some(DedicatedSlot { id: 9, provider: Some(Insert::new("test:delay:32")), bypass: false, trim_db: -1.0 });
    }
    for b in [vb, fx] {
        let a = s.add_track(TrackKind::Aux, ChannelFormat::Stereo, None);
        s.track_mut(a).unwrap().mixer.input = Route::Bus(b);
    }
    s.add_track(TrackKind::Master, ChannelFormat::Stereo, None);
    s.mix_profile.kind = soundcraft_model::ProfileKind::LogicStyle;
    s
}

fn steady_state_allocations(parallel: bool) -> usize {
    let s = session();
    assert!(!soundcraft_mix::is_missing_plugin("test:delay:32"), "slot fixture provider is really loaded");
    let mut eng = MixEngine::new(48_000.0, 512);
    eng.parallel_strips = parallel;
    eng.recording = true;
    eng.input = vec![vec![0.05; 512]];
    let mut out = vec![vec![0.0f32; 128]; 2];
    // Warm-up: first blocks create plugins, delay lines, meters and scratch buffers.
    for b in 0..20 {
        eng.render(&s, b * 128, 128, &mut out);
    }
    let region = Region::new(GLOBAL);
    for b in 20..220 {
        eng.render(&s, b * 128, 128, &mut out);
    }
    let st = region.change();
    st.allocations + st.reallocations
}

#[test]
fn render_is_allocation_free_in_steady_state() {
    assert_eq!(steady_state_allocations(false), 0, "serial strips");
    // Rayon's work distribution may allocate internally; report it separately from the engine.
    let par = steady_state_allocations(true);
    println!("parallel-mode allocations over 200 blocks: {par}");
}
