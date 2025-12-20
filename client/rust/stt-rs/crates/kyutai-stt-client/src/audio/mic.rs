use crate::audio::AudioChunk;
use crate::error::{Result, SttError};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use tokio::sync::mpsc;
use tracing::warn;

const OUTPUT_SAMPLE_RATE_HZ: u32 = 24_000;
const OUTPUT_CHUNK_SAMPLES: usize = 1920;

struct LinearResampler {
    in_rate_hz: u32,
    out_rate_hz: u32,
    step: f64,
    pos: f64,
    buf: Vec<f32>,
}

impl LinearResampler {
    fn new(in_rate_hz: u32, out_rate_hz: u32) -> Self {
        Self {
            in_rate_hz,
            out_rate_hz,
            step: in_rate_hz as f64 / out_rate_hz as f64,
            pos: 0.0,
            buf: Vec::new(),
        }
    }

    fn process_into(&mut self, input: &[f32], out: &mut Vec<f32>) {
        out.clear();
        if input.is_empty() {
            return;
        }

        self.buf.extend_from_slice(input);

        let approx_out_len = ((input.len() as u64 * self.out_rate_hz as u64)
            / self.in_rate_hz.max(1) as u64)
            .saturating_add(2) as usize;

        out.reserve(approx_out_len);

        while self.pos + 1.0 < self.buf.len() as f64 {
            let i = self.pos.floor() as usize;
            let frac = self.pos - i as f64;

            let a = self.buf[i];
            let b = self.buf[i + 1];

            out.push(a + (b - a) * frac as f32);
            self.pos += self.step;
        }

        let drain = self.pos.floor() as usize;
        if drain > 0 {
            self.buf.drain(0..drain);
            self.pos -= drain as f64;
        }

    }
}

pub struct MicCapture {
    sample_rate_hz: u32,
    channels: u16,
    rx: mpsc::Receiver<AudioChunk>,
    _stream: cpal::Stream,
}

impl MicCapture {
    pub fn start_default() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| SttError::Message("no default input device available".to_string()))?;

        let config = device
            .default_input_config()
            .map_err(|e| SttError::Message(e.to_string()))?;

        let input_sample_rate_hz = config.sample_rate().0;
        let input_channels = config.channels();
        let stream_config: StreamConfig = config.clone().into();

        let (tx, rx) = mpsc::channel::<AudioChunk>(8);

        let stream = match config.sample_format() {
            SampleFormat::F32 => build_stream_f32(
                &device,
                &stream_config,
                input_channels,
                input_sample_rate_hz,
                tx.clone(),
            )?,
            SampleFormat::I16 => build_stream_i16(
                &device,
                &stream_config,
                input_channels,
                input_sample_rate_hz,
                tx.clone(),
            )?,
            SampleFormat::U16 => build_stream_u16(
                &device,
                &stream_config,
                input_channels,
                input_sample_rate_hz,
                tx,
            )?,
            other => {
                return Err(SttError::Message(format!(
                    "unsupported input sample format: {other:?}"
                )));
            }
        };

        stream
            .play()
            .map_err(|e| SttError::Message(e.to_string()))?;

        Ok(Self {
            sample_rate_hz: OUTPUT_SAMPLE_RATE_HZ,
            channels: 1,
            rx,
            _stream: stream,
        })
    }

    pub fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub async fn recv(&mut self) -> Option<AudioChunk> {
        self.rx.recv().await
    }
}

fn build_stream_f32(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: u16,
    input_sample_rate_hz: u32,
    tx: mpsc::Sender<AudioChunk>,
) -> Result<cpal::Stream> {
    let channels_usize = usize::from(channels);
    let mut resampler = (input_sample_rate_hz != OUTPUT_SAMPLE_RATE_HZ)
        .then(|| LinearResampler::new(input_sample_rate_hz, OUTPUT_SAMPLE_RATE_HZ));
    let mut mono_buf = Vec::<f32>::new();
    let mut resample_buf = Vec::<f32>::new();
    let mut pending = Vec::<f32>::new();
    let mut pending_read_idx = 0usize;

    device
        .build_input_stream(
            config,
            move |data: &[f32], _info| {
                downmix_f32_to_mono_into(data, channels_usize, &mut mono_buf);
                let samples = match resampler.as_mut() {
                    Some(r) => {
                        r.process_into(&mono_buf, &mut resample_buf);
                        resample_buf.as_slice()
                    }
                    None => mono_buf.as_slice(),
                };

                if samples.is_empty() {
                    return;
                }

                pending.extend_from_slice(samples);

                while pending.len().saturating_sub(pending_read_idx) >= OUTPUT_CHUNK_SAMPLES {
                    let start = pending_read_idx;
                    let end = pending_read_idx + OUTPUT_CHUNK_SAMPLES;
                    let chunk = pending[start..end].to_vec();
                    pending_read_idx = end;
                    if tx
                        .try_send(AudioChunk {
                            samples: chunk,
                            sample_rate_hz: OUTPUT_SAMPLE_RATE_HZ,
                        })
                        .is_err()
                    {
                        pending.clear();
                        pending_read_idx = 0;
                        break;
                    }
                }

                if pending_read_idx > 0 && pending_read_idx >= OUTPUT_CHUNK_SAMPLES * 4 {
                    pending.drain(..pending_read_idx);
                    pending_read_idx = 0;
                }
            },
            move |err| {
                warn!(error = %err, "mic input stream error");
            },
            None,
        )
        .map_err(|e| SttError::Message(e.to_string()))
}

