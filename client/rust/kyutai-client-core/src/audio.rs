use anyhow::{Result, Context};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[cfg(feature = "audio")]
use std::time::Duration as StdDuration;
#[cfg(feature = "audio")]
use tokio::io::AsyncWriteExt;
#[cfg(feature = "audio")]
use tokio::process::Command;
#[cfg(feature = "audio")]
use tokio::sync::mpsc;
#[cfg(feature = "audio")]
use std::process::Stdio;
#[cfg(feature = "audio")]
use ringbuf::{HeapRb, traits::*};
#[cfg(feature = "audio")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub const DEFAULT_SAMPLE_RATE: u32 = 24000;

#[derive(Clone, Debug)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub sample_rate_hz: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResampleQuality {
    #[default]
    Linear,
    High,
}

/// Audio level measurements in decibels.
#[derive(Clone, Copy, Debug, Default)]
pub struct AudioLevel {
    /// RMS (root mean square) level in dBFS.
    pub rms_db: f32,
    /// Peak level in dBFS.
    pub peak_db: f32,
}

/// Minimum dB floor to avoid -infinity for silence.
pub const DB_FLOOR: f32 = -60.0;

impl AudioLevel {
    /// Compute audio levels from f32 samples (expected range: -1.0 to 1.0).
    pub fn compute(samples: &[f32]) -> Self {
        if samples.is_empty() {
            return Self {
                rms_db: DB_FLOOR,
                peak_db: DB_FLOOR,
            };
        }

        let mut sum_sq = 0.0f32;
        let mut peak = 0.0f32;
        for &sample in samples {
            let abs = sample.abs();
            sum_sq += sample * sample;
            if abs > peak {
                peak = abs;
            }
        }
        let rms = (sum_sq / samples.len() as f32).sqrt();

        let rms_db = linear_to_db(rms);
        let peak_db = linear_to_db(peak);

        Self { rms_db, peak_db }
    }

    pub fn is_silent(&self) -> bool {
        self.rms_db <= DB_FLOOR + 1.0
    }
}

pub fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        DB_FLOOR
    } else {
        (20.0 * linear.log10()).max(DB_FLOOR)
    }
}

#[derive(Clone, Debug)]
pub struct LevelMeter {
    smoothing: f32,
    smoothed_rms_db: f32,
    smoothed_peak_db: f32,
}

impl Default for LevelMeter {
    fn default() -> Self {
        Self::new(0.7)
    }
}

impl LevelMeter {
    pub fn new(smoothing: f32) -> Self {
        Self {
            smoothing: smoothing.clamp(0.0, 0.99),
            smoothed_rms_db: DB_FLOOR,
            smoothed_peak_db: DB_FLOOR,
        }
    }

    pub fn process(&mut self, samples: &[f32]) -> AudioLevel {
        let level = AudioLevel::compute(samples);

        self.smoothed_rms_db =
            self.smoothing * self.smoothed_rms_db + (1.0 - self.smoothing) * level.rms_db;

        if level.peak_db > self.smoothed_peak_db {
            self.smoothed_peak_db = level.peak_db;
        } else {
            self.smoothed_peak_db =
                self.smoothing * self.smoothed_peak_db + (1.0 - self.smoothing) * level.peak_db;
        }

        AudioLevel {
            rms_db: self.smoothed_rms_db,
            peak_db: self.smoothed_peak_db,
        }
    }

    pub fn reset(&mut self) {
        self.smoothed_rms_db = DB_FLOOR;
        self.smoothed_peak_db = DB_FLOOR;
    }
}

pub struct LinearResampler {
    in_rate_hz: u32,
    out_rate_hz: u32,
    step: f64,
    pos: f64,
    buf: Vec<f32>,
}

impl LinearResampler {
    pub fn new(in_rate_hz: u32, out_rate_hz: u32) -> Self {
        Self {
            in_rate_hz,
            out_rate_hz,
            step: in_rate_hz as f64 / out_rate_hz as f64,
            pos: 0.0,
            buf: Vec::new(),
        }
    }

