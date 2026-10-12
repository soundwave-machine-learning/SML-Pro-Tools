//! Import, export, save/load, rendering helpers (bounce, consolidate, commit) and analysis.

use crate::{Engine, EngineError, Result};
use serde_json::{Value, json};
use soundcraft_audio_io::{AudioBuffer, EncodeOptions, FileFormat};
use soundcraft_model::{ChannelFormat, Clip, ClipContent, Session, Source, SourceAudio, SourceId, TrackId, TrackKind};
use soundcraft_time::{Range, Samples};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Largest import we accept (samples per channel × channels), ~4 GiB of f32.
const MAX_IMPORT_SAMPLES: usize = 1 << 30;

/// Register decoded audio as a new source. Resamples to the session rate if needed.
pub fn add_source(s: &mut Session, name: &str, mut buf: AudioBuffer, path: Option<&str>, format: FileFormat) -> SourceId {
    let sr = s.sample_rate.hz();
    if buf.sample_rate != sr && buf.sample_rate > 0 {
        buf.channels = soundcraft_dsp::offline::resample(&buf.channels, buf.sample_rate, sr);
        buf.sample_rate = sr;
    }
    let id = SourceId(s.alloc());
    let stem = Path::new(name).file_stem().and_then(|x| x.to_str()).unwrap_or(name).to_string();
    s.sources.push(Source {
        id,
        name: stem.clone(),
        path: path.map_or_else(|| format!("Audio Files/{stem}.wav"), str::to_string),
        channels: u16::try_from(buf.num_channels()).unwrap_or(u16::MAX),
        frames: buf.frames() as u64,
        sample_rate: sr,
        format,
        time_reference: 0,
        unsaved: true,
    });
    s.pool.insert(id, Arc::new(SourceAudio::new(buf)));
    id
}

/// Decode and place an audio file. With no target track, a new track of matching width is made.
pub fn import_audio_bytes(e: &mut Engine, name: &str, bytes: &[u8], path: Option<&str>, target: Option<TrackId>, at: Samples) -> Result<Value> {
    let ext = Path::new(name).extension().and_then(|x| x.to_str()).map(str::to_ascii_lowercase);
    let (info, buf) = soundcraft_audio_io::decode(bytes, ext.as_deref()).map_err(|err| EngineError::Io(format!("{name}: {err}")))?;
    if buf.frames().saturating_mul(buf.num_channels().max(1)) > MAX_IMPORT_SAMPLES {
        return Err(EngineError::Io(format!("{name}: file is too large to import")));
    }
    if buf.frames() == 0 {
        return Err(EngineError::Io(format!("{name}: no audio")));
    }
    let channels = buf.num_channels();
    let s = e.session_mut();
    let src = add_source(s, name, buf, path, info.format);
    if let (Some(bwf), Some(sx)) = (info.bwf.as_ref(), s.sources.last_mut()) {
        sx.time_reference = bwf.time_reference;
    }
    let frames = s.source(src).map_or(0, |x| i64::try_from(x.frames).unwrap_or(0));
    let track = match target {
        Some(t) => t,
        None => {
            let stem = Path::new(name).file_stem().and_then(|x| x.to_str()).unwrap_or("Audio").to_string();
            s.add_track(TrackKind::Audio, ChannelFormat::for_channels(channels.min(2)), Some(&stem))
        }
    };
    let cid = s.new_clip_id();
    let stem = s.source(src).map(|x| x.name.clone()).unwrap_or_default();
    crate::edit::place_clip(s, track, Clip::audio(cid, stem, src, 0, at, frames));
    Ok(json!({"track": track, "clip": cid, "source": src, "frames": frames, "channels": channels}))
}

/// Import a movie: a Video track clip spanning the picture (the movie is referenced by `path`,
/// never copied) plus its audio on a new audio track when it has a usable audio stream. Fails
/// only when the file has neither a readable picture nor audio.
pub fn import_video_bytes(e: &mut Engine, path: &str, bytes: Vec<u8>, at: Samples) -> Result<Value> {
    let stem = Path::new(path).file_stem().and_then(|n| n.to_str()).unwrap_or("Video").to_string();
    let ext = Path::new(path).extension().and_then(|x| x.to_str()).unwrap_or("mp4").to_ascii_lowercase();
    let bytes: Arc<[u8]> = bytes.into();
    let movie = soundcraft_video::Movie::open(Arc::clone(&bytes)).map(|m| m.info().clone());
    let audio_name = format!("{stem} audio.{}", if ext == "mov" { "mp4" } else { ext.as_str() });
    let audio = import_audio_bytes(e, &audio_name, &bytes, Some(path), None, at);
    let info = match (movie, &audio) {
        (Ok(info), _) => info,
        (Err(verr), Ok(a)) => {
            e.message(format!("{stem}: imported the audio; no picture ({verr})"));
            return Ok(json!({"audio": a, "video": Value::Null, "video_error": verr.to_string()}));
        }
        (Err(verr), Err(aerr)) => return Err(EngineError::Io(format!("{path}: {verr}; no usable audio stream ({aerr})"))),
    };
    let abs = std::fs::canonicalize(path).map_or_else(|_| path.to_string(), |p| p.to_string_lossy().into_owned());
    let s = e.session_mut();
    let sr = s.sample_rate.as_f64();
    let len = (info.duration * sr).round();
    let len = if len.is_finite() { (len as Samples).clamp(1, sr as Samples * 3600 * 48) } else { 1 };
    let vid = SourceId(s.alloc());
    s.videos.push(soundcraft_model::VideoSource {
        id: vid,
        name: Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or("movie").to_string(),
        path: abs,
        width: info.width,
        height: info.height,
        frame_rate: info.frame_rate,
        duration: info.duration,
        codec: info.codec.clone(),
    });
    // Reuse the first Video track when the new clip does not overlap its clips; else add one at the top.
    let range = Range::new(at, at.saturating_add(len));
    let track = match s.tracks.iter().find(|t| t.kind == TrackKind::Video) {
        Some(t) if !t.clips().iter().any(|c| c.range().overlaps(&range)) => t.id,
        _ => {
            let id = s.add_track(TrackKind::Video, ChannelFormat::Mono, Some(&stem));
            if let Some(i) = s.track_index(id) {
                let t = s.tracks.remove(i);
                s.tracks.insert(0, t);
            }
            id
        }
    };
    let cid = s.new_clip_id();
    crate::edit::place_clip(s, track, Clip::video(cid, stem.clone(), vid, 0, at, len));
    if !info.decodable {
        e.message(format!("{stem}: {} pictures cannot be shown yet; the Video track shows the clip only", info.codec_detail));
    }
    if let Err(err) = &audio {
        e.message(format!("{stem}: no usable audio stream ({err})"));
    }
    Ok(json!({
        "video": {"track": track, "clip": cid, "source": vid, "length": len, "info": info},
        "audio": audio.as_ref().ok(),
        "audio_error": audio.as_ref().err().map(ToString::to_string),
    }))
}

