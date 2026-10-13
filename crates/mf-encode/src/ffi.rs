//! Windows Media Foundation FFI: an `IMFSinkWriter` from a file URL (the container follows the
//! extension: .mp3 or .m4a), a compressed output stream and a 16-bit PCM input stream. Every
//! `unsafe` block calls Media Foundation with values built here, on a thread where `MFStartup`
//! succeeded, or touches a media buffer only between its own `Lock`/`Unlock`.
//!
//! Media Foundation is **loaded at run time** (`LoadLibraryW` + `GetProcAddress`), never linked:
//! Windows N/KN editions without the Media Feature Pack have no `mfplat.dll`, and a link-time
//! import would stop the whole app from starting there. Without it only MP3/AAC export is
//! unavailable, with that reason. COM interface methods go through their vtables (no imports).

use crate::Codec;
use std::ffi::c_void;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Media::MediaFoundation as mf;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::core::{GUID, HRESULT, HSTRING, Interface, PCSTR, PCWSTR, w};

type FnStartup = unsafe extern "system" fn(u32, u32) -> HRESULT;
type FnShutdown = unsafe extern "system" fn() -> HRESULT;
type FnCreate = unsafe extern "system" fn(*mut *mut c_void) -> HRESULT;
type FnCreateBuffer = unsafe extern "system" fn(u32, *mut *mut c_void) -> HRESULT;
type FnSinkWriter = unsafe extern "system" fn(PCWSTR, *mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT;

/// The Media Foundation entry points this crate uses, resolved once.
struct Api {
    startup: FnStartup,
    shutdown: FnShutdown,
    media_type: FnCreate,
    buffer: FnCreateBuffer,
    sample: FnCreate,
    sink_writer: FnSinkWriter,
}

const MISSING: &str =
    "Media Foundation is not installed on this Windows (Windows N/KN editions need the Media Feature Pack from Settings › Apps › Optional features)";

fn api() -> Result<&'static Api, String> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(|| {
        // SAFETY: loading two system DLLs by name and reading exported function addresses; each
        // address is transmuted to the documented signature of that export (mfapi.h,
        // mfreadwrite.h). The modules stay loaded for the life of the process.
        unsafe {
            let plat = LoadLibraryW(w!("mfplat.dll")).ok()?;
            let rw = LoadLibraryW(w!("mfreadwrite.dll")).ok()?;
            let f = |m, n: &[u8]| GetProcAddress(m, PCSTR(n.as_ptr()));
            Some(Api {
                startup: std::mem::transmute::<unsafe extern "system" fn() -> isize, FnStartup>(f(plat, b"MFStartup\0")?),
                shutdown: std::mem::transmute::<unsafe extern "system" fn() -> isize, FnShutdown>(f(plat, b"MFShutdown\0")?),
                media_type: std::mem::transmute::<unsafe extern "system" fn() -> isize, FnCreate>(f(plat, b"MFCreateMediaType\0")?),
                buffer: std::mem::transmute::<unsafe extern "system" fn() -> isize, FnCreateBuffer>(f(plat, b"MFCreateMemoryBuffer\0")?),
                sample: std::mem::transmute::<unsafe extern "system" fn() -> isize, FnCreate>(f(plat, b"MFCreateSample\0")?),
                sink_writer: std::mem::transmute::<unsafe extern "system" fn() -> isize, FnSinkWriter>(f(rw, b"MFCreateSinkWriterFromURL\0")?),
            })
        }
    })
    .as_ref()
    .ok_or_else(|| MISSING.to_string())
}

/// Whether Media Foundation can be loaded on this computer.
pub fn present() -> bool {
    api().is_ok()
}

/// Calls a Media Foundation `Create*(out)` and wraps the result in its interface type.
fn created<T: Interface>(r: HRESULT, raw: *mut c_void) -> Result<T, String> {
    r.ok().map_err(|e| e.to_string())?;
    // SAFETY: on success the factory returned one owned reference to an object of type `T`;
    // `from_raw` takes over that reference (released on drop).
    if raw.is_null() { Err("Media Foundation returned no object".into()) } else { Ok(unsafe { T::from_raw(raw) }) }
}

