//! Windows COM/WASAPI FFI for the native output modes. Every `unsafe` block calls a WASAPI or
//! COM function through the `windows` bindings with arguments built here, or reads memory those
//! functions returned; each block says which invariant it relies on.
use crate::{Mode, NativeCaps, NativeStream, SampleKind, convert_into, frames_to_hns, hns_to_frames, shared_period};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Devices::Properties;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{self, IAudioClient, IAudioClient3, IAudioRenderClient, IMMDevice, IMMDeviceEnumerator};
use windows::Win32::Media::KernelStreaming;
use windows::Win32::Media::Multimedia;
use windows::Win32::System::Com::{self, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ, StructuredStorage};
use windows::Win32::System::Threading::{AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW, WaitForSingleObject};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::core::{HSTRING, Interface};

struct ComGuard;
impl ComGuard {
    fn new() -> ComGuard {
        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
        unsafe {
            let _ = Com::CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        ComGuard
    }
}
impl Drop for ComGuard {
    fn drop(&mut self) {
        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
        unsafe { Com::CoUninitialize() };
    }
}

fn friendly_name(d: &IMMDevice) -> Option<String> {
    // SAFETY: the property store is read on a COM-initialised thread; the PROPVARIANT is only read
    // as a wide string after checking its type is VT_LPWSTR, the string is NUL-terminated by
    // contract, and the variant is cleared exactly once afterwards (as cpal does).
    unsafe {
        let store = d.OpenPropertyStore(STGM_READ).ok()?;
        let mut v = store.GetValue(&Properties::DEVPKEY_Device_FriendlyName as *const _ as *const _).ok()?;
        let raw = &v.as_raw().Anonymous.Anonymous;
        let out = if raw.vt == VT_LPWSTR.0 {
            let p = *(&raw.Anonymous as *const _ as *const *const u16);
            let mut n = 0;
            while *p.offset(n) != 0 {
                n += 1;
            }
            Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, n as usize)))
        } else {
            None
        };
        let _ = StructuredStorage::PropVariantClear(&mut v);
        out
    }
}

fn find_device(name: &str) -> Result<IMMDevice, String> {
    // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
    unsafe {
        let e: IMMDeviceEnumerator = Com::CoCreateInstance(&Audio::MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        let list = e.EnumAudioEndpoints(Audio::eRender, Audio::DEVICE_STATE_ACTIVE).map_err(|e| e.to_string())?;
        let n = list.GetCount().map_err(|e| e.to_string())?;
        for i in 0..n {
            if let Ok(d) = list.Item(i)
                && friendly_name(&d).as_deref() == Some(name)
            {
                return Ok(d);
            }
        }
        Err(format!("output device `{name}` was not found"))
    }
}

fn client(d: &IMMDevice) -> Result<IAudioClient, String> {
    // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
    unsafe { d.Activate::<IAudioClient>(CLSCTX_ALL, None).map_err(|e| e.to_string()) }
}

fn format(rate: u32, channels: u16, kind: SampleKind) -> Audio::WAVEFORMATEXTENSIBLE {
    let bytes = kind.bytes() as u16;
    let container_bits = 8 * bytes;
    let valid_bits = match kind {
        SampleKind::F32 | SampleKind::I32 => 32,
        SampleKind::I24 => 24,
        SampleKind::I16 => 16,
    };
    let block = channels * bytes;
    let ex = Audio::WAVEFORMATEX {
        wFormatTag: KernelStreaming::WAVE_FORMAT_EXTENSIBLE as u16,
        nChannels: channels,
        nSamplesPerSec: rate,
        nAvgBytesPerSec: rate * u32::from(block),
        nBlockAlign: block,
        wBitsPerSample: container_bits,
        cbSize: (std::mem::size_of::<Audio::WAVEFORMATEXTENSIBLE>() - std::mem::size_of::<Audio::WAVEFORMATEX>()) as u16,
    };
    Audio::WAVEFORMATEXTENSIBLE {
        Format: ex,
        Samples: Audio::WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: valid_bits },
        dwChannelMask: if channels == 2 {
            0x3 /* SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT */
        } else {
            KernelStreaming::KSAUDIO_SPEAKER_DIRECTOUT
        },
        SubFormat: if kind == SampleKind::F32 { Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT } else { KernelStreaming::KSDATAFORMAT_SUBTYPE_PCM },
    }
}

fn exclusive_kind(c: &IAudioClient, rate: u32, channels: u16) -> Option<SampleKind> {
    SampleKind::EXCLUSIVE_ORDER.into_iter().find(|k| {
        let f = format(rate, channels, *k);
        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
        unsafe { c.IsFormatSupported(Audio::AUDCLNT_SHAREMODE_EXCLUSIVE, &f.Format, None).is_ok() }
    })
}

/// The device's shared mix format (rate, channels).
fn mix_format(c: &IAudioClient) -> Result<(u32, u16, *mut Audio::WAVEFORMATEX), String> {
    // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
    unsafe {
        let p = c.GetMixFormat().map_err(|e| e.to_string())?;
        Ok(((*p).nSamplesPerSec, (*p).nChannels, p))
    }
}