/// Resolve each referenced movie: keep it when its path exists, else look for a file of the same
/// name next to the session (moved projects) and relink. Returns the movies still missing.
pub fn relink_videos(s: &mut Session, session_dir: &Path) -> Vec<String> {
    let mut missing = Vec::new();
    for v in &mut s.videos {
        let p = if Path::new(&v.path).is_absolute() { PathBuf::from(&v.path) } else { session_dir.join(&v.path) };
        if p.is_file() {
            continue;
        }
        let name = Path::new(&v.path).file_name().map(PathBuf::from).unwrap_or_default();
        let near =
            [session_dir.join(&name), session_dir.join("Video Files").join(&name)].into_iter().find(|c| !name.as_os_str().is_empty() && c.is_file());
        match near {
            Some(c) => v.path = c.to_string_lossy().into_owned(),
            None => missing.push(format!("{} (video)", p.display())),
        }
    }
    missing
}

/// Import a Standard MIDI File as MIDI tracks (one per SMF track).
pub fn import_midi_bytes(e: &mut Engine, bytes: &[u8], at: Samples, tempo_map: bool) -> Result<Vec<TrackId>> {
    let smf = soundcraft_midi::read_smf(bytes).map_err(|err| EngineError::Io(err.to_string()))?;
    let s = e.session_mut();
    if tempo_map {
        for (tick, bpm) in &smf.tempos {
            let _ = s.tempo.set_tempo(*tick, *bpm);
        }
        for (tick, n, d) in &smf.meters {
            let _ = s.tempo.set_meter(*tick, u32::from(*n), u32::from(*d));
        }
    }
    let base_tick = s.tempo.samples_to_ticks(at, s.sample_rate);
    let mut out = Vec::new();
    for (i, tr) in smf.tracks.iter().enumerate() {
        if tr.sequence.notes.is_empty() {
            continue;
        }
        let name = if tr.name.trim().is_empty() { format!("MIDI {}", i + 1) } else { tr.name.clone() };
        let id = s.add_track(TrackKind::Instrument, ChannelFormat::Stereo, Some(&name));
        // No bundled instrument is assigned (SML Recording ships none); the user picks one.
        let end_tick = base_tick + tr.sequence.end_tick().max(1);
        let len = s.tempo.tick_to_samples(end_tick, s.sample_rate) - at;
        let cid = s.new_clip_id();
        crate::edit::place_clip(s, id, Clip::midi(cid, name, at, len.max(1), tr.sequence.clone()));
        out.push(id);
    }
    Ok(out)
}

/// Export all MIDI clips as a format-1 SMF.
pub fn export_midi(e: &Engine) -> Result<Vec<u8>> {
    let s = e.session();
    let mut smf = soundcraft_midi::Smf {
        format: 1,
        tracks: Vec::new(),
        tempos: s.tempo.tempos().iter().map(|t| (t.tick, t.bpm)).collect(),
        meters: s.tempo.meters().iter().map(|m| (m.tick, u8::try_from(m.numerator).unwrap_or(4), u8::try_from(m.denominator).unwrap_or(4))).collect(),
        markers: s.markers.iter().map(|m| (s.tempo.samples_to_ticks(m.start, s.sample_rate), m.name.clone())).collect(),
        key_sigs: Vec::new(),
    };
    for t in s.tracks.iter().filter(|t| t.kind.is_midi()) {
        let mut seq = soundcraft_midi::Sequence::default();
        for c in t.clips() {
            if let ClipContent::Midi { sequence } = &c.content {
                let base = s.tempo.samples_to_ticks(c.start, s.sample_rate);
                for n in &sequence.notes {
                    let mut n = *n;
                    n.start += base;
                    seq.notes.push(n);
                }
            }
        }
        seq.sort();
        smf.tracks.push(soundcraft_midi::SmfTrack { name: t.name.clone(), sequence: seq });
    }
    soundcraft_midi::write_smf(&smf).map_err(|err| EngineError::Io(err.to_string()))
}

/// Render the main mix. Returns the encoded file and (peak dBFS, integrated LUFS).
pub fn bounce_bytes(e: &Engine, r: Range, opts: &EncodeOptions, normalize: bool) -> Result<(Vec<u8>, (f32, f32, f32))> {
    bounce_bytes_with(e, r, opts, normalize, false)
}

/// Render the main mix in its own format (every main channel, SMPTE/WAV order, with the WAV
/// speaker mask of the main format), or an ITU stereo fold-down when `fold_stereo`.
pub fn bounce_bytes_with(e: &Engine, r: Range, opts: &EncodeOptions, normalize: bool, fold_stereo: bool) -> Result<(Vec<u8>, (f32, f32, f32))> {
    let (bytes, st) = bounce_bytes_checked(e, r, opts, normalize, fold_stereo, &BounceGuard::default())?;
    Ok((bytes, (st.peak_db, st.integrated_lufs, st.true_peak_db)))
}

/// What an export may do when the render is not the mix the user hears (R31).
#[derive(Debug, Clone, Copy, Default)]
pub struct BounceGuard {
    /// Samples (or the declared maximum) rendered past the range end.
    pub tail: soundcraft_mix::Tail,
    /// The user explicitly accepts an export in which a plugin that failed to load is bypassed.
    pub accept_substituted: bool,
    /// Output sample rate when it differs from the session's (windowed-sinc conversion).
    pub rate: Option<u32>,
}

/// Measured facts about an export (never targets: nothing is normalised or limited unless asked).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct ExportStats {
    pub peak_db: f32,
    pub integrated_lufs: f32,
    pub true_peak_db: f32,
    pub loudness_range_lu: f32,
    pub max_momentary_lufs: f32,
    pub rate: u32,
    pub channels: usize,
    pub frames: usize,
    pub tail_samples: usize,
}

