//! HF20: compressed exports (MP3 CBR, AAC-LC in M4A) of the normal bounce.
//!
//! The mix is rendered exactly as for WAV/AIFF/FLAC (same graph, range, tails, rate and fold-down),
//! converted to 16-bit (TPDF dither unless `dither: false`) and handed to the encoder built into
//! Windows (`soundcraft_mf_encode`) on the command's thread, never the audio thread. The encoder
//! writes a hidden temporary file beside the target; it is decoded back and checked (decodable,
//! channels, rate, duration within the codec's delay and padding), measured (peak, true peak,
//! integrated loudness of the *decoded* audio, which lossy coding changes), and only then renamed
//! to the target. On any failure or cancellation the temporary file is removed and nothing is
//! published. Nothing is normalised or limited unless asked (`normalize` is the existing peak
//! normalise; there is no loudness target or limiter), and a format change never alters the mix.

use serde_json::{Value, json};
use soundcraft_audio_io::AudioBuffer;
use soundcraft_mf_encode::Codec;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// Frames a decoded lossy file may differ from the rendered length: the encoder's delay and the
/// padding of its last frame (MP3: up to ~2 × 1152 + 529; AAC: 2112 priming + up to 1024).
pub const DURATION_TOLERANCE_FRAMES: usize = 4608;

/// What the caller asked for.
#[derive(Debug, Clone)]
pub struct LossyRequest {
    pub codec: Codec,
    pub kbps: u32,
    pub dither: bool,
    /// ID3 title / artist / comment for MP3 (M4A gets no tags in this build).
    pub title: Option<String>,
    pub artist: Option<String>,
    pub comment: Option<String>,
}

/// Interleaved 16-bit PCM with TPDF dither (deterministic generator, so exports repeat).
pub fn to_pcm16(buf: &AudioBuffer, dither: bool) -> Vec<i16> {
    let frames = buf.channels.first().map_or(0, Vec::len);
    let n = buf.channels.len();
    let mut out = Vec::with_capacity(frames * n);
    let mut seed: u32 = 0x9e37_79b9;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        f64::from(seed) / f64::from(u32::MAX) - 0.5
    };
    for i in 0..frames {
        for c in &buf.channels {
            let x = f64::from(c.get(i).copied().unwrap_or(0.0)) * 32_767.0;
            let d = if dither { rnd() + rnd() } else { 0.0 };
            out.push((x + d).round().clamp(-32_768.0, 32_767.0) as i16);
        }
    }
    out
}

/// An ID3v2.3 tag (TIT2, TPE1, COMM, TSSE) to put in front of an MP3.
pub fn id3v2_tag(title: Option<&str>, artist: Option<&str>, comment: Option<&str>, software: &str) -> Vec<u8> {
    fn frame(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut f = id.to_vec();
        f.extend_from_slice(&(body.len() as u32).to_be_bytes());
        f.extend_from_slice(&[0, 0]);
        f.extend_from_slice(body);
        f
    }
    // Text as UTF-16 with BOM (encoding 1), which ID3v2.3 readers support.
    fn text(s: &str) -> Vec<u8> {
        let mut b = vec![1u8, 0xff, 0xfe];
        for u in s.encode_utf16() {
            b.extend_from_slice(&u.to_le_bytes());
        }
        b
    }
    let mut frames = Vec::new();
    if let Some(t) = title.filter(|t| !t.is_empty()) {
        frames.extend(frame(b"TIT2", &text(t)));
    }
    if let Some(a) = artist.filter(|a| !a.is_empty()) {
        frames.extend(frame(b"TPE1", &text(a)));
    }
    if let Some(c) = comment.filter(|c| !c.is_empty()) {
        let mut body = vec![1u8, b'e', b'n', b'g', 0xff, 0xfe, 0, 0];
        for u in c.encode_utf16() {
            body.extend_from_slice(&u.to_le_bytes());
        }
        frames.extend(frame(b"COMM", &body));
    }
    frames.extend(frame(b"TSSE", &text(software)));
    let size = frames.len() as u32;
    // Synchsafe size (7 bits per byte).
    let ss = [((size >> 21) & 0x7f) as u8, ((size >> 14) & 0x7f) as u8, ((size >> 7) & 0x7f) as u8, (size & 0x7f) as u8];
    let mut tag = b"ID3".to_vec();
    tag.extend_from_slice(&[3, 0, 0]);
    tag.extend_from_slice(&ss);
    tag.extend(frames);
    tag
}

