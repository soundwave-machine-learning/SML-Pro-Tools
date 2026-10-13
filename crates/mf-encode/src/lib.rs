//! SoundCraft compressed export (HF20): MP3 and AAC-LC (in an M4A/MP4 container) written by the
//! encoders that ship with Windows (Media Foundation's MP3 encoder and AAC encoder), through an
//! `IMFSinkWriter` that also writes the container.
//!
//! Why these encoders: the project takes no GPL/LGPL code (LAME and shine are LGPL) and buys no
//! codec licences; Windows includes these encoders for applications to use. Off Windows there is
//! no encoder, and [`encode_file`] says so.
//!
//! What they accept (Microsoft's documented limits; [`check`] refuses anything else before work
//! starts):
//! - MP3: constant bit rate only (no VBR), 32, 44.1 or 48 kHz, mono or stereo, 128–320 kbps.
//! - AAC-LC: 44.1 or 48 kHz, mono or stereo, 96, 128, 160 or 192 kbps.
//!
//! Input is interleaved 16-bit PCM (what both encoders take). The writer runs on the caller's
//! thread (an export worker, never the audio thread) and checks `cancel` between chunks.
//!
//! Like `soundcraft-vst3-host`, an isolated `unsafe` crate: `unsafe` is denied crate-wide and
//! allowed only in the private, Windows-only `ffi` module. Status: compiled for Windows
//! (cross-build); **not yet run on Windows**.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::Path;
use std::sync::atomic::AtomicBool;

/// A compressed codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Mp3,
    Aac,
}

impl Codec {
    /// The codec a file extension or format name selects (`mp3`; `m4a`, `aac`, `mp4`).
    pub fn for_name(name: &str) -> Option<Codec> {
        match name.trim_start_matches('.').to_ascii_lowercase().as_str() {
            "mp3" => Some(Codec::Mp3),
            "m4a" | "aac" | "mp4" => Some(Codec::Aac),
            _ => None,
        }
    }
    /// The file extension written.
    pub fn extension(self) -> &'static str {
        match self {
            Codec::Mp3 => "mp3",
            Codec::Aac => "m4a",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Codec::Mp3 => "MP3 (CBR)",
            Codec::Aac => "AAC-LC in M4A",
        }
    }
    /// Bit rates (kbps) the Windows encoder takes.
    pub fn bitrates(self) -> &'static [u32] {
        match self {
            Codec::Mp3 => &[128, 160, 192, 224, 256, 320],
            Codec::Aac => &[96, 128, 160, 192],
        }
    }
    /// Sample rates (Hz) the Windows encoder takes.
    pub fn rates(self) -> &'static [u32] {
        match self {
            Codec::Mp3 => &[32_000, 44_100, 48_000],
            Codec::Aac => &[44_100, 48_000],
        }
    }
    /// The default bit rate.
    pub fn default_bitrate(self) -> u32 {
        match self {
            Codec::Mp3 => 320,
            Codec::Aac => 192,
        }
    }
}

/// Refuses settings the Windows encoder does not take, with the reason.
pub fn check(codec: Codec, rate: u32, channels: usize, kbps: u32) -> Result<(), String> {
    if !codec.rates().contains(&rate) {
        return Err(format!("{} supports {} Hz, not {rate} Hz (choose an export rate)", codec.label(), list(codec.rates())));
    }
    if !(1..=2).contains(&channels) {
        return Err(format!("{} supports mono or stereo, not {channels} channels (fold the mix down to stereo)", codec.label()));
    }
    if !codec.bitrates().contains(&kbps) {
        return Err(format!("{} supports {} kbps, not {kbps}", codec.label(), list(codec.bitrates())));
    }
    Ok(())
}

fn list(v: &[u32]) -> String {
    v.iter().map(u32::to_string).collect::<Vec<_>>().join(", ")
}

/// Whether this computer can encode: Windows with Media Foundation present (Windows N/KN
/// editions need the Media Feature Pack). Media Foundation is loaded at run time, never linked,
/// so its absence never stops the app from starting.
pub fn available() -> bool {
    #[cfg(windows)]
    {
        ffi::present()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Encodes interleaved 16-bit `pcm` (`channels` × frames) to `path` (its extension must be the
/// codec's). Checks [`check`] first. Returns an error (and may leave a partial file at `path`,
/// which the caller removes) when the encoder fails or `cancel` is set.
pub fn encode_file(path: &Path, codec: Codec, rate: u32, channels: usize, kbps: u32, pcm: &[i16], cancel: &AtomicBool) -> Result<(), String> {
    check(codec, rate, channels, kbps)?;
    #[cfg(windows)]
    {
        ffi::encode(path, codec, rate, channels, kbps, pcm, cancel)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, pcm, cancel);
        Err(format!("{} export uses the encoder built into Windows (Media Foundation); it is not available on this platform", codec.label()))
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod ffi;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_checked_against_the_windows_encoders() {
        assert_eq!(Codec::for_name("MP3"), Some(Codec::Mp3));
        assert_eq!(Codec::for_name(".m4a"), Some(Codec::Aac));
        assert_eq!(Codec::for_name("aac"), Some(Codec::Aac));
        assert_eq!(Codec::for_name("ogg"), None);
        assert!(check(Codec::Mp3, 48_000, 2, 320).is_ok());
        assert!(check(Codec::Mp3, 44_100, 1, 128).is_ok());
        assert!(check(Codec::Mp3, 96_000, 2, 320).unwrap_err().contains("44100"));
        assert!(check(Codec::Mp3, 48_000, 6, 320).unwrap_err().contains("fold"));
        assert!(check(Codec::Mp3, 48_000, 2, 96).unwrap_err().contains("128"));
        assert!(check(Codec::Aac, 48_000, 2, 192).is_ok());
        assert!(check(Codec::Aac, 32_000, 2, 192).is_err());
        assert!(check(Codec::Aac, 48_000, 2, 320).is_err());
        if !available() {
            let e = encode_file(Path::new("x.mp3"), Codec::Mp3, 48_000, 2, 320, &[0; 4], &AtomicBool::new(false)).unwrap_err();
            assert!(e.contains("not available on this platform"), "{e}");
        }
    }
}