/// Refuses a render in which a hosted plugin failed to load, unless the user accepted that, and
/// always one in which a sandboxed plugin's worker failed mid-render (its blocks became silence:
/// dropouts no one asked for; export again, or bypass or remove the plugin).
fn substituted_guard(cmd: &str, report: &soundcraft_mix::RenderReport, guard: &BounceGuard) -> Result<()> {
    if !report.faulted_plugins.is_empty() {
        let list: Vec<String> =
            report.faulted_plugins.iter().map(|(t, slot, id, blocks)| format!("{t} {slot}: {id} ({blocks} silent blocks)")).collect();
        return Err(EngineError::BadParams(
            cmd.into(),
            format!(
                "{} sandboxed plugin(s) stopped processing during the render (worker crashed, hung or restarted), so this export would contain dropouts ({}); nothing was written. Export again, or bypass or remove the plugin",
                list.len(),
                list.join("; ")
            ),
        ));
    }
    if report.failed_plugins.is_empty() || guard.accept_substituted {
        return Ok(());
    }
    let list: Vec<String> = report.failed_plugins.iter().map(|(t, slot, id)| format!("{t} insert {}: {id}", (b'a' + *slot as u8) as char)).collect();
    Err(EngineError::BadParams(
        cmd.into(),
        format!(
            "{} plugin(s) failed to load, so this export would not be the mix you hear ({}); fix the plugin or pass acknowledge_missing: true to export without their sound",
            list.len(),
            list.join("; ")
        ),
    ))
}

pub fn bounce_bytes_checked(
    e: &Engine,
    r: Range,
    opts: &EncodeOptions,
    normalize: bool,
    fold_stereo: bool,
    guard: &BounceGuard,
) -> Result<(Vec<u8>, ExportStats)> {
    let s = e.session();
    if r.len() > s.sample_rate.samples(4.0 * 3600.0) {
        return Err(EngineError::BadParams("file.bounce_mix".into(), "bounces are limited to 4 hours".into()));
    }
    let report = if let Some(src) = s.mastering.program.and_then(|id| s.pool.get(id)) {
        // Mastering an imported program: it plays in place of the mix through the Master chain.
        soundcraft_mix::render_program_report(s, &src.buffer.channels, r, 1024, guard.tail)
    } else {
        soundcraft_mix::render_range_report(s, r, 1024, guard.tail)
    };
    substituted_guard("file.bounce_mix", &report, guard)?;
    let (mut ch, fmt) = main_format_of(s, report.audio, fold_stereo);
    if normalize {
        soundcraft_dsp::offline::normalize(&mut ch, -0.1, false);
    }
    let mut rate = s.sample_rate.hz();
    if let Some(target) = guard.rate.filter(|t| *t != rate && *t > 0) {
        ch = soundcraft_dsp::offline::resample(&ch, rate, target);
        rate = target;
    }
    let peak = soundcraft_dsp::offline::peak_db(&ch);
    let mut lm = soundcraft_dsp::meter::LoudnessMeter::new(rate as f32, ch.len());
    let frames = ch.first().map_or(0, Vec::len);
    lm.process(&ch, frames);
    let stats = ExportStats {
        peak_db: peak,
        integrated_lufs: lm.integrated_lufs(),
        true_peak_db: lm.true_peak_db(),
        loudness_range_lu: lm.loudness_range_lu(),
        max_momentary_lufs: lm.max_momentary_lufs(),
        rate,
        channels: ch.len(),
        frames,
        tail_samples: report.tail_samples,
    };
    let buf = AudioBuffer { sample_rate: rate, channels: ch };
    let bytes = soundcraft_audio_io::encode_with_channel_mask(&buf, opts, fmt.channel_mask()).map_err(|err| EngineError::Io(err.to_string()))?;
    Ok((bytes, stats))
}

/// Decodes an export back and checks it against what was intended (R34 file verification).
pub fn verify_export(bytes: &[u8], ext: &str, stats: &ExportStats) -> Value {
    match soundcraft_audio_io::decode(bytes, Some(ext)) {
        Ok((_, buf)) => {
            let frames = buf.channels.first().map_or(0, Vec::len);
            let peak = soundcraft_dsp::offline::peak_db(&buf.channels);
            let ok = frames == stats.frames && buf.sample_rate == stats.rate && buf.channels.len() == stats.channels;
            json!({"header_ok": ok, "decoded_frames": frames, "decoded_rate": buf.sample_rate, "decoded_channels": buf.channels.len(), "decoded_peak_db": peak, "expected": {"frames": stats.frames, "rate": stats.rate, "channels": stats.channels}, "content_fnv": crate::autosave::fnv1a64(bytes)})
        }
        Err(e) => json!({"header_ok": false, "error": e.to_string()}),
    }
}

/// The main mix's format (folded to stereo on request).
fn main_format_of(s: &Session, ch: Vec<Vec<f32>>, fold_stereo: bool) -> (Vec<Vec<f32>>, ChannelFormat) {
    let fmt = if ch.len() <= 2 { ChannelFormat::Stereo } else { s.main_format() };
    if fold_stereo && ch.len() > 2 { (soundcraft_mix::fold_down_stereo(fmt, &ch), ChannelFormat::Stereo) } else { (ch, fmt) }
}

/// Bounce one file per audio/instrument track (each soloed, so its sends and auxes are included)
/// into `dir`. Returns the written paths.
pub fn bounce_stems(e: &Engine, dir: &str, r: Range, opts: &EncodeOptions) -> Result<Vec<String>> {
    bounce_stems_with(e, dir, r, opts, false)
}

/// [`bounce_stems`] in the main format, or folded down to stereo when `fold_stereo`.
pub fn bounce_stems_with(e: &Engine, dir: &str, r: Range, opts: &EncodeOptions, fold_stereo: bool) -> Result<Vec<String>> {
    bounce_stems_checked(e, dir, r, opts, fold_stereo, &BounceGuard::default())
}

pub fn bounce_stems_checked(e: &Engine, dir: &str, r: Range, opts: &EncodeOptions, fold_stereo: bool, guard: &BounceGuard) -> Result<Vec<String>> {
    let s = e.session();
    std::fs::create_dir_all(dir).map_err(|err| EngineError::Io(format!("{dir}: {err}")))?;
    let ext = match opts.format {
        FileFormat::Aiff => "aif",
        FileFormat::Flac => "flac",
        _ => "wav",
    };
    let mut out = Vec::new();
    let stems: Vec<(TrackId, String)> = s
        .tracks
        .iter()
        .filter(|t| matches!(t.kind, TrackKind::Audio | TrackKind::Instrument | TrackKind::Midi) && !t.inactive && !t.mixer.mute)
        .map(|t| (t.id, t.name.clone()))
        .collect();
    for (id, name) in stems {
        let mut solo = s.clone();
        for t in &mut solo.tracks {
            t.mixer.solo = t.id == id;
        }
        let report = soundcraft_mix::render_range_report(&solo, r, 1024, guard.tail);
        substituted_guard("file.bounce_stems", &report, guard)?;
        let (ch, fmt) = main_format_of(&solo, report.audio, fold_stereo);
        let buf = AudioBuffer { sample_rate: s.sample_rate.hz(), channels: ch };
        let bytes = soundcraft_audio_io::encode_with_channel_mask(&buf, opts, fmt.channel_mask()).map_err(|err| EngineError::Io(err.to_string()))?;
        let path = Path::new(dir).join(format!("{} - {}.{ext}", sanitize_name(&s.name), sanitize_name(&name)));
        std::fs::write(&path, bytes).map_err(|err| EngineError::Io(format!("{}: {err}", path.display())))?;
        out.push(path.to_string_lossy().into_owned());
    }
    Ok(out)
}