    pub fn process_into(&mut self, input: &[f32], out: &mut Vec<f32>) {
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

#[cfg(feature = "audio")]
pub struct HqResampler {
    resampler: rubato::FftFixedInOut<f32>,
    input_buffer: Vec<Vec<f32>>,
    output_buffer: Vec<Vec<f32>>,
    pending: Vec<f32>,
}

#[cfg(feature = "audio")]
impl HqResampler {
    pub fn new(in_rate: u32, out_rate: u32, chunk_size: usize) -> Result<Self> {
        use rubato::Resampler as _;
        let resampler = rubato::FftFixedInOut::<f32>::new(
            in_rate as usize,
            out_rate as usize,
            chunk_size,
            1,
        )?;
        let input_buffer = resampler.input_buffer_allocate(true);
        let output_buffer = resampler.output_buffer_allocate(true);
        Ok(Self {
            resampler,
            input_buffer,
            output_buffer,
            pending: Vec::new(),
        })
    }

    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) -> Result<()> {
        use rubato::Resampler as _;

        if !input.is_empty() {
            self.pending.extend_from_slice(input);
        }

        while self.pending.len() >= self.resampler.input_frames_next() {
            let chunk_size = self.resampler.input_frames_next();
            let in_chunk = &self.pending[..chunk_size];
            self.input_buffer[0].copy_from_slice(in_chunk);
            let (_in_len, out_len) = self.resampler.process_into_buffer(
                &self.input_buffer,
                &mut self.output_buffer,
                None,
            )?;
            output.extend_from_slice(&self.output_buffer[0][..out_len]);
            self.pending.drain(..chunk_size);
        }
        Ok(())
    }

    pub fn flush(&mut self, output: &mut Vec<f32>) -> Result<()> {
        use rubato::Resampler as _;
        if !self.pending.is_empty() {
            // FftFixedInOut requires full chunks or process_partial
            let (_in_len, out_len) = self.resampler.process_partial_into_buffer(
                Some(&[&self.pending]),
                &mut self.output_buffer,
                None,
            )?;
            output.extend_from_slice(&self.output_buffer[0][..out_len]);
            self.pending.clear();
        }
        Ok(())
    }
}

pub enum DynResampler {
    Linear(LinearResampler),
    #[cfg(feature = "audio")]
    High(Box<HqResampler>),
}

impl DynResampler {
    pub fn new(in_rate_hz: u32, out_rate_hz: u32, quality: ResampleQuality) -> Result<Option<Self>> {
        if in_rate_hz == out_rate_hz {
            return Ok(None);
        }

        match quality {
            ResampleQuality::Linear => Ok(Some(Self::Linear(LinearResampler::new(
                in_rate_hz,
                out_rate_hz,
            )))),
            ResampleQuality::High => {
                #[cfg(feature = "audio")]
                {
                    Ok(Some(Self::High(Box::new(HqResampler::new(
                        in_rate_hz,
                        out_rate_hz,
                        1024,
                    )?))))
                }
                #[cfg(not(feature = "audio"))]
                {
                    anyhow::bail!("audio feature (for hq-resample) is not enabled")
                }
            }
        }
    }

