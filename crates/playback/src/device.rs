//! Device selection and buffer negotiation (CL02).
//!
//! [`negotiate`] is a pure function from a [`DeviceRequest`] and the capabilities a backend
//! reports ([`DeviceCaps`]) to the configuration actually used ([`NegotiatedConfig`]) or an
//! explicit error — no silent fallback to the default device, rate or buffer. [`enumerate`] reads
//! real capabilities through cpal (whatever hosts this build enables: ALSA/JACK on Linux,
//! CoreAudio on macOS, WASAPI on Windows; ASIO only when cpal's `asio` feature is compiled in,
//! which this workspace does not do yet). The *effective* callback size is what the stream then
//! delivers ([`crate::clock::StreamClock::observed_block_range`]); it is reported separately
//! from the requested buffer and the backend's supported range.

/// What the user asked for.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct DeviceRequest {
    /// Backend host name (e.g. "ALSA", "WASAPI", "ASIO"); `None` = platform default host.
    pub host: Option<String>,
    /// Device name; `None` = the host's default device.
    pub device: Option<String>,
    pub sample_rate: u32,
    /// Requested callback buffer in frames; `None` = backend default.
    pub buffer_frames: Option<u32>,
    pub channels: u16,
}

/// Supported buffer sizes reported by the backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BufferRange {
    Range { min: u32, max: u32 },
    Unknown,
}

/// One supported configuration range of a device.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigRange {
    pub channels: u16,
    pub min_rate: u32,
    pub max_rate: u32,
    pub buffer: BufferRange,
    pub float: bool,
}

/// A device as a backend reports it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DeviceCaps {
    pub host: String,
    pub name: String,
    pub is_default: bool,
    pub configs: Vec<ConfigRange>,
}

/// Hardware clock lock as reported by the backend (cpal reports none: always Unknown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ClockLock {
    Locked,
    Unlocked,
    Unknown,
}

/// The configuration the stream is opened with.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NegotiatedConfig {
    pub host: String,
    pub device: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// Buffer requested from the backend (`None` = backend default).
    pub requested_buffer: Option<u32>,
    /// Buffer range the backend reports for this config.
    pub reported_buffer: BufferRange,
    pub clock_lock: ClockLock,
    /// The buffer actually asked of the backend (`None` = its default). Differs from
    /// `requested_buffer` when the host cannot set the buffer (HF07: WASAPI shared mode).
    #[serde(default)]
    pub applied_buffer: Option<u32>,
    /// Why the requested buffer was not applied as asked, shown beside the delivered size.
    #[serde(default)]
    pub buffer_note: Option<String>,
    /// Native WASAPI modes: the stream latency the driver reports (`GetStreamLatency`), frames.
    #[serde(default)]
    pub driver_latency_frames: Option<u32>,
    /// Native WASAPI modes: the device sample format used ("32-bit float", "24-bit integer"…).
    #[serde(default)]
    pub sample_format: Option<String>,
}

/// How a host honours a requested buffer size (HF07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BufferControl {
    /// The driver takes the requested size (ASIO, CoreAudio, ALSA, JACK…), within its range.
    Settable,
    /// The operating system's audio engine sets the period; a request cannot change it. cpal 0.16
    /// opens WASAPI in shared, event-driven mode, where Windows calls back once per engine period
    /// (usually 10 ms = 480 frames at 48 kHz) whatever buffer is asked for.
    EnginePeriod,
}

/// How `host` honours buffer requests in this build. Only plain WASAPI (cpal's shared mode) has
/// its period set by Windows; the native exclusive and low-latency modes take a requested period.
pub fn buffer_control(host: &str) -> BufferControl {
    if host.eq_ignore_ascii_case("WASAPI") { BufferControl::EnginePeriod } else { BufferControl::Settable }
}

/// A driver type offered in Audio Setup, with whether this build can use it and why not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DriverType {
    /// Host name as devices report it ("WASAPI", "ASIO", "ALSA", …).
    pub name: String,
    /// What it is, for the menu ("Windows shared mode", …).
    pub mode: String,
    pub available: bool,
    pub buffer: BufferControl,
    /// Why it is unavailable, or what to know about it.
    pub note: String,
}