/// Limit a range used for rendering to the track's material (plus a second), so a huge
/// selection never renders hours of silence.
pub fn bounded_to_track(s: &Session, t: TrackId, r: Range) -> Range {
    let Some(tr) = s.track(t) else { return Range::point(r.start) };
    let end = tr.clips().iter().map(Clip::end).max().unwrap_or(0).saturating_add(s.sample_rate.samples(1.0));
    let start = r.start.max(0).min(end);
    Range::new(start, r.end.min(end).max(start))
}

/// Transient positions (absolute) in a track's clips within `r`.
pub fn transients_in(s: &Session, t: TrackId, r: Range, sens: f32) -> Vec<Samples> {
    let r = if r.is_empty() { Range::new(0, s.content_end()) } else { r };
    let r = bounded_to_track(s, t, r);
    let audio = soundcraft_mix::render_clips(s, t, r);
    soundcraft_dsp::offline::detect_transients(&audio, s.sample_rate.as_f64() as f32, sens).into_iter().map(|i| r.start + i as i64).collect()
}

/// Strip Silence: keep only the non-silent ranges of clips inside `r`.
pub fn strip_silence(s: &mut Session, t: TrackId, r: Range, thr: f32, min_len: Samples, pre: Samples, post: Samples) -> usize {
    let r = bounded_to_track(s, t, r);
    let audio = soundcraft_mix::render_clips(s, t, r);
    let keep = soundcraft_dsp::offline::non_silent_ranges(
        &audio,
        thr,
        usize::try_from(min_len).unwrap_or(0),
        usize::try_from(pre).unwrap_or(0),
        usize::try_from(post).unwrap_or(0),
    );
    let mut cut_points = vec![r.start];
    for (a, b) in &keep {
        cut_points.push(r.start + *a as i64);
        cut_points.push(r.start + *b as i64);
    }
    cut_points.push(r.end);
    for p in &cut_points {
        crate::edit::separate_at(s, t, *p);
    }
    // Remove pieces that fall in the gaps.
    let mut gaps = Vec::new();
    let mut prev = r.start;
    for (a, b) in &keep {
        let a = r.start + *a as i64;
        if a > prev {
            gaps.push(Range::new(prev, a));
        }
        prev = r.start + *b as i64;
    }
    if prev < r.end {
        gaps.push(Range::new(prev, r.end));
    }
    if let Some(pl) = s.track_mut(t).and_then(|x| x.playlist_mut()) {
        pl.clips.retain(|c| !gaps.iter().any(|g| c.start >= g.start && c.end() <= g.end));
    }
    keep.len()
}

/// Consolidate the selection on a track into one new clip backed by a rendered source.
pub fn consolidate(e: &mut Engine, t: TrackId, r: Range) -> Result<bool> {
    let s = e.session();
    if s.track(t).is_none_or(|tr| !tr.kind.has_playlist() || tr.kind.is_midi()) {
        return Ok(false);
    }
    let r = bounded_to_track(s, t, r);
    if r.is_empty() {
        return Ok(false);
    }
    let audio = soundcraft_mix::render_clips(s, t, r);
    let name = format!("{}_consolidated", s.track(t).map_or("Audio", |x| x.name.as_str()));
    let buf = AudioBuffer { sample_rate: s.sample_rate.hz(), channels: audio };
    let s = e.session_mut();
    let src = add_source(s, &name, buf, None, FileFormat::Wav);
    crate::edit::clear_range(s, t, r, false);
    let cid = s.new_clip_id();
    crate::edit::place_clip(s, t, Clip::audio(cid, name, src, 0, r.start, r.len()));
    Ok(true)
}

/// Render clip gain into a new source and reset the clip's gain.
pub fn render_clip_gain(e: &mut Engine, id: soundcraft_model::ClipId) -> Result<bool> {
    let s = e.session();
    let Some((t, c)) = s.find_clip(id).map(|(t, c)| (t, c.clone())) else { return Ok(false) };
    if !c.is_audio() {
        return Ok(false);
    }
    let mut c2 = c.clone();
    c2.fade_in = Default::default();
    c2.fade_out = Default::default();
    let mut tmp = s.clone();
    if let Some(pl) = tmp.track_mut(t).and_then(|x| x.playlist_mut()) {
        pl.clips = vec![c2];
    }
    let audio = soundcraft_mix::render_clips(&tmp, t, c.range());
    let buf = AudioBuffer { sample_rate: s.sample_rate.hz(), channels: audio };
    let s = e.session_mut();
    let src = add_source(s, &format!("{}_gain", c.name), buf, None, FileFormat::Wav);
    if let Some(cl) = s.find_clip_mut(id) {
        cl.content = ClipContent::Audio { source: src, offset: 0 };
        cl.gain_db = 0.0;
        cl.gain_env.clear();
    }
    Ok(true)
}

