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
    Ok(NegotiatedConfig {
        host: dev.host.clone(),
        device: dev.name.clone(),
        sample_rate: req.sample_rate,
        channels: cfg.channels,
        requested_buffer: req.buffer_frames,
        reported_buffer: cfg.buffer,
        clock_lock: ClockLock::Unknown,
    })
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
            out.push(DeviceCaps { host: id.name().to_string(), is_default: default.as_deref() == Some(name.as_str()), name, configs });
        }
    }
    out
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
        buffer_size: match cfg.requested_buffer {
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

    /// Real-driver capability audit on this machine (whatever it has; often nothing in CI).
    #[test]
    fn enumerate_never_panics() {
        let _ = enumerate(Direction::Input);
        let _ = enumerate(Direction::Output);
    }
}