/// Decodes a lossy export and checks it against what was rendered. `Ok` carries the measured
/// facts of the decoded audio; `Err` says why the file is not acceptable.
pub fn verify_lossy(bytes: &[u8], ext: &str, rate: u32, channels: usize, frames: usize) -> Result<Value, String> {
    let (_, buf) = soundcraft_audio_io::decode(bytes, Some(ext)).map_err(|e| format!("the encoded file does not decode: {e}"))?;
    let got = buf.channels.first().map_or(0, Vec::len);
    if buf.sample_rate != rate {
        return Err(format!("decoded at {} Hz, expected {rate} Hz", buf.sample_rate));
    }
    if buf.channels.len() != channels {
        return Err(format!("decoded {} channel(s), expected {channels}", buf.channels.len()));
    }
    if got.abs_diff(frames) > DURATION_TOLERANCE_FRAMES {
        return Err(format!("decoded {got} frames, expected {frames} (more than the codec's delay and padding)"));
    }
    let peak = soundcraft_dsp::offline::peak_db(&buf.channels);
    let mut lm = soundcraft_dsp::meter::LoudnessMeter::new(rate as f32, channels);
    lm.process(&buf.channels, got);
    Ok(json!({
        "decoded_frames": got,
        "expected_frames": frames,
        "delay_padding_frames": got as i64 - frames as i64,
        "decoded_rate": buf.sample_rate,
        "decoded_channels": buf.channels.len(),
        "decoded_peak_db": peak,
        "decoded_true_peak_db": lm.true_peak_db(),
        "decoded_integrated_lufs": lm.integrated_lufs(),
        "tolerance_frames": DURATION_TOLERANCE_FRAMES,
    }))
}

/// The hidden temporary file the encoder writes beside `target` (same extension, so the
/// container is the same).
pub fn partial_path(target: &Path, codec: Codec) -> PathBuf {
    let stem = target.file_stem().and_then(|s| s.to_str()).unwrap_or("export");
    target.with_file_name(format!(".{stem}.sml-partial.{}", codec.extension()))
}