/// Commit a track's inserts into a new audio track (Track › Commit).
pub fn commit_track(e: &mut Engine, t: TrackId) -> Result<Option<TrackId>> {
    let s = e.session();
    let Some(tr) = s.track(t) else { return Ok(None) };
    if !tr.kind.is_audio_path() || tr.kind == TrackKind::Master {
        return Ok(None);
    }
    let end = tr.clips().iter().map(Clip::end).max().unwrap_or(0);
    if end <= 0 {
        return Ok(None);
    }
    let range = Range::new(0, end + s.sample_rate.samples(2.0));
    let audio = soundcraft_mix::render_track_pre_fader(s, t, range);
    let name = format!("{}.cm", tr.name);
    let fmt = ChannelFormat::for_channels(audio.len().clamp(1, 2));
    let mixer = tr.mixer.clone();
    let buf = AudioBuffer { sample_rate: s.sample_rate.hz(), channels: audio };
    let s = e.session_mut();
    let src = add_source(s, &name, buf, None, FileFormat::Wav);
    let nt = s.add_track(TrackKind::Audio, fmt, Some(&name));
    if let (Some(idx), Some(new_idx)) = (s.track_index(t), s.track_index(nt)) {
        let tr = s.tracks.remove(new_idx);
        s.tracks.insert((idx + 1).min(s.tracks.len()), tr);
    }
    let cid = s.new_clip_id();
    if let Some(ntr) = s.track_mut(nt) {
        ntr.mixer.volume_db = mixer.volume_db;
        ntr.mixer.output = mixer.output.clone();
        ntr.mixer.sends = mixer.sends.clone();
        if let Some(pl) = ntr.playlist_mut() {
            pl.clips.push(Clip::audio(cid, name, src, 0, 0, range.len()));
        }
    }
    if let Some(old) = s.track_mut(t) {
        old.inactive = true;
        old.hidden = true;
    }
    Ok(Some(nt))
}

pub fn export_clips(e: &Engine, ids: &[soundcraft_model::ClipId], dir: &str) -> Result<usize> {
    let s = e.session();
    std::fs::create_dir_all(dir).map_err(|err| EngineError::Io(format!("{dir}: {err}")))?;
    let mut n = 0;
    for id in ids {
        let Some((t, c)) = s.find_clip(*id) else { continue };
        let audio = soundcraft_mix::render_clips(s, t, c.range());
        let buf = AudioBuffer { sample_rate: s.sample_rate.hz(), channels: audio };
        let bytes = soundcraft_audio_io::encode(&buf, &EncodeOptions::default()).map_err(|err| EngineError::Io(err.to_string()))?;
        let stem = sanitize_name(&c.name);
        let mut path = Path::new(dir).join(format!("{stem}.wav"));
        let mut k = 2u32;
        while path.exists() {
            path = Path::new(dir).join(format!("{stem}-{k}.wav"));
            k = k.saturating_add(1);
            if k > 10_000 {
                return Err(EngineError::Io(format!("{dir}: too many files named {stem}")));
            }
        }
        std::fs::write(&path, bytes).map_err(|err| EngineError::Io(format!("{}: {err}", path.display())))?;
        n += 1;
    }
    Ok(n)
}

pub fn sanitize_name(n: &str) -> String {
    let s: String = n.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | ' ' | '.') { c } else { '_' }).collect();
    let s = s.trim().trim_start_matches('.').to_string();
    if s.is_empty() { "untitled".into() } else { s.chars().take(100).collect() }
}

/// Save the session to `path` (a `.scraft` file). Audio that only exists in memory is written
/// as 32-bit float WAV into `Audio Files/` next to it. Returns how many audio files were written.
pub fn save_session(e: &mut Engine, path: &str) -> Result<usize> {
    let (fresh, written, p) = write_session(e.session(), path, false)?;
    // Keep the in-memory document in sync (paths, unsaved flags) without an undo step.
    *e.session_mut() = fresh;
    e.path = Some(p.to_string_lossy().into_owned());
    e.mark_clean();
    Ok(written)
}

/// Write a copy of the session (and any audio it needs) without touching the open document —
/// used for autosave / crash recovery. Audio files in the target folder are overwritten in place.
pub fn save_copy(e: &Engine, path: &str) -> Result<usize> {
    write_session(e.session(), path, true).map(|(_, n, _)| n)
}

/// Write `session` to `path`; returns the session with updated media paths, the number of audio
/// files written and the final path.
fn write_session(session: &Session, path: &str, overwrite: bool) -> Result<(Session, usize, PathBuf)> {
    let p = PathBuf::from(path);
    let p = if p.extension().is_none() { p.with_extension(soundcraft_model::SESSION_EXTENSION) } else { p };
    let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
    let audio_dir = dir.join("Audio Files");
    let mut s = session.clone();
    let mut written = 0;
    for src in &mut s.sources {
        let rel_target = format!("Audio Files/{}.wav", sanitize_name(&src.name));
        let exists_rel = !Path::new(&src.path).is_absolute() && dir.join(&src.path).exists();
        if exists_rel && !src.unsaved {
            continue;
        }
        let Some(audio) = s.pool.get(src.id) else { continue };
        std::fs::create_dir_all(&audio_dir).map_err(|err| EngineError::Io(format!("{}: {err}", audio_dir.display())))?;
        let mut target = rel_target.clone();
        if overwrite {
            // One file per source id so autosaves never collide or pile up.
            target = format!("Audio Files/{}_{}.wav", sanitize_name(&src.name), src.id.0);
            if dir.join(&target).exists() {
                src.path = target;
                src.unsaved = false;
                continue;
            }
        } else {
            let mut k = 1;
            while dir.join(&target).exists() && !(exists_rel && target == src.path) {
                target = format!("Audio Files/{}_{k:02}.wav", sanitize_name(&src.name));
                k += 1;
                if k > 999 {
                    break;
                }
            }
        }
        let opts = EncodeOptions { format: FileFormat::Wav, bit_depth: soundcraft_audio_io::BitDepth::Float32, dither: false, bwf: None };
        let bytes = soundcraft_audio_io::encode(&audio.buffer, &opts).map_err(|err| EngineError::Io(err.to_string()))?;
        std::fs::write(dir.join(&target), bytes).map_err(|err| EngineError::Io(format!("{target}: {err}")))?;
        src.path = target;
        src.unsaved = false;
        src.format = FileFormat::Wav;
        written += 1;
    }
    if !overwrite && let Some(stem) = p.file_stem().and_then(|x| x.to_str()) {
        s.name = stem.to_string();
    }
    let text = s.to_json().map_err(|err| EngineError::Io(err.to_string()))?;
    // Atomic-ish write: temp file then rename.
    let tmp = p.with_extension("scraft.tmp");
    std::fs::write(&tmp, text).map_err(|err| EngineError::Io(format!("{}: {err}", tmp.display())))?;
    std::fs::rename(&tmp, &p).map_err(|err| EngineError::Io(format!("{}: {err}", p.display())))?;
    Ok((s, written, p))
}