    pub fn process_into(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<()> {
        match self {
            DynResampler::Linear(resampler) => {
                resampler.process_into(input, out);
                Ok(())
            }
            #[cfg(feature = "audio")]
            DynResampler::High(resampler) => resampler.process(input, out),
        }
    }

    pub fn flush(&mut self, out: &mut Vec<f32>) -> Result<()> {
        match self {
            DynResampler::Linear(resampler) => {
                resampler.process_into(&[], out);
                Ok(())
            }
            #[cfg(feature = "audio")]
            DynResampler::High(resampler) => resampler.flush(out),
        }
    }
}

pub fn downmix_f32_to_mono_into(data: &[f32], channels: usize, out: &mut Vec<f32>) {
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

pub fn downmix_i16_to_mono_into(data: &[i16], channels: usize, out: &mut Vec<f32>) {
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

pub fn downmix_u16_to_mono_into(data: &[u16], channels: usize, out: &mut Vec<f32>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_silence() {
        let samples = vec![0.0; 1920];
        let level = AudioLevel::compute(&samples);
        assert!(level.rms_db <= DB_FLOOR + 0.1);
        assert!(level.peak_db <= DB_FLOOR + 0.1);
        assert!(level.is_silent());
    }

    #[test]
    fn test_level_full_scale() {
        let samples: Vec<f32> = (0..1920)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let level = AudioLevel::compute(&samples);
        assert!((level.rms_db - 0.0).abs() < 0.1);
        assert!((level.peak_db - 0.0).abs() < 0.1);
    }
}

#[cfg(feature = "audio")]
pub struct AudioPlayer {
    pub _stream: cpal::Stream,
    pub producer: ringbuf::HeapProd<f32>,
    pub queued_samples: Arc<AtomicUsize>,
    pub started: Arc<AtomicBool>,
    pub output_sample_rate: usize,
}

#[cfg(feature = "audio")]
impl AudioPlayer {
    pub fn setup(
        prebuffer_ms: u32,
        max_buffer_ms: u32,
        sample_rate_hz: Option<u32>,
        buffer_frames: Option<u32>,
        verbose: bool,
    ) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("no output device available")?;

        let mut supported_configs_range = device.supported_output_configs()?;
        let config_range = match supported_configs_range.find(|c| c.channels() == 1) {
            None => device
                .supported_output_configs()?
                .next()
                .context("no audio output available")?,
            Some(config_range) => config_range,
        };

        let default_sr = device
            .default_output_config()
            .ok()
            .map(|cfg| cfg.sample_rate().0);
        let desired_sr = sample_rate_hz.or(default_sr).unwrap_or(DEFAULT_SAMPLE_RATE);

        let sample_rate = cpal::SampleRate(desired_sr).clamp(
            config_range.min_sample_rate(),
            config_range.max_sample_rate(),
        );
        let mut config: cpal::StreamConfig = config_range.with_sample_rate(sample_rate).into();
        if let Some(frames) = buffer_frames
            && frames > 0
        {
            config.buffer_size = cpal::BufferSize::Fixed(frames);
        }
        let channels = config.channels as usize;

        let output_sample_rate = config.sample_rate.0 as usize;

        let min_buffer_samples = ((output_sample_rate as u64 * prebuffer_ms as u64) / 1000) as usize;
        let max_buffer_samples = ((output_sample_rate as u64 * max_buffer_ms as u64) / 1000) as usize;
        let min_buffer_samples = usize::max(min_buffer_samples, output_sample_rate / 20);
        let max_buffer_samples = usize::max(max_buffer_samples, min_buffer_samples.saturating_mul(2));

        let rb = HeapRb::<f32>::new(max_buffer_samples);
        let (producer, mut consumer) = rb.split();
        let queued_samples = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(AtomicBool::new(false));
        let qs = queued_samples.clone();
        let started_cb = started.clone();
        let mut last_elem_state = 0.0f32;

        if verbose {
            let device_name = device.name().unwrap_or_else(|_| "unk".to_string());
            eprintln!(
                "cpal device: {device_name} sample_rate={} channels={} buffer={:?}",
                config.sample_rate.0, config.channels, config.buffer_size
            );
        }

        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                data.fill(0.);

                if !started_cb.load(Ordering::Acquire) {
                    if qs.load(Ordering::Acquire) < min_buffer_samples {
                        return;
                    }
                    started_cb.store(true, Ordering::Release);
                }

                let mut last_elem = last_elem_state;
                let mut popped = 0usize;
                for (idx, elem) in data.iter_mut().enumerate() {
                    if idx % channels == 0 {
                        let v_opt = consumer.try_pop();
                        match v_opt {
                            None => {
                                break;
                            }
                            Some(v) => {
                                last_elem = v;
                                popped = popped.saturating_add(1);
                                *elem = v;
                            }
                        }
                    } else {
                        *elem = last_elem
                    }
                }

                if popped > 0 {
                    let _ = qs.fetch_update(Ordering::AcqRel, Ordering::Acquire, |v0| {
                        Some(v0.saturating_sub(popped))
                    });
                }
                last_elem_state = last_elem;
            },
            move |err| eprintln!("cpal error: {err}"),
            None,
        )?;
        stream.play()?;

        Ok(AudioPlayer {
            _stream: stream,
            producer,
            queued_samples,
            started,
            output_sample_rate,
        })
    }
}

#[cfg(feature = "audio")]
pub struct PulsePlayer {
    pub child: tokio::process::Child,
    pub stdin: tokio::process::ChildStdin,
    pub scratch: Vec<u8>,
}

#[cfg(feature = "audio")]
impl PulsePlayer {
    pub async fn start(
        sample_rate_hz: u32,
        latency_ms: Option<u32>,
        process_time_ms: Option<u32>,
    ) -> Result<Self> {
        let mut args = vec![
            "--raw".to_string(),
            format!("--rate={sample_rate_hz}"),
            "--channels=1".to_string(),
            "--format=float32le".to_string(),
        ];
        if let Some(ms) = latency_ms
            && ms > 0
        {
            args.push(format!("--latency-msec={ms}"));
        }
        if let Some(ms) = process_time_ms
            && ms > 0
        {
            args.push(format!("--process-time-msec={ms}"));
        }

        let mut child = Command::new("pacat")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to spawn pacat")?;

        let stdin = child.stdin.take().context("Failed to open pacat stdin")?;

        Ok(Self {
            child,
            stdin,
            scratch: Vec::new(),
        })
    }

