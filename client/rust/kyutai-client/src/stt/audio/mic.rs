use crate::stt::audio::{AudioChunk, ResampleQuality};
use crate::stt::error::{Result, SttError};
use kyutai_client_core::audio::{
    DynResampler, downmix_f32_to_mono_into, downmix_i16_to_mono_into, downmix_u16_to_mono_into,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use tokio::sync::mpsc;
use tracing::warn;

const OUTPUT_SAMPLE_RATE_HZ: u32 = 24_000;
const OUTPUT_CHUNK_SAMPLES: usize = 1920;

#[derive(Clone, Copy, Debug)]
pub struct MicCaptureConfig {
    pub resample_quality: ResampleQuality,
}

impl Default for MicCaptureConfig {
    fn default() -> Self {
        Self {
            resample_quality: ResampleQuality::Linear,
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
        Self::start_default_with_config(MicCaptureConfig::default())
    }

    pub fn start_default_with_config(config: MicCaptureConfig) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| SttError::Message("no default input device available".to_string()))?;

        let input_config = device
            .default_input_config()
            .map_err(|e| SttError::Message(e.to_string()))?;

        let input_sample_rate_hz = input_config.sample_rate().0;
        let input_channels = input_config.channels();
        let stream_config: StreamConfig = input_config.clone().into();
        let resample_quality = config.resample_quality;

        let (tx, rx) = mpsc::channel::<AudioChunk>(8);

        let stream = match input_config.sample_format() {
            SampleFormat::F32 => build_stream_f32(
                &device,
                &stream_config,
                input_channels,
                input_sample_rate_hz,
                tx.clone(),
                resample_quality,
            )?,
            SampleFormat::I16 => build_stream_i16(
                &device,
                &stream_config,
                input_channels,
                input_sample_rate_hz,
                tx.clone(),
                resample_quality,
            )?,
            SampleFormat::U16 => build_stream_u16(
                &device,
                &stream_config,
                input_channels,
                input_sample_rate_hz,
                tx,
                resample_quality,
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
    resample_quality: ResampleQuality,
) -> Result<cpal::Stream> {
    let channels_usize = usize::from(channels);
    let mut resampler =
        DynResampler::new(input_sample_rate_hz, OUTPUT_SAMPLE_RATE_HZ, resample_quality)
            .map_err(|e| SttError::Message(e.to_string()))?;
    let mut mono_buf = Vec::<f32>::with_capacity(OUTPUT_CHUNK_SAMPLES * channels_usize);
    let mut resample_buf = Vec::<f32>::with_capacity(OUTPUT_CHUNK_SAMPLES);
    let mut pending = Vec::<f32>::with_capacity(OUTPUT_CHUNK_SAMPLES * 4);
    let mut pending_read_idx = 0usize;

    device
        .build_input_stream(
            config,
            move |data: &[f32], _info| {
                downmix_f32_to_mono_into(data, channels_usize, &mut mono_buf);
                let samples = match resampler.as_mut() {
                    Some(r) => {
                        if let Err(err) = r.process_into(&mono_buf, &mut resample_buf) {
                            warn!(error = %err, "mic resampling failed");
                            return;
                        }
                        resample_buf.as_slice()
                    }
                    None => mono_buf.as_slice(),
                };

                if samples.is_empty() {
                    return;
                }

                pending.reserve(samples.len());
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

                if pending_read_idx >= OUTPUT_CHUNK_SAMPLES * 4 {
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
    resample_quality: ResampleQuality,
) -> Result<cpal::Stream> {
    let channels_usize = usize::from(channels);
    let mut resampler =
        DynResampler::new(input_sample_rate_hz, OUTPUT_SAMPLE_RATE_HZ, resample_quality)
            .map_err(|e| SttError::Message(e.to_string()))?;
    let mut mono_buf = Vec::<f32>::with_capacity(OUTPUT_CHUNK_SAMPLES * channels_usize);
    let mut resample_buf = Vec::<f32>::with_capacity(OUTPUT_CHUNK_SAMPLES);
    let mut pending = Vec::<f32>::with_capacity(OUTPUT_CHUNK_SAMPLES * 4);
    let mut pending_read_idx = 0usize;

    device
        .build_input_stream(
            config,
            move |data: &[i16], _info| {
                downmix_i16_to_mono_into(data, channels_usize, &mut mono_buf);
                let samples = match resampler.as_mut() {
                    Some(r) => {
                        if let Err(err) = r.process_into(&mono_buf, &mut resample_buf) {
                            warn!(error = %err, "mic resampling failed");
                            return;
                        }
                        resample_buf.as_slice()
                    }
                    None => mono_buf.as_slice(),
                };

                if samples.is_empty() {
                    return;
                }

                pending.reserve(samples.len());
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

                if pending_read_idx >= OUTPUT_CHUNK_SAMPLES * 4 {
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
    resample_quality: ResampleQuality,
) -> Result<cpal::Stream> {
    let channels_usize = usize::from(channels);
    let mut resampler =
        DynResampler::new(input_sample_rate_hz, OUTPUT_SAMPLE_RATE_HZ, resample_quality)
            .map_err(|e| SttError::Message(e.to_string()))?;
    let mut mono_buf = Vec::<f32>::with_capacity(OUTPUT_CHUNK_SAMPLES * channels_usize);
    let mut resample_buf = Vec::<f32>::with_capacity(OUTPUT_CHUNK_SAMPLES);
    let mut pending = Vec::<f32>::with_capacity(OUTPUT_CHUNK_SAMPLES * 4);
    let mut pending_read_idx = 0usize;

    device
        .build_input_stream(
            config,
            move |data: &[u16], _info| {
                downmix_u16_to_mono_into(data, channels_usize, &mut mono_buf);
                let samples = match resampler.as_mut() {
                    Some(r) => {
                        if let Err(err) = r.process_into(&mono_buf, &mut resample_buf) {
                            warn!(error = %err, "mic resampling failed");
                            return;
                        }
                        resample_buf.as_slice()
                    }
                    None => mono_buf.as_slice(),
                };

                if samples.is_empty() {
                    return;
                }

                pending.reserve(samples.len());
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

                if pending_read_idx >= OUTPUT_CHUNK_SAMPLES * 4 {
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