/// Open a `.scraft` session and decode its audio files (missing files are reported, not fatal).
pub fn open_session(e: &mut Engine, path: &str) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path).map_err(|err| EngineError::Io(format!("{path}: {err}")))?;
    let mut s = Session::from_json(&text).map_err(|err| EngineError::Io(err.to_string()))?;
    let dir = Path::new(path).parent().map(Path::to_path_buf).unwrap_or_default();
    let mut missing = Vec::new();
    for src in s.sources.clone() {
        let p = if Path::new(&src.path).is_absolute() { PathBuf::from(&src.path) } else { dir.join(&src.path) };
        match std::fs::read(&p) {
            Ok(bytes) => match soundcraft_audio_io::decode(&bytes, p.extension().and_then(|x| x.to_str())) {
                Ok((_, mut buf)) => {
                    if buf.sample_rate != s.sample_rate.hz() && buf.sample_rate > 0 {
                        buf.channels = soundcraft_dsp::offline::resample(&buf.channels, buf.sample_rate, s.sample_rate.hz());
                        buf.sample_rate = s.sample_rate.hz();
                    }
                    s.pool.insert(src.id, Arc::new(SourceAudio::new(buf)));
                }
                Err(err) => missing.push(format!("{}: {err}", p.display())),
            },
            Err(_) => missing.push(p.display().to_string()),
        }
    }
    missing.extend(relink_videos(&mut s, &dir));
    e.replace_session(s);
    e.path = Some(path.to_string());
    for m in &missing {
        e.message(format!("missing media: {m}"));
    }
    // Derived Bend media that did not load is rebuilt from the original (R30).
    for m in crate::cmd::rerender_missing_warps(e) {
        e.message(m);
    }
    Ok(missing)
}

/// Turn a recorded take into clips on the record-armed tracks (one undo step: "Record").
/// `channels` is the planar input capture at `rate`; each track takes the input channels its
/// input route names ("In 3" → channel 2), defaulting to the first channels.
/// Count Off: where the transport starts so `bars` whole bars (of the meter in force before
/// each bar line, at the tempo map's tempos) are counted before `record_at`. Clamped at the
/// session start, so a count-off near zero is shorter.
pub fn countoff_start(s: &Session, record_at: Samples, bars: u32) -> Samples {
    let sr = s.sample_rate;
    let mut tick = s.tempo.samples_to_ticks(record_at.max(0), sr);
    for _ in 0..bars {
        let bar = s.tempo.meter_at_tick(tick.saturating_sub(1).max(0)).ticks_per_bar();
        tick = tick.saturating_sub(bar);
        if tick <= 0 {
            return 0;
        }
    }
    // Same count-off length in samples as the tempo map gives, ending exactly at `record_at`.
    let len = s.tempo.tick_to_samples(s.tempo.samples_to_ticks(record_at.max(0), sr), sr) - s.tempo.tick_to_samples(tick, sr);
    record_at.saturating_sub(len).max(0)
}

/// Trims freshly recorded clips to start no earlier than `from` (Count Off / pre-roll audio is
/// kept in the file, outside the clip). Part of the Record step: call right after
/// [`add_recording`].
pub fn trim_takes_to_start(e: &mut Engine, clips: &[soundcraft_model::ClipId], from: Samples) {
    let s = e.session_mut();
    for id in clips {
        if let Some(c) = s.find_clip_mut(*id)
            && c.start < from
        {
            c.trim_start_to(from);
        }
    }
}

/// Zero-based first capture channel of an armed track ("In 3" → 2, "In 1-2" → 0; other
/// inputs → 0).
fn first_input_channel(t: &soundcraft_model::Track) -> usize {
    match &t.mixer.input {
        soundcraft_model::Route::Hardware(h) => {
            h.trim_start_matches("In ").split(['-', ' ']).next().and_then(|n| n.parse::<usize>().ok()).map_or(0, |n| n.saturating_sub(1))
        }
        _ => 0,
    }
}

/// R01/H1 preflight: armed audio tracks whose input channels the capture device does not have
/// (`device_channels` inputs). A one-input device may feed a stereo track (dual mono, stated in
/// the manual); any other shortfall would record a different physical input, so recording must
/// not start. Returns one message per affected track.
pub fn input_channel_problems(s: &Session, device_channels: usize) -> Vec<String> {
    s.tracks
        .iter()
        .filter(|t| t.mixer.record_arm && t.kind == TrackKind::Audio && !t.inactive)
        .filter_map(|t| {
            let first = first_input_channel(t);
            let need = first + t.channels();
            let dual_mono = device_channels == 1 && first == 0;
            (need > device_channels && !dual_mono)
                .then(|| format!("`{}` records input {}–{} but the input device has {device_channels} channel(s)", t.name, first + 1, need))
        })
        .collect()
}

/// Which capture channels (of `device_channels`) the armed audio tracks record — read back only
/// these after a take.
pub fn armed_input_channels(s: &Session, device_channels: usize) -> Vec<bool> {
    let mut keep = vec![false; device_channels];
    for t in s.tracks.iter().filter(|t| t.mixer.record_arm && t.kind == TrackKind::Audio && !t.inactive) {
        let first = first_input_channel(t);
        for k in 0..t.channels() {
            let ch = if device_channels == 1 { 0 } else { first + k };
            if let Some(x) = keep.get_mut(ch) {
                *x = true;
            }
        }
    }
    keep
}

pub fn add_recording(e: &mut Engine, start: Samples, channels: Vec<Vec<f32>>, rate: u32) -> Result<Vec<soundcraft_model::ClipId>> {
    // Channels no armed track records may be empty (read back selectively).
    let frames = channels.iter().map(Vec::len).max().unwrap_or(0);
    if frames == 0 {
        return Ok(Vec::new());
    }
    let armed: Vec<(TrackId, usize, usize, String)> = e
        .session()
        .tracks
        .iter()
        .filter(|t| t.mixer.record_arm && t.kind == TrackKind::Audio && !t.inactive)
        .map(|t| (t.id, first_input_channel(t), t.channels(), t.name.clone()))
        .collect();
    if armed.is_empty() {
        return Err(EngineError::BadParams("record".into(), "no tracks are record-enabled".into()));
    }
    // Never substitute another physical input (checked before recording starts, again here).
    let problems = input_channel_problems(e.session(), channels.len());
    if !problems.is_empty() {
        return Err(EngineError::BadParams("record".into(), problems.join("; ")));
    }
    let before = e.session_arc();
    let mut out = Vec::new();
    let take_no = e.session().sources.len() + 1;
    for (tid, first, n, name) in armed {
        // A one-input capture feeding a stereo track is dual mono (the only allowed fill).
        let chans: Vec<Vec<f32>> = (0..n).map(|k| channels.get((first + k).min(channels.len() - 1)).cloned().unwrap_or_default()).collect();
        let buf = AudioBuffer { sample_rate: rate, channels: chans };
        let clip_name = format!("{name}_{take_no:02}");
        let s = e.session_mut();
        let src = add_source(s, &clip_name, buf, None, FileFormat::Wav);
        let len = s.source(src).map_or(0, |x| i64::try_from(x.frames).unwrap_or(0));
        let cid = s.new_clip_id();
        crate::edit::place_clip(s, tid, Clip::audio(cid, clip_name, src, 0, start, len));
        out.push(cid);
    }
    e.push_undo("Record", before);
    Ok(out)
}

