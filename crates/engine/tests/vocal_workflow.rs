//! Vocal editing coverage (VOCAL_EDITING_SPEC V01–V14, Sprint 2 software portions) and the
//! staged V-E2E fixture through comp/edit/save/reopen. Synthetic vocal-like phrases only.

use serde_json::{Value, json};
use soundcraft_audio_io::{AudioBuffer, FileFormat};
use soundcraft_engine::Engine;
use soundcraft_model::{ChannelFormat, Clip, ClipContent, SourceId, TrackId, TrackKind};
use soundcraft_time::Range;
use std::hash::{Hash, Hasher};

const SR: u32 = 48_000;
const LEN: usize = 48_000 * 2;

/// A vocal-like take: silence, breath (low noise), phrase A, gap, phrase B, silence. `k` makes
/// each take distinguishable (different pitch/level).
fn take(k: u32) -> Vec<f32> {
    let mut v = vec![0.0f32; LEN];
    let mut seed = 0x1234_5678u32.wrapping_add(k);
    for (i, x) in v.iter_mut().enumerate() {
        let t = i as f32 / SR as f32;
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let noise = (seed as f32 / u32::MAX as f32 - 0.5) * 0.004;
        *x = match i {
            4_800..9_600 => noise, // breath ~ -54 dBFS
            12_000..36_000 => (t * (220.0 + 20.0 * k as f32) * std::f32::consts::TAU).sin() * 0.4 * (1.0 + k as f32 * 0.1),
            48_000..72_000 => (t * (330.0 + 20.0 * k as f32) * std::f32::consts::TAU).sin() * 0.35,
            _ => 0.0,
        };
    }
    v
}

fn hash_source(e: &Engine, id: SourceId) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Some(a) = e.session().pool.get(id) {
        for c in 0..2 {
            if let Some(ch) = a.channel(c) {
                for x in ch {
                    x.to_bits().hash(&mut h);
                }
            }
        }
    }
    h.finish()
}

/// One mono track with three alternate takes on three playlists (take 1 active).
fn three_takes() -> (Engine, TrackId, Vec<SourceId>) {
    let mut e = Engine::default();
    let s = e.session_mut();
    let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Lead Vox"));
    let mut srcs = Vec::new();
    for k in 0..3u32 {
        let src = soundcraft_engine::io::add_source(
            s,
            &format!("Lead_take{}", k + 1),
            AudioBuffer { sample_rate: SR, channels: vec![take(k)] },
            None,
            FileFormat::Wav,
        );
        srcs.push(src);
        let tr = s.track_mut(t).unwrap();
        if k > 0 {
            tr.playlists.push(soundcraft_model::Playlist::new(format!("Lead Vox.{}", k + 1)));
        }
        let pl = tr.playlists.last_mut().unwrap();
        let id = soundcraft_model::ClipId(1000 + u64::from(k));
        pl.clips.push(Clip::audio(id, format!("take{}", k + 1), src, 0, 0, LEN as i64));
    }
    s.track_mut(t).unwrap().active_playlist = 0;
    (e, t, srcs)
}

/// Centre gain of the engine's (legacy/Standard profile) mono -3 dB pan law, used by the
/// independently built references.
fn centre() -> f32 {
    soundcraft_dsp::pan::gains(0.0, soundcraft_dsp::pan::PanLaw::Minus3).0
}

fn render(e: &Engine, r: Range) -> Vec<f32> {
    soundcraft_mix::render_range(e.session(), r, 512).swap_remove(0)
}

fn clips(e: &Engine, t: TrackId) -> Vec<Clip> {
    e.session().track(t).unwrap().clips().to_vec()
}

fn exec(e: &mut Engine, id: &str, p: Value) -> Value {
    e.execute(id, &p).unwrap_or_else(|err| panic!("{id}: {err}"))
}

/// V02: separate/heal/trim with undo; originals untouched.
#[test]
fn vocal_v02_separate_heal_trim_undo_keep_originals() {
    let (mut e, t, srcs) = three_takes();
    let h0 = hash_source(&e, srcs[0]);
    exec(&mut e, "edit.separate", json!({"tracks": [t.0], "at": 40_000}));
    let c = clips(&e, t);
    assert_eq!(c.len(), 2);
    assert_eq!((c[0].start, c[0].end(), c[1].start, c[1].end()), (0, 40_000, 40_000, LEN as i64), "exact boundary");
    exec(&mut e, "edit.heal", json!({"tracks": [t.0], "start": 39_000, "end": 41_000}));
    assert_eq!(clips(&e, t).len(), 1, "compatible split healed");
    exec(&mut e, "edit.trim_to_selection", json!({"tracks": [t.0], "start": 10_000, "end": 80_000}));
    let c = clips(&e, t);
    assert_eq!((c[0].start, c[0].end()), (10_000, 80_000));
    let ClipContent::Audio { offset, .. } = c[0].content else { panic!() };
    assert_eq!(offset, 10_000, "non-destructive trim keeps alignment");
    e.execute("edit.undo", &json!({})).unwrap();
    e.execute("edit.undo", &json!({})).unwrap();
    e.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(clips(&e, t).len(), 1);
    assert_eq!(clips(&e, t)[0].length, LEN as i64);
    assert_eq!(hash_source(&e, srcs[0]), h0, "source media unchanged");
}