/// The ASIO build requirement, stated once.
pub const ASIO_BLOCKER: &str = "ASIO is not in this build. It needs Steinberg's ASIO SDK, used under a licence the owner must accept (Steinberg's own licence or GPLv3), plus the MSVC toolchain and LLVM at build time; this candidate is cross-built with the GNU toolchain. The interface's ASIO driver is what delivers 64/128-frame buffers.";
/// What WASAPI exclusive mode means for the user (HF07: native backend, `soundcraft-wasapi-native`).
pub const WASAPI_EXCLUSIVE_NOTE: &str = "SML Studio owns the device: other programs cannot play through it while it is open. The buffer is the size you choose, aligned to what the driver accepts. Output only; recording stays on the shared input.";
/// What WASAPI low-latency shared mode means for the user.
pub const WASAPI_LOW_LATENCY_NOTE: &str = "Shared with other programs, at a smaller period when the driver offers one (IAudioClient3). Runs at the device's own rate and channel count; the buffer is rounded to the driver's period step.";

/// The driver types to offer: every host compiled into this build (`compiled`), plus, on
/// Windows, the ones this build cannot use with the reason (pure, for tests).
pub fn driver_types_for(compiled: &[String], windows: bool) -> Vec<DriverType> {
    let mut compiled = compiled.to_vec();
    if windows && compiled.iter().any(|h| h.eq_ignore_ascii_case("WASAPI")) {
        // HF07: the native modes are compiled into every Windows build.
        compiled.push(crate::wasapi_native::EXCLUSIVE_HOST.to_string());
        compiled.push(crate::wasapi_native::LOW_LATENCY_HOST.to_string());
    }
    let mut out: Vec<DriverType> = compiled
        .iter()
        .map(|h| {
            let buffer = buffer_control(h);
            let (mode, note) = match h.to_ascii_uppercase().as_str() {
                "WASAPI" => (
                    "Windows audio, shared mode (standard)",
                    "Windows mixes this app with others. In this mode the buffer is the Windows audio engine's period (often 10 ms = 480 frames at 48 kHz on this backend); a size cannot be requested here. For a smaller buffer choose WASAPI Low Latency or WASAPI Exclusive.",
                ),
                "WASAPI EXCLUSIVE" => ("Windows audio, exclusive mode", WASAPI_EXCLUSIVE_NOTE),
                "WASAPI LOW LATENCY" => ("Windows audio, low-latency shared mode", WASAPI_LOW_LATENCY_NOTE),
                "ASIO" => ("manufacturer's ASIO driver", "The driver sets the buffer; its control panel is opened from the driver's own utility."),
                "COREAUDIO" => ("macOS Core Audio", ""),
                "ALSA" => ("Linux ALSA", ""),
                "JACK" => ("JACK server", "The JACK server sets the buffer."),
                _ => ("", ""),
            };
            DriverType { name: h.clone(), mode: mode.into(), available: true, buffer, note: note.into() }
        })
        .collect();
    if windows && !compiled.iter().any(|h| h.eq_ignore_ascii_case("ASIO")) {
        out.push(DriverType {
            name: "ASIO".into(),
            mode: "manufacturer's ASIO driver".into(),
            available: false,
            buffer: BufferControl::Settable,
            note: ASIO_BLOCKER.into(),
        });
    }
    out
}

/// [`driver_types_for`] this build and platform.
pub fn driver_types() -> Vec<DriverType> {
    #[cfg(not(target_arch = "wasm32"))]
    let compiled: Vec<String> = cpal::available_hosts().into_iter().map(|h| h.name().to_string()).collect();
    #[cfg(target_arch = "wasm32")]
    let compiled: Vec<String> = Vec::new();
    driver_types_for(&compiled, cfg!(windows))
}

/// Buffer duration in milliseconds (a buffer's length, not round-trip latency).
pub fn buffer_ms(frames: u32, rate: u32) -> f64 {
    f64::from(frames) * 1000.0 / f64::from(rate.max(1))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NegotiationError {
    NoSuchHost(String),
    NoSuchDevice(String),
    UnsupportedRate { device: String, rate: u32 },
    UnsupportedBuffer { device: String, frames: u32, min: u32, max: u32 },
    UnsupportedChannels { device: String, channels: u16 },
    NoFloatFormat(String),
}

impl std::fmt::Display for NegotiationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NegotiationError::NoSuchHost(h) => write!(f, "audio host `{h}` is not available in this build"),
            NegotiationError::NoSuchDevice(d) => write!(f, "audio device `{d}` was not found"),
            NegotiationError::UnsupportedRate { device, rate } => write!(f, "`{device}` does not support {rate} Hz"),
            NegotiationError::UnsupportedBuffer { device, frames, min, max } => {
                write!(f, "`{device}` supports buffers {min}..={max} frames, not {frames}")
            }
            NegotiationError::UnsupportedChannels { device, channels } => write!(f, "`{device}` has no {channels}-channel float configuration"),
            NegotiationError::NoFloatFormat(d) => write!(f, "`{d}` offers no 32-bit float format"),
        }
    }
}