/// Loop recording (V07): splits one continuous capture into one take per loop pass of `sel`,
/// each on its own playlist (earlier passes stay as alternates), every pass placed at
/// `sel.start`. A final partial pass shorter than 1/8 of the loop is dropped. Returns the number
/// of takes made. Pass boundaries are exact sample multiples of the loop length (at the capture
/// rate); the capture file itself is not modified.
pub fn add_loop_recording(e: &mut Engine, sel: soundcraft_time::Range, channels: &[Vec<f32>], rate: u32) -> usize {
    add_loop_recording_at(e, sel, sel.start, channels, rate)
}

/// Loop recording whose first captured frame belongs at session position `capture_start` (from
/// stream timestamps, CL04). Pass `k` is the capture span that played during loop pass `k`:
/// audio captured before the loop start is not part of any pass; when capture began late, the
/// first take starts later than the loop start by that amount. The capture is never resampled.
pub fn add_loop_recording_at(e: &mut Engine, sel: soundcraft_time::Range, capture_start: Samples, channels: &[Vec<f32>], rate: u32) -> usize {
    let to_capture = |session_samples: i64| soundcraft_time::to_samples(session_samples as f64 * f64::from(rate) / e.session().sample_rate.as_f64());
    let pass = to_capture(sel.len()).max(1);
    // Capture index of the loop start (negative: capture began after it).
    let lead = to_capture(sel.start - capture_start);
    let total = channels.iter().map(Vec::len).max().unwrap_or(0) as i64;
    let passes = (total - lead).max(0).div_euclid(pass) + i64::from((total - lead).max(0).rem_euclid(pass) > 0);
    let mut made = 0;
    for k in 0..passes.max(1) {
        let begin = lead + k * pass;
        let (from, late) = if begin < 0 { (0, -begin) } else { (begin, 0) };
        let end = (begin + pass).min(total);
        if end - from < pass / 8 {
            continue;
        }
        let (Ok(from_u), Ok(end_u)) = (usize::try_from(from), usize::try_from(end)) else { continue };
        let chunk: Vec<Vec<f32>> = channels.iter().map(|c| c.get(from_u..end_u.min(c.len())).map(<[f32]>::to_vec).unwrap_or_default()).collect();
        if made > 0 {
            let armed: Vec<u64> = e.session().tracks.iter().filter(|t| t.mixer.record_arm).map(|t| t.id.0).collect();
            let _ = e.execute("track.playlist_new", &serde_json::json!({"tracks": armed}));
        }
        let at = sel.start + soundcraft_time::to_samples(late as f64 * e.session().sample_rate.as_f64() / f64::from(rate));
        if add_recording(e, at, chunk, rate).is_ok() {
            made += 1;
        }
    }
    made
}

#[cfg(test)]
mod input_channel_tests {
    use super::*;
    use soundcraft_model::{ChannelFormat, Route};

    /// R01/H1: a track never records a different physical input than its own; a one-input
    /// device may feed a stereo track (dual mono); the take maps "In 3" to capture channel 3.
    #[test]
    fn recording_never_substitutes_another_input_channel() {
        let mut e = Engine::default();
        let v = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
        let tr = e.session_mut().track_mut(v).unwrap();
        tr.mixer.record_arm = true;
        tr.mixer.input = Route::Hardware("In 3".into());
        let cap = |n: usize| (0..n).map(|c| vec![c as f32 + 1.0; 100]).collect::<Vec<_>>();
        assert_eq!(input_channel_problems(e.session(), 2).len(), 1, "In 3 on a 2-input device");
        let err = add_recording(&mut e, 0, cap(2), 48_000).unwrap_err().to_string();
        assert!(err.contains("Vox") && err.contains("2 channel"), "{err}");
        assert!(e.session().track(v).unwrap().clips().is_empty(), "nothing placed");
        assert!(input_channel_problems(e.session(), 8).is_empty());
        let ids = add_recording(&mut e, 0, cap(8), 48_000).unwrap();
        let c = e.session().track(v).unwrap().clips().iter().find(|c| c.id == ids[0]).cloned().unwrap();
        let ClipContent::Audio { source, .. } = c.content else { panic!() };
        assert_eq!(e.session().pool.get(source).unwrap().channel(0).unwrap()[0], 3.0, "capture channel 3");
        // Selective read-back: only channel 3 present (others empty) records the same take.
        let keep = armed_input_channels(e.session(), 8);
        assert_eq!(keep, vec![false, false, true, false, false, false, false, false]);
        let sparse: Vec<Vec<f32>> = cap(8).into_iter().enumerate().map(|(i, c)| if keep[i] { c } else { Vec::new() }).collect();
        let ids = add_recording(&mut e, 0, sparse, 48_000).unwrap();
        let c = e.session().track(v).unwrap().clips().iter().find(|c| c.id == ids[0]).cloned().unwrap();
        assert_eq!(c.length, 100, "length from the kept channel, not channel 0");
        // Stereo track on a one-input device: dual mono, allowed.
        let st = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Stereo, Some("Gtr"));
        e.session_mut().track_mut(v).unwrap().mixer.record_arm = false;
        e.session_mut().track_mut(st).unwrap().mixer.record_arm = true;
        assert!(input_channel_problems(e.session(), 1).is_empty());
        e.session_mut().track_mut(st).unwrap().mixer.input = Route::Hardware("In 2-3".into());
        assert_eq!(input_channel_problems(e.session(), 2).len(), 1, "In 2-3 needs 3 inputs");
    }
}

#[cfg(test)]
mod countoff_tests {
    use super::*;