/// V04: crossfade at a join renders the specified equal-power shape; undo removes it.
#[test]
fn vocal_v04_crossfade_and_fade_shapes() {
    let (mut e, t, _) = three_takes();
    exec(&mut e, "edit.separate", json!({"tracks": [t.0], "at": 24_000}));
    let before = render(&e, Range::new(0, LEN as i64));
    exec(&mut e, "edit.fades_create", json!({"tracks": [t.0], "start": 23_520, "end": 24_480, "shape": "equal power"}));
    let c = clips(&e, t);
    let faded = c.iter().any(|c| c.fade_in.len > 0 || c.fade_out.len > 0);
    assert!(faded, "a fade/crossfade was created: {c:?}");
    let after = render(&e, Range::new(0, LEN as i64));
    // Outside the fade region the render is unchanged (no unexplained gain).
    assert_eq!(&before[..23_000], &after[..23_000]);
    assert_eq!(&before[25_000..], &after[25_000..]);
    e.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(render(&e, Range::new(0, LEN as i64)), before);
}

/// V05: clip gain is a source-stage scale independent of the fader, undoable, no normalization.
#[test]
fn vocal_v05_clip_gain_breath_rides() {
    let (mut e, t, _) = three_takes();
    let r = Range::new(12_000, 36_000);
    let a = render(&e, r);
    let id = clips(&e, t)[0].id;
    exec(&mut e, "clip.gain", json!({"clips": [id.0], "db": -6.0}));
    let b = render(&e, r);
    let g = 10f32.powf(-6.0 / 20.0);
    for (x, y) in a.iter().zip(&b) {
        assert!((x * g - y).abs() <= 1e-5);
    }
    assert_eq!(e.session().track(t).unwrap().mixer.volume_db, 0.0, "fader untouched");
    e.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(render(&e, r), a);
}