/// Chooses the exact configuration for `req` among `devices`, or explains why it cannot.
pub fn negotiate(req: &DeviceRequest, devices: &[DeviceCaps]) -> Result<NegotiatedConfig, NegotiationError> {
    let on_host: Vec<&DeviceCaps> = match &req.host {
        Some(h) => {
            let v: Vec<&DeviceCaps> = devices.iter().filter(|d| d.host.eq_ignore_ascii_case(h)).collect();
            if v.is_empty() {
                return Err(NegotiationError::NoSuchHost(h.clone()));
            }
            v
        }
        None => devices.iter().collect(),
    };
    let dev = match &req.device {
        Some(n) => on_host.iter().find(|d| &d.name == n).copied().ok_or_else(|| NegotiationError::NoSuchDevice(n.clone()))?,
        None => on_host.iter().find(|d| d.is_default).or(on_host.first()).copied().ok_or_else(|| NegotiationError::NoSuchDevice("default".into()))?,
    };
    let float: Vec<&ConfigRange> = dev.configs.iter().filter(|c| c.float).collect();
    if float.is_empty() {
        return Err(NegotiationError::NoFloatFormat(dev.name.clone()));
    }
    let rated: Vec<&&ConfigRange> = float.iter().filter(|c| c.min_rate <= req.sample_rate && c.max_rate >= req.sample_rate).collect();
    if rated.is_empty() {
        return Err(NegotiationError::UnsupportedRate { device: dev.name.clone(), rate: req.sample_rate });
    }
    let want_ch = req.channels.max(1);
    // Exact channel count first, else the smallest config wider than requested.
    let mut fits: Vec<&&&ConfigRange> = rated.iter().filter(|c| c.channels >= want_ch).collect();
    fits.sort_by_key(|c| c.channels);
    let cfg = fits.first().ok_or(NegotiationError::UnsupportedChannels { device: dev.name.clone(), channels: want_ch })?;
    if let (Some(fr), BufferRange::Range { min, max }) = (req.buffer_frames, cfg.buffer)
        && (fr < min || fr > max)
    {
        return Err(NegotiationError::UnsupportedBuffer { device: dev.name.clone(), frames: fr, min, max });
    }
    // HF07: a host whose engine sets the period cannot take a fixed buffer. Say so and open
    // with the backend default instead of passing a request that is silently not honoured.
    let (applied_buffer, buffer_note) = match (req.buffer_frames, buffer_control(&dev.host)) {
        (Some(fr), BufferControl::EnginePeriod) => (
            None,
            Some(format!(
                "{} shared mode cannot use the requested {fr} frames: the Windows audio engine sets the buffer in this mode. Choose WASAPI Low Latency or WASAPI Exclusive for a smaller buffer.",
                dev.host
            )),
        ),
        (b, _) => (b, None),
    };
    Ok(NegotiatedConfig {
        host: dev.host.clone(),
        device: dev.name.clone(),
        sample_rate: req.sample_rate,
        channels: cfg.channels,
        requested_buffer: req.buffer_frames,
        reported_buffer: cfg.buffer,
        clock_lock: ClockLock::Unknown,
        applied_buffer,
        buffer_note,
        driver_latency_frames: None,
        sample_format: None,
    })
}

/// The request for recording from `dev`: every physical input the device offers at `rate` (the
/// widest float configuration), so any track input channel can be captured. `None` when the
/// device has no float configuration at that rate.
pub fn input_request(dev: &DeviceCaps, rate: u32, buffer_frames: Option<u32>) -> Option<DeviceRequest> {
    let channels = dev.configs.iter().filter(|c| c.float && c.min_rate <= rate && c.max_rate >= rate).map(|c| c.channels).max()?;
    Some(DeviceRequest { host: Some(dev.host.clone()), device: Some(dev.name.clone()), sample_rate: rate, buffer_frames, channels })
}

/// Input or output side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

