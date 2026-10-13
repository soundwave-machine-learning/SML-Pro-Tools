//! Windows Media Foundation FFI: an `IMFSinkWriter` from a file URL (the container follows the
//! extension: .mp3 or .m4a), a compressed output stream and a 16-bit PCM input stream. Every
//! `unsafe` block calls Media Foundation through the `windows` bindings with values built here,
//! on a thread where `MFStartup` succeeded, or touches a media buffer only between its own
//! `Lock`/`Unlock`.

use crate::Codec;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Media::MediaFoundation as mf;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::core::HSTRING;

struct Session;
impl Session {
    fn start() -> Result<Session, String> {
        // SAFETY: COM and Media Foundation start-up on this thread; balanced by Drop.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            mf::MFStartup(mf::MF_VERSION, mf::MFSTARTUP_FULL).map_err(|e| format!("Media Foundation is not available: {e}"))?;
        }
        Ok(Session)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: matches the successful MFStartup/CoInitializeEx in `start`.
        unsafe {
            let _ = mf::MFShutdown();
            CoUninitialize();
        }
    }
}

fn audio_type(subtype: &windows::core::GUID, rate: u32, channels: u32, extra: &[(&windows::core::GUID, u32)]) -> Result<mf::IMFMediaType, String> {
    // SAFETY: a new media type object, configured with GUID/UINT32 attributes only.
    unsafe {
        let t = mf::MFCreateMediaType().map_err(|e| e.to_string())?;
        t.SetGUID(&mf::MF_MT_MAJOR_TYPE, &mf::MFMediaType_Audio).map_err(|e| e.to_string())?;
        t.SetGUID(&mf::MF_MT_SUBTYPE, subtype).map_err(|e| e.to_string())?;
        t.SetUINT32(&mf::MF_MT_AUDIO_SAMPLES_PER_SECOND, rate).map_err(|e| e.to_string())?;
        t.SetUINT32(&mf::MF_MT_AUDIO_NUM_CHANNELS, channels).map_err(|e| e.to_string())?;
        for (k, v) in extra {
            t.SetUINT32(*k as *const windows::core::GUID, *v).map_err(|e| e.to_string())?;
        }
        Ok(t)
    }
}

pub fn encode(path: &Path, codec: Codec, rate: u32, channels: usize, kbps: u32, pcm: &[i16], cancel: &AtomicBool) -> Result<(), String> {
    let _mf = Session::start()?;
    let ch = channels as u32;
    let (subtype, extra): (windows::core::GUID, Vec<(&windows::core::GUID, u32)>) = match codec {
        Codec::Mp3 => (mf::MFAudioFormat_MP3, vec![(&mf::MF_MT_AUDIO_AVG_BYTES_PER_SECOND, kbps * 1000 / 8)]),
        Codec::Aac => (
            mf::MFAudioFormat_AAC,
            vec![(&mf::MF_MT_AUDIO_BITS_PER_SAMPLE, 16), (&mf::MF_MT_AUDIO_AVG_BYTES_PER_SECOND, kbps * 1000 / 8), (&mf::MF_MT_AAC_PAYLOAD_TYPE, 0)],
        ),
    };
    let out_type = audio_type(&subtype, rate, ch, &extra)?;
    let block = ch * 2;
    let in_type = audio_type(
        &mf::MFAudioFormat_PCM,
        rate,
        ch,
        &[(&mf::MF_MT_AUDIO_BITS_PER_SAMPLE, 16), (&mf::MF_MT_AUDIO_BLOCK_ALIGNMENT, block), (&mf::MF_MT_AUDIO_AVG_BYTES_PER_SECOND, rate * block)],
    )?;
    // SAFETY: the sink writer is created for a file path we own and used only on this thread;
    // every sample's buffer is written only between Lock and Unlock, within its allocated length.
    unsafe {
        let url = HSTRING::from(path.as_os_str());
        let w = mf::MFCreateSinkWriterFromURL(&url, None, None).map_err(|e| format!("cannot create {}: {e}", path.display()))?;
        let stream = w.AddStream(&out_type).map_err(|e| format!("the Windows {} encoder refused these settings: {e}", codec.label()))?;
        w.SetInputMediaType(stream, &in_type, None).map_err(|e| format!("the Windows {} encoder refused 16-bit PCM input: {e}", codec.label()))?;
        w.BeginWriting().map_err(|e| e.to_string())?;
        let frames_per_chunk = 4096usize;
        let mut t100ns: i64 = 0;
        for chunk in pcm.chunks(frames_per_chunk * channels) {
            if cancel.load(Ordering::Acquire) {
                return Err("export cancelled".into());
            }
            let bytes = chunk.len() * 2;
            let buf = mf::MFCreateMemoryBuffer(bytes as u32).map_err(|e| e.to_string())?;
            let mut ptr: *mut u8 = std::ptr::null_mut();
            buf.Lock(&mut ptr, None, None).map_err(|e| e.to_string())?;
            if ptr.is_null() {
                let _ = buf.Unlock();
                return Err("Media Foundation returned no buffer".into());
            }
            let dst = std::slice::from_raw_parts_mut(ptr, bytes);
            for (d, s) in dst.chunks_exact_mut(2).zip(chunk) {
                d.copy_from_slice(&s.to_le_bytes());
            }
            buf.Unlock().map_err(|e| e.to_string())?;
            buf.SetCurrentLength(bytes as u32).map_err(|e| e.to_string())?;
            let sample = mf::MFCreateSample().map_err(|e| e.to_string())?;
            sample.AddBuffer(&buf).map_err(|e| e.to_string())?;
            let frames = (chunk.len() / channels) as i64;
            let dur = frames * 10_000_000 / i64::from(rate);
            sample.SetSampleTime(t100ns).map_err(|e| e.to_string())?;
            sample.SetSampleDuration(dur).map_err(|e| e.to_string())?;
            w.WriteSample(stream, &sample).map_err(|e| format!("encoding failed: {e}"))?;
            t100ns += dur;
        }
        w.Finalize().map_err(|e| format!("could not finish {}: {e}", path.display()))?;
    }
    Ok(())
}