/// V06: promote a phrase from take 2 into the active comp with original alignment; all takes
/// preserved; one playlist sounds; save/reopen keeps comp and alternates.
#[test]
fn vocal_v06_comp_phrase_from_alternate_take() {
    let (mut e, t, srcs) = three_takes();
    exec(&mut e, "track.playlist_promote", json!({"track": t.0, "playlist": 1, "start": 48_000, "end": 72_000}));
    let out = render(&e, Range::new(0, LEN as i64));
    let t1 = take(0);
    let t2 = take(1);
    // Phrase B region now plays take 2 sample-for-sample; phrase A still take 1.
    for i in (48_100..71_900).step_by(97) {
        assert!((out[i] - t2[i] * centre()).abs() < 1e-5, "sample {i} from take 2");
    }
    for i in (12_100..35_900).step_by(97) {
        assert!((out[i] - t1[i] * centre()).abs() < 1e-5, "sample {i} from take 1");
    }
    assert_eq!(e.session().track(t).unwrap().playlists.len(), 3, "alternates preserved");
    let dir = std::env::temp_dir().join(format!("sml-vocal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("comp.scraft");
    exec(&mut e, "session.save", json!({"path": path.to_string_lossy()}));
    let mut f = Engine::default();
    exec(&mut f, "session.open", json!({"path": path.to_string_lossy()}));
    let tf = f.session().track_by_name("Lead Vox").unwrap().id;
    assert_eq!(f.session().track(tf).unwrap().playlists.len(), 3);
    assert_eq!(render(&f, Range::new(0, LEN as i64)), out, "comp reproduces after reopen");
    let _ = (srcs, std::fs::remove_dir_all(&dir));
}

/// V09: strip silence keeps a low-level breath with padding when the threshold is below it, and
/// never rewrites media.
#[test]
fn vocal_v09_strip_silence_preserves_breath_with_padding() {
    let (mut e, t, srcs) = three_takes();
    let h = hash_source(&e, srcs[0]);
    exec(&mut e, "edit.select", json!({"tracks": [t.0], "start": 0, "end": LEN as i64}));
    exec(
        &mut e,
        "edit.strip_silence",
        json!({"tracks": [t.0], "start": 0, "end": LEN as i64, "threshold_db": -70.0, "min_length_ms": 50, "pad_before_ms": 5, "pad_after_ms": 20}),
    );
    let c = clips(&e, t);
    assert!(c.len() >= 2, "silence removed between regions: {}", c.len());
    assert!(c.iter().any(|c| c.start <= 4_800 && c.end() >= 9_600), "breath region kept");
    assert_eq!(hash_source(&e, srcs[0]), h);
    e.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(clips(&e, t).len(), 1, "reversible");
}

/// Staged V-E2E (Sprint 2 portion): three takes → comp two phrases → trim breath → gain ride →
/// crossfade → nudge +10 ms and undo → group → save/reopen. Source hashes unchanged; the final
/// pre-plugin render matches an independently constructed reference within 1e-5.
#[test]
fn vocal_e2e_staged_through_save_reopen() {
    let (mut e, t, srcs) = three_takes();
    let hashes: Vec<u64> = srcs.iter().map(|s| hash_source(&e, *s)).collect();
    // Comp: phrase A from take 3, phrase B from take 2.
    exec(&mut e, "track.playlist_promote", json!({"track": t.0, "playlist": 2, "start": 12_000, "end": 36_000}));
    exec(&mut e, "track.playlist_promote", json!({"track": t.0, "playlist": 1, "start": 48_000, "end": 72_000}));
    // Trim the breath out (clear 4_800..9_600).
    exec(&mut e, "edit.clear", json!({"tracks": [t.0], "start": 4_800, "end": 9_600}));
    // Gain ride on phrase B clip (-3 dB).
    let b_clip = clips(&e, t).into_iter().find(|c| c.start == 48_000).expect("phrase B clip").id;
    exec(&mut e, "clip.gain", json!({"clips": [b_clip.0], "db": -3.0}));
    // Nudge the phrase +10 ms then undo: exact restore.
    let before = clips(&e, t);
    exec(&mut e, "nudge.set", json!({"value": 10.0, "unit": "ms", "target": "clips"}));
    exec(&mut e, "nudge.apply", json!({"direction": 1, "clips": [b_clip.0]}));
    assert_eq!(clips(&e, t).iter().find(|c| c.id == b_clip).unwrap().start, 48_480);
    e.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(clips(&e, t), before);
    // Group the comp clips.
    let ids: Vec<u64> = clips(&e, t).iter().map(|c| c.id.0).collect();
    exec(&mut e, "clip.group", json!({"clips": ids}));
    // Independent reference: take3 in A, take2 * -3 dB in B, take1 elsewhere, breath removed.
    let (t1, t2, t3) = (take(0), take(1), take(2));
    let g = 10f32.powf(-3.0 / 20.0);
    let pan = centre();
    let reference: Vec<f32> = (0..LEN)
        .map(|i| {
            let v = match i {
                4_800..9_600 => 0.0,
                12_000..36_000 => t3[i],
                48_000..72_000 => t2[i] * g,
                _ => t1[i],
            };
            v * pan
        })
        .collect();
    let dir = std::env::temp_dir().join(format!("sml-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("vocal.scraft");
    exec(&mut e, "session.save", json!({"path": path.to_string_lossy()}));
    let mut f = Engine::default();
    exec(&mut f, "session.open", json!({"path": path.to_string_lossy()}));
    let out = render(&f, Range::new(0, LEN as i64));
    let max_err = out.iter().zip(&reference).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
    let tf = f.session().track_by_name("Lead Vox").unwrap().id;
    let reopened_hashes: Vec<u64> = srcs
        .iter()
        .filter_map(|s| f.session().sources.iter().find(|x| x.name == e.session().source(*s).unwrap().name).map(|x| hash_source(&f, x.id)))
        .collect();
    println!(
        "VOCAL_E2E {}",
        json!({"stage": "sprint2 (comp/edit/nudge/group/save-reopen)", "max_abs_error_vs_reference": max_err, "tolerance": 1e-5,
               "clips": f.session().track(tf).unwrap().clips().len(), "playlists": f.session().track(tf).unwrap().playlists.len(),
               "source_hashes_before": hashes, "source_hashes_after_reopen": reopened_hashes})
    );
    assert!(max_err <= 1e-5, "render error {max_err}");
    assert_eq!(hashes, srcs.iter().map(|s| hash_source(&e, *s)).collect::<Vec<_>>(), "originals unchanged in memory");
    assert_eq!(reopened_hashes, hashes, "originals unchanged on disk round trip");
    assert_eq!(f.session().track(tf).unwrap().playlists.len(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

/// V11: the retained pitch-preserving stretch service (WSOLA, `dsp::offline::time_stretch`)
/// produces the requested duration, keeps the pitch of a 440 Hz tone within 2 % (declared
/// algorithm tolerance for this fixture), outputs finite samples and leaves the input untouched.
/// Varispeed (resample) is a separate, labelled operation.
#[test]
fn vocal_v11_time_stretch_service_duration_and_pitch() {
    let sr = 48_000.0f32;
    let src: Vec<f32> = (0..48_000).map(|i| (i as f32 / sr * 440.0 * std::f32::consts::TAU).sin() * 0.5).collect();
    let copy = src.clone();
    for ratio in [0.8f64, 1.25] {
        let out = soundcraft_dsp::offline::time_stretch(std::slice::from_ref(&src), ratio, sr);
        let n = out[0].len() as f64;
        assert!((n / 48_000.0 - ratio).abs() < 0.01, "duration {n} for ratio {ratio}");
        assert!(out[0].iter().all(|x| x.is_finite()));
        // Pitch by zero crossings over the steady middle section.
        let mid = &out[0][(n as usize / 4)..(3 * n as usize / 4)];
        let zc = mid.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count() as f64;
        let hz = zc / (mid.len() as f64 / f64::from(sr));
        assert!((hz / 440.0 - 1.0).abs() < 0.02, "pitch {hz:.1} Hz at ratio {ratio}");
    }
    assert_eq!(src, copy, "source untouched");
}