/// Every device of every host compiled into this build, with its float configurations.
pub fn enumerate(dir: Direction) -> Vec<DeviceCaps> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let mut out = Vec::new();
    for id in cpal::available_hosts() {
        let Ok(host) = cpal::host_from_id(id) else { continue };
        #[allow(deprecated)]
        let default = match dir {
            Direction::Input => host.default_input_device().and_then(|d| d.name().ok()),
            Direction::Output => host.default_output_device().and_then(|d| d.name().ok()),
        };
        let devices = match dir {
            Direction::Input => host.input_devices().map(|d| d.collect::<Vec<_>>()),
            Direction::Output => host.output_devices().map(|d| d.collect::<Vec<_>>()),
        };
        let Ok(devices) = devices else { continue };
        for d in devices {
            #[allow(deprecated)]
            let Ok(name) = d.name() else { continue };
            let ranges: Vec<cpal::SupportedStreamConfigRange> = match dir {
                Direction::Input => d.supported_input_configs().map(|c| c.collect()).unwrap_or_default(),
                Direction::Output => d.supported_output_configs().map(|c| c.collect()).unwrap_or_default(),
            };
            let configs = ranges
                .iter()
                .map(|c| ConfigRange {
                    channels: c.channels(),
                    min_rate: c.min_sample_rate().0,
                    max_rate: c.max_sample_rate().0,
                    buffer: match c.buffer_size() {
                        cpal::SupportedBufferSize::Range { min, max } => BufferRange::Range { min: *min, max: *max },
                        cpal::SupportedBufferSize::Unknown => BufferRange::Unknown,
                    },
                    float: c.sample_format() == cpal::SampleFormat::F32,
                })
                .collect();
            #[cfg(windows)]
            let native_name = (dir == Direction::Output && id.name().eq_ignore_ascii_case("WASAPI")).then(|| name.clone());
            out.push(DeviceCaps { host: id.name().to_string(), is_default: default.as_deref() == Some(name.as_str()), name, configs });
            #[cfg(windows)]
            if let Some(n) = native_name {
                out.extend(native_caps(&n));
            }
        }
    }
    out
}

/// HF07: the native WASAPI modes of one output device, as the driver reports them: exclusive mode
/// at each common rate the device accepts, low-latency shared mode at the device's mix rate.
#[cfg(windows)]
fn native_caps(name: &str) -> Vec<DeviceCaps> {
    use crate::wasapi_native::{self as wn, Mode};
    let mut v = Vec::new();
    let excl: Vec<ConfigRange> = [44_100u32, 48_000, 88_200, 96_000]
        .iter()
        .filter_map(|r| wn::caps(name, Mode::Exclusive, *r).ok())
        .map(|c| ConfigRange {
            channels: c.channels,
            min_rate: c.rate,
            max_rate: c.rate,
            buffer: BufferRange::Range { min: c.min_frames, max: c.max_frames },
            float: true,
        })
        .collect();
    if !excl.is_empty() {
        v.push(DeviceCaps { host: wn::EXCLUSIVE_HOST.into(), name: name.into(), is_default: false, configs: excl });
    }
    if let Ok(c) = wn::caps(name, Mode::LowLatencyShared, 48_000) {
        v.push(DeviceCaps {
            host: wn::LOW_LATENCY_HOST.into(),
            name: name.into(),
            is_default: false,
            configs: vec![ConfigRange {
                channels: c.channels,
                min_rate: c.rate,
                max_rate: c.rate,
                buffer: BufferRange::Range { min: c.min_frames, max: c.max_frames },
                float: true,
            }],
        });
    }
    v
}

/// Opens the negotiated cpal device (by host and exact name). Errors instead of substituting
/// another device.
pub fn find_device(cfg: &NegotiatedConfig, dir: Direction) -> Result<cpal::Device, String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let id = cpal::available_hosts()
        .into_iter()
        .find(|h| h.name().eq_ignore_ascii_case(&cfg.host))
        .ok_or_else(|| format!("host {} unavailable", cfg.host))?;
    let host = cpal::host_from_id(id).map_err(|e| e.to_string())?;
    let mut devices = match dir {
        Direction::Input => host.input_devices().map_err(|e| e.to_string())?.collect::<Vec<_>>(),
        Direction::Output => host.output_devices().map_err(|e| e.to_string())?.collect::<Vec<_>>(),
    };
    #[allow(deprecated)]
    let pos = devices.iter().position(|d| d.name().ok().as_deref() == Some(cfg.device.as_str()));
    match pos {
        Some(i) => Ok(devices.swap_remove(i)),
        None => Err(format!("device `{}` disappeared", cfg.device)),
    }
}