pub fn caps(device: &str, mode: Mode, rate: u32) -> Result<NativeCaps, String> {
    let _com = ComGuard::new();
    let d = find_device(device)?;
    let c = client(&d)?;
    // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
    unsafe {
        match mode {
            Mode::Exclusive => {
                let (_, mix_ch, mp) = mix_format(&c)?;
                Com::CoTaskMemFree(Some(mp as *const _));
                let kind = exclusive_kind(&c, rate, mix_ch).ok_or_else(|| format!("`{device}` has no exclusive-mode format at {rate} Hz"))?;
                let (mut def, mut min) = (0i64, 0i64);
                c.GetDevicePeriod(Some(&mut def), Some(&mut min)).map_err(|e| e.to_string())?;
                Ok(NativeCaps {
                    rate,
                    channels: mix_ch,
                    min_frames: hns_to_frames(min, rate).max(1),
                    max_frames: hns_to_frames(def, rate).max(hns_to_frames(min, rate)).max(1) * 4,
                    step_frames: 1,
                    sample_kind: kind,
                })
            }
            Mode::LowLatencyShared => {
                let c3: IAudioClient3 =
                    c.cast().map_err(|_| "this Windows version or driver has no IAudioClient3 (low-latency shared mode)".to_string())?;
                let (mix_rate, mix_ch, mp) = mix_format(&c)?;
                let (mut def, mut fund, mut min, mut max) = (0u32, 0u32, 0u32, 0u32);
                let r = c3.GetSharedModeEnginePeriod(mp, &mut def, &mut fund, &mut min, &mut max);
                Com::CoTaskMemFree(Some(mp as *const _));
                r.map_err(|e| e.to_string())?;
                Ok(NativeCaps {
                    rate: mix_rate,
                    channels: mix_ch,
                    min_frames: min,
                    max_frames: max,
                    step_frames: fund.max(1),
                    sample_kind: SampleKind::F32,
                })
            }
        }
    }
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Opens the stream on its own thread; returns once it is running (or failed to start).
pub fn open(
    device: &str,
    mode: Mode,
    rate: u32,
    channels: u16,
    frames: Option<u32>,
    on_lost: Box<dyn Fn(u8) + Send>,
    mut render: Box<dyn FnMut(&mut [f32], usize) + Send>,
) -> Result<NativeStream, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(u32, u32, u16, SampleKind, Option<u32>), String>>();
    let name = device.to_string();
    let stop2 = Arc::clone(&stop);
    let thread = std::thread::Builder::new()
        .name("soundcraft-wasapi".into())
        .spawn(move || {
            let _com = ComGuard::new();
            let setup = || -> Result<(IAudioClient, IAudioRenderClient, Handle, u32, u32, u16, SampleKind, Option<u32>), String> {
                let d = find_device(&name)?;
                // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                let event = Handle(unsafe { CreateEventW(None, false, false, None).map_err(|e| e.to_string())? });
                match mode {
                    Mode::Exclusive => {
                        let c = client(&d)?;
                        let kind = exclusive_kind(&c, rate, channels)
                            .ok_or_else(|| format!("`{name}` has no exclusive-mode format at {rate} Hz with {channels} channels"))?;
                        let f = format(rate, channels, kind);
                        let (mut def, mut min) = (0i64, 0i64);
                        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                        unsafe { c.GetDevicePeriod(Some(&mut def), Some(&mut min)).map_err(|e| e.to_string())? };
                        let mut period = frames.map_or(def, |n| frames_to_hns(n, rate)).max(min);
                        let flags = Audio::AUDCLNT_STREAMFLAGS_EVENTCALLBACK;
                        let mut c = c;
                        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                        let r = unsafe { c.Initialize(Audio::AUDCLNT_SHAREMODE_EXCLUSIVE, flags, period, period, &f.Format, None) };
                        if let Err(e) = r {
                            if e.code() != Audio::AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED {
                                return Err(format!("exclusive mode refused: {e}"));
                            }
                            // Microsoft's alignment procedure: the driver's buffer size, a
                            // fresh client, the period recomputed from it.
                            // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                            let aligned = unsafe { c.GetBufferSize().map_err(|e| e.to_string())? };
                            period = frames_to_hns(aligned, rate);
                            c = client(&d)?;
                            // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                            unsafe {
                                c.Initialize(Audio::AUDCLNT_SHAREMODE_EXCLUSIVE, flags, period, period, &f.Format, None)
                                    .map_err(|e| format!("exclusive mode refused: {e}"))?
                            };
                        }
                        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                        unsafe { c.SetEventHandle(event.0).map_err(|e| e.to_string())? };
                        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                        let buf = unsafe { c.GetBufferSize().map_err(|e| e.to_string())? };
                        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                        let lat = unsafe { c.GetStreamLatency().ok().map(|l| hns_to_frames(l, rate)) };
                        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                        let rc: IAudioRenderClient = unsafe { c.GetService().map_err(|e| e.to_string())? };
                        Ok((c, rc, event, buf, rate, channels, kind, lat))
                    }
                    Mode::LowLatencyShared => {
                        let c = client(&d)?;
                        let c3: IAudioClient3 = c.cast().map_err(|_| "no IAudioClient3 (low-latency shared mode) on this system".to_string())?;
                        let (mix_rate, mix_ch, mp) = mix_format(&c)?;
                        let (mut def, mut fund, mut min, mut max) = (0u32, 0u32, 0u32, 0u32);
                        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                        let r = unsafe { c3.GetSharedModeEnginePeriod(mp, &mut def, &mut fund, &mut min, &mut max) };
                        if let Err(e) = r {
                            // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                            unsafe { Com::CoTaskMemFree(Some(mp as *const _)) };
                            return Err(e.to_string());
                        }
                        let period = shared_period(frames, fund, min, max);
                        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                        let r = unsafe { c3.InitializeSharedAudioStream(Audio::AUDCLNT_STREAMFLAGS_EVENTCALLBACK, period, mp, None) };
                        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                        unsafe { Com::CoTaskMemFree(Some(mp as *const _)) };
                        r.map_err(|e| format!("low-latency shared mode refused: {e}"))?;
                        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                        unsafe { c.SetEventHandle(event.0).map_err(|e| e.to_string())? };
                        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                        let lat = unsafe { c.GetStreamLatency().ok().map(|l| hns_to_frames(l, mix_rate)) };
                        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                        let rc: IAudioRenderClient = unsafe { c.GetService().map_err(|e| e.to_string())? };
                        Ok((c, rc, event, period, mix_rate, mix_ch, SampleKind::F32, lat))
                    }
                }
            };
            let (c, rc, event, period, out_rate, out_ch, kind, lat) = match setup() {
                Ok(v) => v,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            // Buffers sized once, before the stream runs.
            // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
            let bufsize = unsafe { c.GetBufferSize().unwrap_or(period) } as usize;
            let mut scratch = vec![0.0f32; bufsize * usize::from(out_ch)];
            let mut task = 0u32;
            // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
            let mmcss = unsafe { AvSetMmThreadCharacteristicsW(&HSTRING::from("Pro Audio"), &mut task).ok() };
            // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
            if unsafe { c.Start() }.is_err() {
                let _ = ready_tx.send(Err("the stream could not start".into()));
                return;
            }
            let _ = ready_tx.send(Ok((period, out_rate, out_ch, kind, lat)));
            let exclusive = mode == Mode::Exclusive;
            while !stop2.load(Ordering::Acquire) {
                // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                if unsafe { WaitForSingleObject(event.0, 200) } != WAIT_OBJECT_0 {
                    continue;
                }
                let avail = if exclusive {
                    bufsize
                } else {
                    // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                    match unsafe { c.GetCurrentPadding() } {
                        Ok(p) => bufsize.saturating_sub(p as usize),
                        Err(_) => {
                            on_lost(1);
                            break;
                        }
                    }
                };
                if avail == 0 {
                    continue;
                }
                let n = avail * usize::from(out_ch);
                let Some(dst) = scratch.get_mut(..n) else { continue };
                render(dst, usize::from(out_ch));
                // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                match unsafe { rc.GetBuffer(avail as u32) } {
                    Ok(p) => {
                        // SAFETY: GetBuffer(avail) returned a writable buffer of `avail` frames of the
                        // initialised format, i.e. avail * channels * kind.bytes() bytes, valid until
                        // ReleaseBuffer; `n` = avail * channels.
                        let bytes = unsafe { std::slice::from_raw_parts_mut(p, n * kind.bytes()) };
                        convert_into(dst, kind, bytes);
                        // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
                        if unsafe { rc.ReleaseBuffer(avail as u32, 0) }.is_err() {
                            on_lost(2);
                            break;
                        }
                    }
                    Err(e) => {
                        let code = if e.code() == Audio::AUDCLNT_E_DEVICE_INVALIDATED { 1 } else { 2 };
                        on_lost(code);
                        break;
                    }
                }
            }
            // SAFETY: a WASAPI/COM call through the `windows` bindings on this thread (COM initialised by ComGuard); pointers passed are to live locals or to memory WASAPI returned for this call.
            unsafe {
                let _ = c.Stop();
                if let Some(h) = mmcss {
                    let _ = AvRevertMmThreadCharacteristics(h);
                }
            }
        })
        .map_err(|e| e.to_string())?;
    match ready_rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok((period, out_rate, out_ch, kind, lat))) => Ok(NativeStream {
            stop,
            thread: Some(thread),
            period_frames: period,
            rate: out_rate,
            channels: out_ch,
            sample_kind: kind,
            stream_latency_frames: lat,
        }),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => {
            stop.store(true, Ordering::Release);
            Err("the native WASAPI stream did not start within 5 s".into())
        }
    }
}