    pub async fn write_f32(&mut self, pcm: &[f32]) -> Result<()> {
        if cfg!(target_endian = "little") {
            let bytes =
                unsafe { std::slice::from_raw_parts(pcm.as_ptr() as *const u8, pcm.len() * 4) };
            self.stdin.write_all(bytes).await?;
        } else {
            self.scratch.clear();
            self.scratch.reserve(pcm.len() * 4);
            for s in pcm {
                self.scratch.extend_from_slice(&s.to_le_bytes());
            }
            self.stdin.write_all(&self.scratch).await?;
        }
        Ok(())
    }

    pub async fn finish(mut self, timeout: StdDuration) {
        let _ = self.stdin.shutdown().await;
        drop(self.stdin);
        let _ = tokio::time::timeout(timeout, self.child.wait()).await;
    }
}

#[cfg(feature = "audio")]
pub async fn run_pulse_writer(
    mut rx: mpsc::Receiver<Vec<f32>>,
    mut prebuf: Vec<f32>,
    sample_rate_hz: u32,
    latency_ms: u32,
    process_time_ms: u32,
    finish_timeout: StdDuration,
    verbose: bool,
) {
    let mut pp =
        match PulsePlayer::start(sample_rate_hz, Some(latency_ms), Some(process_time_ms)).await {
            Ok(v) => v,
            Err(e) => {
                if verbose {
                    eprintln!("Warning: failed to start pacat: {e}");
                }
                return;
            }
        };

    let chunk_ms = if process_time_ms > 0 {
        process_time_ms
    } else {
        40
    };
    let chunk_samples = ((sample_rate_hz as u64 * chunk_ms as u64) / 1000) as usize;

    let mut pending: Vec<f32> = Vec::new();
    let mut pending_pos: usize = 0;

    pending.append(&mut prebuf);

    loop {
        while pending.len().saturating_sub(pending_pos) >= chunk_samples && chunk_samples > 0 {
            let end = pending_pos + chunk_samples;
            if let Err(e) = pp.write_f32(&pending[pending_pos..end]).await {
                if verbose {
                    eprintln!("Warning: failed to write to pacat: {e}");
                }
                return;
            }
            pending_pos = end;
        }

        if pending_pos > 0 && pending_pos >= pending.len() / 2 {
            pending.drain(..pending_pos);
            pending_pos = 0;
        }

        let Some(chunk) = rx.recv().await else {
            break;
        };
        pending.extend_from_slice(&chunk);
    }

    if pending_pos < pending.len() {
        let _ = pp.write_f32(&pending[pending_pos..]).await;
    }
    pp.finish(finish_timeout).await;
}