struct Session(&'static Api);
impl Session {
    fn start() -> Result<Session, String> {
        let api = api()?;
        // SAFETY: COM and Media Foundation start-up on this thread; balanced by Drop.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            if let Err(e) = (api.startup)(mf::MF_VERSION, mf::MFSTARTUP_FULL).ok() {
                CoUninitialize();
                return Err(format!("Media Foundation is not available: {e}"));
            }
        }
        Ok(Session(api))
    }
    fn media_type(&self) -> Result<mf::IMFMediaType, String> {
        let mut raw = std::ptr::null_mut();
        // SAFETY: MFCreateMediaType writes one interface pointer to `raw`.
        let r = unsafe { (self.0.media_type)(&mut raw) };
        created(r, raw)
    }
    fn buffer(&self, bytes: u32) -> Result<mf::IMFMediaBuffer, String> {
        let mut raw = std::ptr::null_mut();
        // SAFETY: MFCreateMemoryBuffer writes one interface pointer to `raw`.
        let r = unsafe { (self.0.buffer)(bytes, &mut raw) };
        created(r, raw)
    }
    fn sample(&self) -> Result<mf::IMFSample, String> {
        let mut raw = std::ptr::null_mut();
        // SAFETY: MFCreateSample writes one interface pointer to `raw`.
        let r = unsafe { (self.0.sample)(&mut raw) };
        created(r, raw)
    }
    fn sink_writer(&self, url: &HSTRING) -> Result<mf::IMFSinkWriter, String> {
        let mut raw = std::ptr::null_mut();
        // SAFETY: `url` is a NUL-terminated wide string that outlives the call; no byte stream
        // and no attributes (null); one interface pointer is written to `raw`.
        let r = unsafe { (self.0.sink_writer)(PCWSTR(url.as_ptr()), std::ptr::null_mut(), std::ptr::null_mut(), &mut raw) };
        created(r, raw)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: matches the successful MFStartup/CoInitializeEx in `start`.
        unsafe {
            let _ = (self.0.shutdown)();
            CoUninitialize();
        }
    }
}

fn audio_type(mfs: &Session, subtype: &GUID, rate: u32, channels: u32, extra: &[(&GUID, u32)]) -> Result<mf::IMFMediaType, String> {
    let t = mfs.media_type()?;
    // SAFETY: a new media type object, configured with GUID/UINT32 attributes only.
    unsafe {
        t.SetGUID(&mf::MF_MT_MAJOR_TYPE, &mf::MFMediaType_Audio).map_err(|e| e.to_string())?;
        t.SetGUID(&mf::MF_MT_SUBTYPE, subtype).map_err(|e| e.to_string())?;
        t.SetUINT32(&mf::MF_MT_AUDIO_SAMPLES_PER_SECOND, rate).map_err(|e| e.to_string())?;
        t.SetUINT32(&mf::MF_MT_AUDIO_NUM_CHANNELS, channels).map_err(|e| e.to_string())?;
        for (k, v) in extra {
            t.SetUINT32(*k as *const GUID, *v).map_err(|e| e.to_string())?;
        }
        Ok(t)
    }
}

pub fn encode(path: &Path, codec: Codec, rate: u32, channels: usize, kbps: u32, pcm: &[i16], cancel: &AtomicBool) -> Result<(), String> {
    let mfs = Session::start()?;
    let ch = channels as u32;
    let (subtype, extra): (GUID, Vec<(&GUID, u32)>) = match codec {
        Codec::Mp3 => (mf::MFAudioFormat_MP3, vec![(&mf::MF_MT_AUDIO_AVG_BYTES_PER_SECOND, kbps * 1000 / 8)]),
        Codec::Aac => (
            mf::MFAudioFormat_AAC,
            vec![(&mf::MF_MT_AUDIO_BITS_PER_SAMPLE, 16), (&mf::MF_MT_AUDIO_AVG_BYTES_PER_SECOND, kbps * 1000 / 8), (&mf::MF_MT_AAC_PAYLOAD_TYPE, 0)],
        ),
    };
    let out_type = audio_type(&mfs, &subtype, rate, ch, &extra)?;
    let block = ch * 2;
    let in_type = audio_type(
        &mfs,
        &mf::MFAudioFormat_PCM,
        rate,
        ch,
        &[(&mf::MF_MT_AUDIO_BITS_PER_SAMPLE, 16), (&mf::MF_MT_AUDIO_BLOCK_ALIGNMENT, block), (&mf::MF_MT_AUDIO_AVG_BYTES_PER_SECOND, rate * block)],
    )?;
    // SAFETY: the sink writer is created for a file path we own and used only on this thread;
    // every sample's buffer is written only between Lock and Unlock, within its allocated length.
    unsafe {
        let url = HSTRING::from(path.as_os_str());
        let w = mfs.sink_writer(&url).map_err(|e| format!("cannot create {}: {e}", path.display()))?;
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
            let buf = mfs.buffer(bytes as u32)?;
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
            let sample = mfs.sample()?;
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