/// The cpal stream config for a negotiated configuration (fixed buffer when requested).
pub fn stream_config(cfg: &NegotiatedConfig) -> cpal::StreamConfig {
    cpal::StreamConfig {
        channels: cfg.channels,
        sample_rate: cpal::SampleRate(cfg.sample_rate),
        buffer_size: match cfg.applied_buffer {
            Some(n) => cpal::BufferSize::Fixed(n),
            None => cpal::BufferSize::Default,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iface() -> Vec<DeviceCaps> {
        vec![
            DeviceCaps {
                host: "ASIO".into(),
                name: "Studio Interface".into(),
                is_default: false,
                configs: vec![ConfigRange {
                    channels: 2,
                    min_rate: 44_100,
                    max_rate: 96_000,
                    buffer: BufferRange::Range { min: 32, max: 2048 },
                    float: true,
                }],
            },
            DeviceCaps {
                host: "WASAPI".into(),
                name: "Speakers".into(),
                is_default: true,
                configs: vec![
                    ConfigRange { channels: 2, min_rate: 48_000, max_rate: 48_000, buffer: BufferRange::Unknown, float: true },
                    ConfigRange { channels: 8, min_rate: 48_000, max_rate: 48_000, buffer: BufferRange::Unknown, float: true },
                ],
            },
            DeviceCaps {
                host: "WASAPI".into(),
                name: "Int16 only".into(),
                is_default: false,
                configs: vec![ConfigRange { channels: 2, min_rate: 48_000, max_rate: 48_000, buffer: BufferRange::Unknown, float: false }],
            },
        ]
    }

    fn req(host: Option<&str>, dev: Option<&str>, rate: u32, buf: Option<u32>) -> DeviceRequest {
        DeviceRequest { host: host.map(Into::into), device: dev.map(Into::into), sample_rate: rate, buffer_frames: buf, channels: 2 }
    }

    /// CL-T2: the requested device/rate/buffer is honoured exactly and reported back.
    #[test]
    fn negotiates_the_requested_device_rate_and_buffer() {
        for (rate, buf) in [(48_000, 64), (48_000, 128), (44_100, 256), (48_000, 512), (44_100, 1024)] {
            let c = negotiate(&req(Some("asio"), Some("Studio Interface"), rate, Some(buf)), &iface()).unwrap();
            assert_eq!((c.device.as_str(), c.sample_rate, c.requested_buffer), ("Studio Interface", rate, Some(buf)));
            assert_eq!(c.reported_buffer, BufferRange::Range { min: 32, max: 2048 });
            assert_eq!(c.clock_lock, ClockLock::Unknown, "never claims lock it was not told about");
        }
    }

    /// Unsupported values fail explicitly — no substitution of another device, rate or buffer.
    #[test]
    fn unsupported_requests_fail_without_substitution() {
        let d = iface();
        assert_eq!(
            negotiate(&req(Some("ASIO"), Some("Studio Interface"), 48_000, Some(16)), &d).unwrap_err(),
            NegotiationError::UnsupportedBuffer { device: "Studio Interface".into(), frames: 16, min: 32, max: 2048 }
        );
        assert!(matches!(negotiate(&req(Some("WASAPI"), Some("Speakers"), 44_100, None), &d), Err(NegotiationError::UnsupportedRate { .. })));
        assert!(matches!(negotiate(&req(Some("CoreAudio"), None, 48_000, None), &d), Err(NegotiationError::NoSuchHost(_))));
        assert!(matches!(negotiate(&req(None, Some("Unplugged"), 48_000, None), &d), Err(NegotiationError::NoSuchDevice(_))));
        assert!(matches!(negotiate(&req(Some("WASAPI"), Some("Int16 only"), 48_000, None), &d), Err(NegotiationError::NoFloatFormat(_))));
    }

    #[test]
    fn default_device_and_channel_choice() {
        let c = negotiate(&req(Some("WASAPI"), None, 48_000, None), &iface()).unwrap();
        assert_eq!(c.device, "Speakers");
        assert_eq!(c.channels, 2, "smallest config that fits");
        assert_eq!(c.reported_buffer, BufferRange::Unknown, "unknown stays unknown");
        let mut r = req(Some("WASAPI"), None, 48_000, None);
        r.channels = 6;
        assert_eq!(negotiate(&r, &iface()).unwrap().channels, 8);
        r.channels = 10;
        assert!(matches!(negotiate(&r, &iface()), Err(NegotiationError::UnsupportedChannels { .. })));
    }

    /// HF07: WASAPI (shared, as cpal 0.16 opens it) cannot take a fixed buffer. The owner asked
    /// for 128 frames and got 480 with no warning; the request is now opened with the backend
    /// default and the reason is reported, while a settable host still gets exactly what was asked.
    #[test]
    fn wasapi_shared_never_silently_accepts_a_fixed_buffer() {
        let c = negotiate(&req(Some("WASAPI"), Some("Speakers"), 48_000, Some(128)), &iface()).unwrap();
        assert_eq!((c.requested_buffer, c.applied_buffer), (Some(128), None));
        // Hotfix 03: the note points to the native low-latency modes (it pointed to ASIO before
        // they existed).
        assert!(c.buffer_note.as_deref().is_some_and(|n| n.contains("128 frames") && n.contains("WASAPI Low Latency")), "{c:?}");
        assert_eq!(stream_config(&c).buffer_size, cpal::BufferSize::Default);
        let a = negotiate(&req(Some("ASIO"), Some("Studio Interface"), 48_000, Some(64)), &iface()).unwrap();
        assert_eq!((a.applied_buffer, a.buffer_note.as_deref()), (Some(64), None));
        assert_eq!(stream_config(&a).buffer_size, cpal::BufferSize::Fixed(64));
        // No buffer asked: nothing to warn about.
        let d = negotiate(&req(Some("WASAPI"), None, 48_000, None), &iface()).unwrap();
        assert_eq!(d.buffer_note, None);
        assert!((buffer_ms(64, 48_000) - 1.333).abs() < 0.001 && (buffer_ms(128, 48_000) - 2.667).abs() < 0.001);
    }

    /// Driver types: compiled hosts are offered; on Windows the native WASAPI Exclusive and Low
    /// Latency modes (HF07, Hotfix 03) are available with settable buffers, and ASIO (when not
    /// compiled) is listed as unavailable with the exact reason, never as a working choice.
    #[test]
    fn driver_types_state_what_this_build_can_use() {
        let w = driver_types_for(&["WASAPI".to_string()], true);
        let names: Vec<(&str, bool)> = w.iter().map(|d| (d.name.as_str(), d.available)).collect();
        assert_eq!(names, [("WASAPI", true), ("WASAPI Exclusive", true), ("WASAPI Low Latency", true), ("ASIO", false)]);
        assert_eq!(w[0].buffer, BufferControl::EnginePeriod);
        assert_eq!((w[1].buffer, w[2].buffer), (BufferControl::Settable, BufferControl::Settable));
        assert!(w[3].note.contains("Steinberg") && w[1].note.contains("other programs cannot play"));
        assert!(!w[0].note.contains("usually 10 ms"), "no universal claim about Windows' shared period");
        let with_asio = driver_types_for(&["ASIO".to_string(), "WASAPI".to_string()], true);
        assert!(with_asio.iter().any(|d| d.name == "ASIO" && d.available && d.buffer == BufferControl::Settable));
        assert_eq!(driver_types_for(&["ALSA".to_string()], false).len(), 1);
    }

    /// Real-driver capability audit on this machine (whatever it has; often nothing in CI).
    #[test]
    fn enumerate_never_panics() {
        let _ = enumerate(Direction::Input);
        let _ = enumerate(Direction::Output);
    }

    #[test]
    fn input_request_captures_every_input_and_negotiates_exactly() {
        let devs = iface();
        // Widest float config at the rate (8 inputs), never the int16-only device.
        let r = input_request(&devs[1], 48_000, Some(128)).unwrap();
        assert_eq!((r.host.as_deref(), r.device.as_deref(), r.channels, r.buffer_frames), (Some("WASAPI"), Some("Speakers"), 8, Some(128)));
        let n = negotiate(&r, &devs).unwrap();
        assert_eq!((n.device.as_str(), n.channels, n.sample_rate), ("Speakers", 8, 48_000));
        assert!(input_request(&devs[1], 44_100, None).is_none(), "rate the device cannot run");
        assert!(input_request(&devs[2], 48_000, None).is_none(), "no float configuration");
        // A buffer outside the reported range is an error, not a substitution.
        let r = input_request(&devs[0], 48_000, Some(4096)).unwrap();
        assert!(matches!(negotiate(&r, &devs), Err(NegotiationError::UnsupportedBuffer { .. })));
    }
}