fn build_stream_i16(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: u16,
    input_sample_rate_hz: u32,
    tx: mpsc::Sender<AudioChunk>,
) -> Result<cpal::Stream> {
    let channels_usize = usize::from(channels);
    let mut resampler = (input_sample_rate_hz != OUTPUT_SAMPLE_RATE_HZ)
        .then(|| LinearResampler::new(input_sample_rate_hz, OUTPUT_SAMPLE_RATE_HZ));
    let mut mono_buf = Vec::<f32>::new();
    let mut resample_buf = Vec::<f32>::new();
    let mut pending = Vec::<f32>::new();
    let mut pending_read_idx = 0usize;

    device
        .build_input_stream(
            config,
            move |data: &[i16], _info| {
                downmix_i16_to_mono_into(data, channels_usize, &mut mono_buf);
                let samples = match resampler.as_mut() {
                    Some(r) => {
                        r.process_into(&mono_buf, &mut resample_buf);
                        resample_buf.as_slice()
                    }
                    None => mono_buf.as_slice(),
                };

                if samples.is_empty() {
                    return;
                }

                pending.extend_from_slice(samples);

                while pending.len().saturating_sub(pending_read_idx) >= OUTPUT_CHUNK_SAMPLES {
                    let start = pending_read_idx;
                    let end = pending_read_idx + OUTPUT_CHUNK_SAMPLES;
                    let chunk = pending[start..end].to_vec();
                    pending_read_idx = end;
                    if tx
                        .try_send(AudioChunk {
                            samples: chunk,
                            sample_rate_hz: OUTPUT_SAMPLE_RATE_HZ,
                        })
                        .is_err()
                    {
                        pending.clear();
                        pending_read_idx = 0;
                        break;
                    }
                }

                if pending_read_idx > 0 && pending_read_idx >= OUTPUT_CHUNK_SAMPLES * 4 {
                    pending.drain(..pending_read_idx);
                    pending_read_idx = 0;
                }
            },
            move |err| {
                warn!(error = %err, "mic input stream error");
            },
            None,
        )
        .map_err(|e| SttError::Message(e.to_string()))
}

fn build_stream_u16(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: u16,
    input_sample_rate_hz: u32,
    tx: mpsc::Sender<AudioChunk>,
) -> Result<cpal::Stream> {
    let channels_usize = usize::from(channels);
    let mut resampler = (input_sample_rate_hz != OUTPUT_SAMPLE_RATE_HZ)
        .then(|| LinearResampler::new(input_sample_rate_hz, OUTPUT_SAMPLE_RATE_HZ));
    let mut mono_buf = Vec::<f32>::new();
    let mut resample_buf = Vec::<f32>::new();
    let mut pending = Vec::<f32>::new();
    let mut pending_read_idx = 0usize;

    device
        .build_input_stream(
            config,
            move |data: &[u16], _info| {
                downmix_u16_to_mono_into(data, channels_usize, &mut mono_buf);
                let samples = match resampler.as_mut() {
                    Some(r) => {
                        r.process_into(&mono_buf, &mut resample_buf);
                        resample_buf.as_slice()
                    }
                    None => mono_buf.as_slice(),
                };

                if samples.is_empty() {
                    return;
                }

                pending.extend_from_slice(samples);

                while pending.len().saturating_sub(pending_read_idx) >= OUTPUT_CHUNK_SAMPLES {
                    let start = pending_read_idx;
                    let end = pending_read_idx + OUTPUT_CHUNK_SAMPLES;
                    let chunk = pending[start..end].to_vec();
                    pending_read_idx = end;
                    if tx
                        .try_send(AudioChunk {
                            samples: chunk,
                            sample_rate_hz: OUTPUT_SAMPLE_RATE_HZ,
                        })
                        .is_err()
                    {
                        pending.clear();
                        pending_read_idx = 0;
                        break;
                    }
                }

                if pending_read_idx > 0 && pending_read_idx >= OUTPUT_CHUNK_SAMPLES * 4 {
                    pending.drain(..pending_read_idx);
                    pending_read_idx = 0;
                }
            },
            move |err| {
                warn!(error = %err, "mic input stream error");
            },
            None,
        )
        .map_err(|e| SttError::Message(e.to_string()))
}

fn downmix_f32_to_mono_into(data: &[f32], channels: usize, out: &mut Vec<f32>) {
    out.clear();
    if channels <= 1 {
        out.extend_from_slice(data);
        return;
    }

    let frames = data.len() / channels;
    out.reserve(frames);

    for frame_idx in 0..frames {
        let mut sum = 0.0;
        let base = frame_idx * channels;
        for ch in 0..channels {
            sum += data[base + ch];
        }
        out.push(sum / channels as f32);
    }
}

fn downmix_i16_to_mono_into(data: &[i16], channels: usize, out: &mut Vec<f32>) {
    out.clear();
    if channels <= 1 {
        out.reserve(data.len());
        for &sample in data {
            out.push(sample as f32 / 32768.0);
        }
        return;
    }

    let frames = data.len() / channels;
    out.reserve(frames);

    for frame_idx in 0..frames {
        let mut sum = 0.0;
        let base = frame_idx * channels;
        for ch in 0..channels {
            sum += data[base + ch] as f32 / 32768.0;
        }
        out.push(sum / channels as f32);
    }
}

fn downmix_u16_to_mono_into(data: &[u16], channels: usize, out: &mut Vec<f32>) {
    out.clear();
    if channels <= 1 {
        out.reserve(data.len());
        for &sample in data {
            out.push((sample as f32 - 32768.0) / 32768.0);
        }
        return;
    }

    let frames = data.len() / channels;
    out.reserve(frames);

    for frame_idx in 0..frames {
        let mut sum = 0.0;
        let base = frame_idx * channels;
        for ch in 0..channels {
            sum += (data[base + ch] as f32 - 32768.0) / 32768.0;
        }
        out.push(sum / channels as f32);
    }
}
