//! B01 (next cycle, Sprint 1): structural session edits during playback — add/remove tracks and
//! buses, change routing, add/remove inserts and dedicated-slot providers, bypass a latent
//! insert, route a sidechain, reorder tracks, replace the whole session — must not allocate or
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

/// A different song: other track count and formats, a bus with an aux return, a send and inserts.
fn other() -> Session {
    let mut s = Session::default();
    let b = s.add_bus("Verb", ChannelFormat::Stereo);
    for k in 0..6u64 {
        let t = s.add_track(TrackKind::Audio, if k % 3 == 0 { ChannelFormat::Mono } else { ChannelFormat::Stereo }, None);
        let ch = s.track(t).unwrap().channels();
        s.pool.insert(SourceId(100 + k), Arc::new(SourceAudio::new(AudioBuffer { sample_rate: 48_000, channels: vec![vec![0.05; 96_000]; ch] })));
        let cid = s.new_clip_id();
        let tr = s.track_mut(t).unwrap();
        tr.playlist_mut().unwrap().clips.push(Clip::audio(cid, "o", SourceId(100 + k), 0, 0, 96_000));
        tr.mixer.sends[0] = Some(SendSlot::new(Route::Bus(b)));
        tr.mixer.inserts[0] = Some(Insert::new(if k % 2 == 0 { "test:gain:-2" } else { "test:delay:16" }));
    }
    let a = s.add_track(TrackKind::Aux, ChannelFormat::Stereo, Some("Verb"));
    s.track_mut(a).unwrap().mixer.input = Route::Bus(b);
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
        ("add a latent insert (64 samples)", |s| {
            let t = s.tracks[0].id;
            s.track_mut(t).unwrap().mixer.inserts[1] = Some(Insert::new("test:delay:64"));
        }),
        ("bypass the latent insert (its delay is kept)", |s| {
            let t = s.tracks[0].id;
            s.track_mut(t).unwrap().mixer.inserts[1].as_mut().unwrap().bypass = true;
        }),
        ("route the bus into a gate's sidechain", |s| {
            let b = s.busses[0].id;
            let t = s.tracks[2].id;
            let mut gate = Insert::new("test:sidegate:-60");
            gate.sidechain = Some(b);
            s.track_mut(t).unwrap().mixer.inserts[1] = Some(gate);
        }),
        ("reorder tracks", |s| {
            s.tracks.swap(0, 2);
            let n = s.tracks.len();
            s.tracks.swap(1, n - 1);
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
        ("replace the whole session (open another song)", |s| {
            *s = other();
        }),
    ]
}

#[test]
fn structural_edits_do_not_allocate_on_the_audio_thread() {
    let mut s = base();
    soundcraft_mix::mark_audio_thread();
    soundcraft_mix::reset_prepared_fallbacks();
    let mut eng = MixEngine::new(48_000.0, MB);
    eng.parallel_strips = false;
    eng.set_external_instances(true);
    eng.set_session_changes_signalled(true);
    // Reference: the offline engine (per-block structure check, allocating sync) must produce
    // the identical signal for the same session sequence.
    let mut reference = MixEngine::new(48_000.0, MB);
    reference.parallel_strips = false;
    let mut out = vec![vec![0.0f32; 128]; 2];
    let mut out_ref = vec![vec![0.0f32; 128]; 2];
    // First graph: prepared off-thread like every later one (allocation allowed here).
    let prep = GraphPrep::build(&s, 48_000.0, MB, soundcraft_mix::prepare_instances(&s, 48_000.0, MB), 0);
    let _garbage = eng.apply_prepared(&s, prep);
    for b in 0..20 {
        eng.render(&s, b * 128, 128, &mut out);
        reference.render(&s, b * 128, 128, &mut out_ref);
        assert_eq!(out, out_ref, "initial prepared graph renders identically to the offline engine");
    }
    assert!(out[0].iter().any(|x| *x != 0.0), "the fixture is audible");
    let mut pos = 20 * 128;
    let mut failures = Vec::new();
    let mut prev_tracks = s.tracks.len();
    for (name, edit) in edits() {
        edit(&mut s);
        // Off the audio thread: instances and resources for the new structure.
        let instances = soundcraft_mix::prepare_instances(&s, 48_000.0, MB);
        let prep = GraphPrep::build(&s, 48_000.0, MB, instances, prev_tracks);
        prev_tracks = s.tracks.len();
        // The fallback counter is global and the reference engine (allocating by design) runs
        // on this thread too: count only around the prepared engine's calls.
        let mut fb = 0;
        let fb0 = soundcraft_mix::prepared_fallbacks();
        let region = Region::new(GLOBAL);
        // Audio thread: apply and keep rendering.
        let garbage = eng.apply_prepared(&s, prep);
        let apply = region.change();
        fb += soundcraft_mix::prepared_fallbacks() - fb0;
        let mut blocks = Vec::new();
        for _ in 0..4 {
            let fb1 = soundcraft_mix::prepared_fallbacks();
            let r = Region::new(GLOBAL);
            eng.render(&s, pos, 128, &mut out);
            let st = r.change();
            fb += soundcraft_mix::prepared_fallbacks() - fb1;
            blocks.push((st.allocations, st.reallocations, st.deallocations));
            reference.render(&s, pos, 128, &mut out_ref);
            assert_eq!(out, out_ref, "{name}: prepared graph renders identically to the offline engine");
            pos += 128;
        }
        println!(
            "{name}: apply {}/{}/{} (alloc/realloc/dealloc), blocks {:?}, counted fallbacks {fb}",
            apply.allocations, apply.reallocations, apply.deallocations, blocks
        );
        let total: usize = apply.allocations + apply.reallocations + apply.deallocations + blocks.iter().map(|b| b.0 + b.1 + b.2).sum::<usize>();
        if total > 0 || fb > 0 {
            failures.push(format!(
                "{name}: apply {:?}, blocks {:?}, fallbacks {fb}",
                (apply.allocations, apply.reallocations, apply.deallocations),
                blocks
            ));
        }
        drop(garbage); // the UI thread frees it
    }
    assert!(failures.is_empty(), "structural edits allocated on the audio thread:\n  {}", failures.join("\n  "));
}
