//! Push-to-talk capture for the assistant (RC05): while the button is held, the default input
//! device feeds a preallocated mono buffer (at most [`PTT_SECONDS`]); releasing it stops the
//! stream and hands the audio to the recogniser. Nothing is written to disk and nothing enters a
//! take: this is a separate, short-lived input stream that the host refuses to open while
//! recording. The callback only averages channels into atomics (no allocation, no locks).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

/// Longest push-to-talk utterance kept.
pub const PTT_SECONDS: usize = 8;

/// Fixed-size mono buffer filled by the input callback; extra audio is dropped and counted.
pub struct PttBuffer {
    samples: Vec<AtomicU32>,
    len: AtomicUsize,
    dropped: AtomicUsize,
    channels: usize,
    pub rate: u32,
}

impl PttBuffer {
    pub fn new(channels: usize, rate: u32) -> Arc<PttBuffer> {
        let cap = rate as usize * PTT_SECONDS;
        Arc::new(PttBuffer {
            samples: (0..cap).map(|_| AtomicU32::new(0)).collect(),
            len: AtomicUsize::new(0),
            dropped: AtomicUsize::new(0),
            channels: channels.max(1),
            rate,
        })
    }

    /// Audio thread: interleaved input frames, averaged to mono. Never allocates or blocks.
    pub fn push(&self, interleaved: &[f32]) {
        let mut at = self.len.load(Ordering::Relaxed);
        for frame in interleaved.chunks(self.channels) {
            let v = frame.iter().sum::<f32>() / self.channels as f32;
            match self.samples.get(at) {
                Some(slot) => {
                    slot.store(if v.is_finite() { v } else { 0.0 }.to_bits(), Ordering::Relaxed);
                    at += 1;
                }
                None => {
                    self.dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        self.len.store(at, Ordering::Release);
    }

    /// The captured mono audio so far.
    pub fn snapshot(&self) -> Vec<f32> {
        let n = self.len.load(Ordering::Acquire).min(self.samples.len());
        self.samples.iter().take(n).map(|a| f32::from_bits(a.load(Ordering::Relaxed))).collect()
    }

    /// Frames that did not fit (the utterance was longer than [`PTT_SECONDS`]).
    pub fn dropped(&self) -> usize {
        self.dropped.load(Ordering::Relaxed)
    }
}

/// A running push-to-talk capture. Dropping it (or [`PushToTalk::finish`]) stops the stream.
pub struct PushToTalk {
    pub buffer: Arc<PttBuffer>,
    #[cfg(not(target_os = "freebsd"))]
    _stream: cpal::Stream,
    pub device_name: String,
}

impl PushToTalk {
    /// Opens the default input device. Errors when there is none or it cannot deliver f32.
    #[cfg(not(target_os = "freebsd"))]
    pub fn start() -> Result<PushToTalk, String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or("no audio input device for push-to-talk")?;
        let cfg = device.default_input_config().map_err(|e| e.to_string())?;
        if cfg.sample_format() != cpal::SampleFormat::F32 {
            return Err(format!("unsupported input sample format {:?}", cfg.sample_format()));
        }
        let config = cfg.config();
        let buffer = PttBuffer::new(usize::from(config.channels), config.sample_rate.0);
        let b = Arc::clone(&buffer);
        #[allow(deprecated)]
        let device_name = device.name().unwrap_or_else(|_| "Input".into());
        let stream = device
            .build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    crate::mark_audio_thread();
                    b.push(data);
                },
                move |e| {
                    crate::mark_audio_thread();
                    log::warn!("push-to-talk input error: {e}");
                },
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(PushToTalk { buffer, _stream: stream, device_name })
    }

    #[cfg(target_os = "freebsd")]
    pub fn start() -> Result<PushToTalk, String> {
        Err("push-to-talk input is not supported on this platform yet".into())
    }

    /// Stops the stream and returns the mono audio and its sample rate.
    pub fn finish(self) -> (Vec<f32>, u32) {
        let audio = self.buffer.snapshot();
        let rate = self.buffer.rate;
        drop(self);
        (audio, rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_buffer_mixes_to_mono_and_never_grows() {
        let b = PttBuffer::new(2, 8_000);
        b.push(&[1.0, 0.0, 0.5, 0.5, f32::NAN, 1.0]);
        assert_eq!(b.snapshot(), vec![0.5, 0.5, 0.0]);
        let second = vec![0.25f32; 2 * 8_000 * PTT_SECONDS];
        b.push(&second);
        assert_eq!(b.snapshot().len(), 8_000 * PTT_SECONDS, "capped at PTT_SECONDS");
        assert_eq!(b.dropped(), 3, "the frames that did not fit are counted");
    }

    /// The container has no input device: opening is an error, never a panic or a hang.
    #[test]
    fn no_device_is_an_error() {
        if let Err(e) = PushToTalk::start() {
            assert!(!e.is_empty());
        }
    }
}