    /// CL-T3 count-in: N bars of the meter in force before each bar line, ending exactly at the
    /// record point, across tempo and meter changes; clamped at zero.
    #[test]
    fn countoff_counts_whole_bars_back_from_the_record_point() {
        let mut s = Session::default();
        let sr = s.sample_rate;
        let tpb = s.tempo.meter_at_tick(0).ticks_per_beat();
        s.tempo.set_tempo(0, 100.0).unwrap();
        s.tempo.set_meter(8 * tpb, 3, 4).unwrap(); // 4/4 for two bars, then 3/4
        s.tempo.set_tempo(8 * tpb, 140.0).unwrap();
        // Record at the start of the 2nd 3/4 bar: 2 bars back = one 3/4 bar + one 4/4 bar.
        let rec_tick = 8 * tpb + 3 * tpb;
        let rec = s.tempo.tick_to_samples(rec_tick, sr);
        let from = countoff_start(&s, rec, 2);
        assert_eq!(from, s.tempo.tick_to_samples(rec_tick - 3 * tpb - 4 * tpb, sr));
        assert_eq!(countoff_start(&s, rec, 0), rec);
        assert_eq!(countoff_start(&s, s.tempo.tick_to_samples(tpb, sr), 2), 0, "clamped at the start");
        // Trimming the take leaves the count-off audio outside the clip.
        let mut e = Engine::default();
        let t = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        e.session_mut().track_mut(t).unwrap().mixer.record_arm = true;
        let ids = add_recording(&mut e, 1000, vec![vec![0.5; 4000]], 48_000).unwrap();
        trim_takes_to_start(&mut e, &ids, 3000);
        let c = e.session().track(t).unwrap().clips()[0].clone();
        assert_eq!((c.start, c.length, c.source_offset()), (3000, 2000, 2000));
    }
}

#[cfg(test)]
mod loop_record_tests {
    use super::*;

    /// V07 / CL-T3: 3.5 loop passes → 4 takes on 4 playlists, each starting exactly at the
    /// loop start with the capture samples of its pass; a sliver tail (< 1/8 pass) is dropped.
    #[test]
    fn loop_passes_become_aligned_takes() {
        let mut e = Engine::default();
        let t = e.session_mut().add_track(TrackKind::Audio, soundcraft_model::ChannelFormat::Mono, Some("Vox"));
        e.session_mut().track_mut(t).unwrap().mixer.record_arm = true;
        let sel = soundcraft_time::Range::new(48_000, 48_000 + 1000);
        let capture: Vec<f32> = (0..3500).map(|i| i as f32).collect();
        assert_eq!(add_loop_recording(&mut e, sel, std::slice::from_ref(&capture), 48_000), 4);
        let tr = e.session().track(t).unwrap();
        assert_eq!(tr.playlists.len(), 4);
        for (k, pl) in tr.playlists.iter().enumerate() {
            let c = &pl.clips[0];
            assert_eq!(c.start, 48_000, "pass {k} placed at the loop start");
            let ClipContent::Audio { source, offset } = c.content else { panic!() };
            let audio = e.session().pool.get(source).unwrap();
            assert_eq!(audio.channel(0).unwrap()[offset as usize], (k * 1000) as f32, "pass {k} begins at its exact sample");
        }
        let sliver: Vec<f32> = (0..3100).map(|i| i as f32).collect();
        let mut e2 = Engine::default();
        let t2 = e2.session_mut().add_track(TrackKind::Audio, soundcraft_model::ChannelFormat::Mono, None);
        e2.session_mut().track_mut(t2).unwrap().mixer.record_arm = true;
        assert_eq!(add_loop_recording(&mut e2, sel, &[sliver], 48_000), 3, "100-sample tail (< 1/8 pass) dropped");
    }

    /// CL04 + V07: with a timestamp-derived capture start, passes split at the loop start's
    /// capture index — early capture (pre-roll/latency) is skipped, late capture shortens and
    /// shifts only the first take; later passes still begin exactly at the loop start.
    #[test]
    fn loop_passes_follow_the_timestamp_capture_start() {
        let sel = soundcraft_time::Range::new(48_000, 48_000 + 1000);
        let capture: Vec<f32> = (0..3300).map(|i| i as f32).collect();
        for (capture_start, first_at, first_sample, takes) in [(47_700i64, 48_000i64, 300.0f32, 3usize), (48_050, 48_050, 0.0, 4)] {
            let mut e = Engine::default();
            let t = e.session_mut().add_track(TrackKind::Audio, soundcraft_model::ChannelFormat::Mono, Some("Vox"));
            e.session_mut().track_mut(t).unwrap().mixer.record_arm = true;
            assert_eq!(add_loop_recording_at(&mut e, sel, capture_start, std::slice::from_ref(&capture), 48_000), takes, "start {capture_start}");
            let tr = e.session().track(t).unwrap();
            for (k, pl) in tr.playlists.iter().enumerate() {
                let c = &pl.clips[0];
                let ClipContent::Audio { source, offset } = c.content else { panic!() };
                let first = e.session().pool.get(source).unwrap().channel(0).unwrap()[offset as usize];
                if k == 0 {
                    assert_eq!((c.start, first), (first_at, first_sample), "start {capture_start}: first take");
                } else {
                    assert_eq!(c.start, 48_000);
                    // Pass k begins where the loop start recurs in the capture.
                    assert_eq!(first, (sel.start - capture_start + k as i64 * 1000) as f32);
                }
            }
            if capture_start > sel.start {
                assert_eq!(tr.playlists[0].clips[0].length, 950, "late capture: first take is short");
            }
        }
    }
}

#[cfg(test)]
mod save_tests {
    use super::*;

    #[test]
    fn autosave_copy_leaves_document_untouched_and_reuses_audio() {
        let mut e = crate::demo::demo_engine();
        e.execute("mix.volume", &serde_json::json!({"track": "Kick", "db": -7.0})).unwrap();
        let dir = std::env::temp_dir().join(format!("soundcraft-autosave-{}", std::process::id()));
        let path = dir.join("Midnight Groove.scraft");
        let p = path.to_string_lossy().into_owned();
        let n1 = save_copy(&e, &p).unwrap();
        let n2 = save_copy(&e, &p).unwrap();
        assert!(n1 >= 5);
        assert_eq!(n2, 0, "second autosave must not rewrite audio");
        assert!(e.is_dirty() && e.path.is_none());
        let mut r = Engine::default();
        open_session(&mut r, &p).unwrap();
        assert_eq!(r.session().track_by_name("Kick").unwrap().mixer.volume_db, -7.0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod stem_tests {
    #[test]
    fn stems_one_file_per_track() {
        let mut e = crate::demo::demo_engine();
        let dir = std::env::temp_dir().join(format!("soundcraft-stems-{}", std::process::id()));
        let r = e
            .execute("file.bounce_stems", &serde_json::json!({"dir": dir.to_string_lossy(), "start": {"seconds": 10.0}, "end": {"seconds": 11.0}}))
            .unwrap();
        let n = r["files"].as_array().map_or(0, Vec::len);
        assert_eq!(n, 7, "{r}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