/// Encodes `buf` to `target` through a verified temporary file. Returns the verification facts.
pub fn write(target: &Path, buf: &AudioBuffer, req: &LossyRequest, cancel: &AtomicBool) -> Result<Value, String> {
    let channels = buf.channels.len();
    let frames = buf.channels.first().map_or(0, Vec::len);
    soundcraft_mf_encode::check(req.codec, buf.sample_rate, channels, req.kbps)?;
    let tmp = partial_path(target, req.codec);
    let result = (|| {
        let pcm = to_pcm16(buf, req.dither);
        soundcraft_mf_encode::encode_file(&tmp, req.codec, buf.sample_rate, channels, req.kbps, &pcm, cancel)?;
        let mut bytes = std::fs::read(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
        if req.codec == Codec::Mp3 {
            let mut tagged =
                id3v2_tag(req.title.as_deref(), req.artist.as_deref(), req.comment.as_deref(), concat!("SML Studio ", env!("CARGO_PKG_VERSION")));
            tagged.extend_from_slice(&bytes);
            bytes = tagged;
            std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
        }
        let v = verify_lossy(&bytes, req.codec.extension(), buf.sample_rate, channels, frames)?;
        std::fs::rename(&tmp, target).map_err(|e| format!("{}: {e}", target.display()))?;
        Ok(v)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, secs: f32, ch: usize) -> AudioBuffer {
        let n = (rate as f32 * secs) as usize;
        let c: Vec<f32> = (0..n).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.5).collect();
        AudioBuffer { sample_rate: rate, channels: vec![c; ch] }
    }

    #[test]
    fn pcm16_and_id3() {
        let b = AudioBuffer { sample_rate: 48_000, channels: vec![vec![0.0, 1.0, -1.0, 0.5], vec![0.25; 4]] };
        assert_eq!(to_pcm16(&b, false), vec![0, 8_192, 32_767, 8_192, -32_767, 8_192, 16_384, 8_192]);
        let d = to_pcm16(&b, true);
        assert!(d.iter().zip(to_pcm16(&b, false)).all(|(a, b)| (i32::from(*a) - i32::from(b)).abs() <= 1), "TPDF is at most ±1 LSB");
        assert_eq!(to_pcm16(&b, true), d, "deterministic");
        let tag = id3v2_tag(Some("Song"), Some("Me"), None, "SML Studio");
        assert_eq!(&tag[..5], b"ID3\x03\x00");
        let size = (u32::from(tag[6]) << 21) | (u32::from(tag[7]) << 14) | (u32::from(tag[8]) << 7) | u32::from(tag[9]);
        assert_eq!(size as usize, tag.len() - 10);
        assert!(tag.windows(4).any(|w| w == b"TIT2") && tag.windows(4).any(|w| w == b"TPE1") && tag.windows(4).any(|w| w == b"TSSE"));
    }

    /// The decode check: exact rate and channels, duration within the delay/padding tolerance.
    #[test]
    fn verification_rules() {
        let b = sine(48_000, 1.0, 2);
        let wav = soundcraft_audio_io::encode(&b, &soundcraft_audio_io::EncodeOptions::default()).unwrap();
        let v = verify_lossy(&wav, "wav", 48_000, 2, 48_000).unwrap();
        assert_eq!(v["delay_padding_frames"], 0);
        assert!(verify_lossy(&wav, "wav", 44_100, 2, 48_000).unwrap_err().contains("Hz"));
        assert!(verify_lossy(&wav, "wav", 48_000, 1, 48_000).unwrap_err().contains("channel"));
        assert!(verify_lossy(&wav, "wav", 48_000, 2, 48_000 + DURATION_TOLERANCE_FRAMES + 1).unwrap_err().contains("frames"));
        assert!(verify_lossy(&wav[..40], "wav", 48_000, 2, 48_000).is_err(), "a truncated file is refused");
    }

    /// Real MP3 and M4A files (made here with ffmpeg when it is installed; the app itself uses the
    /// Windows encoders) decode and pass the check, with codec delay/padding reported; a corrupted
    /// file is refused.
    #[test]
    fn real_lossy_files_decode_and_verify() {
        let dir = std::env::temp_dir().join(format!("sml-lossy-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let wav = dir.join("in.wav");
        std::fs::write(&wav, soundcraft_audio_io::encode(&sine(48_000, 2.0, 2), &soundcraft_audio_io::EncodeOptions::default()).unwrap()).unwrap();
        for (ext, args) in [("mp3", ["-c:a", "libmp3lame", "-b:a", "192k"]), ("m4a", ["-c:a", "aac", "-b:a", "128k"])] {
            let out = dir.join(format!("out.{ext}"));
            let ok = std::process::Command::new("ffmpeg")
                .args(["-loglevel", "error", "-y", "-i"])
                .arg(&wav)
                .args(args)
                .arg(&out)
                .status()
                .is_ok_and(|s| s.success());
            if !ok {
                eprintln!("ffmpeg not available: skipping the {ext} fixture check");
                continue;
            }
            let bytes = std::fs::read(&out).unwrap();
            let v = verify_lossy(&bytes, ext, 48_000, 2, 96_000).unwrap_or_else(|e| panic!("{ext}: {e}"));
            assert!(v["decoded_peak_db"].as_f64().unwrap() > -7.0, "{ext}: {v}");
            if ext == "mp3" {
                // The ID3 tag this module puts in front of an MP3 does not disturb decoding.
                let mut tagged = id3v2_tag(Some("Title"), Some("Artist"), Some("Comment"), "SML Studio");
                tagged.extend_from_slice(&bytes);
                verify_lossy(&tagged, ext, 48_000, 2, 96_000).unwrap_or_else(|e| panic!("tagged mp3: {e}"));
            }
            let mut bad = bytes.clone();
            let cut = bad.len() / 3;
            bad.truncate(cut);
            assert!(verify_lossy(&bad, ext, 48_000, 2, 96_000).is_err(), "{ext}: a truncated file is refused");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Off Windows the export is refused with the reason and no file (temporary or final) remains.
    #[test]
    fn unavailable_encoder_leaves_nothing() {
        if soundcraft_mf_encode::available() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("sml-lossy-na-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join("mix.mp3");
        let req = LossyRequest { codec: Codec::Mp3, kbps: 320, dither: true, title: None, artist: None, comment: None };
        let e = write(&target, &sine(48_000, 0.1, 2), &req, &AtomicBool::new(false)).unwrap_err();
        assert!(e.contains("not available on this platform"), "{e}");
        assert!(!target.exists() && !partial_path(&target, Codec::Mp3).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
