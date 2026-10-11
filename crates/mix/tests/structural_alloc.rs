//! B01 (next cycle, Sprint 1): structural session edits during playback — add/remove tracks and
//! buses, change routing, add/remove inserts and dedicated-slot providers — must not allocate or
//! free on the audio thread. The graph for the new session is prepared off the audio thread
//! (`GraphPrep`) and applied by moving existing strips across; leftovers return as garbage.
//!
//! The steady-state guard (`realtime_alloc.rs`) does not cover this: it renders one structure.

use soundcraft_audio_io::AudioBuffer;
use soundcraft_mix::{GraphPrep, MixEngine};
use soundcraft_model::{ChannelFormat, Clip, DedicatedSlot, Insert, Route, SendSlot, Session, SourceAudio, SourceId, TrackKind};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use std::alloc::System;
use std::sync::Arc;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const MB: usize = 512;

fn base() -> Session {
    let mut s = Session::default();
    for k in 0..4u64 {
        let t = s.add_track(TrackKind::Audio, if k % 2 == 0 { ChannelFormat::Mono } else { ChannelFormat::Stereo }, None);
        let ch = s.track(t).unwrap().channels();
        s.pool.insert(SourceId(k + 1), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![vec![0.1; 96_000]; ch] })));
        let cid = s.new_clip_id();
        s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(cid, "c", SourceId(k + 1), 0, 0, 96_000));
    }
    s.add_track(TrackKind::Master, ChannelFormat::Stereo, None);
    s
}

/// One structural edit applied to a session (what a user does while the transport runs).
type Edit = (&'static str, fn(&mut Session));

fn edits() -> Vec<Edit> {
    vec![
        ("add audio track", |s| {
            s.add_track(TrackKind::Audio, ChannelFormat::Stereo, Some("Dbl"));
        }),
        ("add bus + aux return and route a track to it", |s| {
            let b = s.add_bus("Vox", ChannelFormat::Stereo);
            let a = s.add_track(TrackKind::Aux, ChannelFormat::Stereo, Some("Vox"));
            s.track_mut(a).unwrap().mixer.input = Route::Bus(b);
            let t = s.tracks[0].id;
            s.track_mut(t).unwrap().mixer.output = Route::Bus(b);
        }),
        ("add a send", |s| {
            let b = s.busses[0].id;
            let t = s.tracks[1].id;
            s.track_mut(t).unwrap().mixer.sends[0] = Some(SendSlot::new(Route::Bus(b)));
        }),
        ("add an insert (test provider)", |s| {
            let t = s.tracks[1].id;
            s.track_mut(t).unwrap().mixer.inserts[0] = Some(Insert::new("test:gain:-3"));
        }),
        ("add a Tape slot provider", |s| {
            let t = s.tracks[2].id;
            s.track_mut(t).unwrap().mixer.slots.tape =
                Some(DedicatedSlot { id: 7, provider: Some(Insert::new("test:delay:32")), bypass: false, trim_db: 0.0 });
        }),
        ("remove the insert again", |s| {
            let t = s.tracks[1].id;
            s.track_mut(t).unwrap().mixer.inserts[0] = None;
        }),
        ("remove a track", |s| {
            let t = s.tracks[3].id;
            s.tracks.retain(|x| x.id != t);
        }),
        ("re-route the aux to Main and delete the bus", |s| {
            let b = s.busses[0].id;
            for t in s.tracks.iter_mut() {
                if t.mixer.output == Route::Bus(b) {
                    t.mixer.output = Route::Main;
                }
                if t.mixer.input == Route::Bus(b) {
                    t.mixer.input = Route::None;
                }
                t.mixer.sends[0] = None;
            }
            s.busses.retain(|x| x.id != b);
        }),
    ]
}

#[test]
fn structural_edits_do_not_allocate_on_the_audio_thread() {
    let mut s = base();
    let mut eng = MixEngine::new(48_000.0, MB);
    eng.parallel_strips = false;
    eng.set_external_instances(true);
    eng.set_session_changes_signalled(true);
    let mut out = vec![vec![0.0f32; 128]; 2];
    // First graph: prepared off-thread like every later one (allocation allowed here).
    let prep = GraphPrep::build(&s, 48_000.0, MB, Vec::new());
    let _garbage = eng.apply_prepared(&s, prep);
    for b in 0..20 {
        eng.render(&s, b * 128, 128, &mut out);
    }
    let mut pos = 20 * 128;
    let mut failures = Vec::new();
    for (name, edit) in edits() {
        edit(&mut s);
        // Off the audio thread: instances and resources for the new structure.
        let instances = soundcraft_mix::test_providers::prepare_instances(&s, 48_000.0, MB);
        let prep = GraphPrep::build(&s, 48_000.0, MB, instances);
        let region = Region::new(GLOBAL);
        // Audio thread: apply and keep rendering.
        let garbage = eng.apply_prepared(&s, prep);
        for _ in 0..4 {
            eng.render(&s, pos, 128, &mut out);
            pos += 128;
        }
        let st = region.change();
        println!("{name}: allocations {} reallocations {} deallocations {}", st.allocations, st.reallocations, st.deallocations);
        if st.allocations + st.reallocations + st.deallocations > 0 {
            failures.push(format!("{name}: {} alloc / {} realloc / {} dealloc", st.allocations, st.reallocations, st.deallocations));
        }
        drop(garbage); // the UI thread frees it
        assert!(!eng.prepared_fallbacks() > 0 || eng.prepared_fallbacks() == 0, "no fallback allocations");
    }
    assert!(failures.is_empty(), "structural edits allocated on the audio thread:\n  {}", failures.join("\n  "));
    assert_eq!(eng.prepared_fallbacks(), 0, "every resource came from the prepared bundle");
}
