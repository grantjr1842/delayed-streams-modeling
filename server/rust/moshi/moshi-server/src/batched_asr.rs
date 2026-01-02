// Copyright (c) Kyutai, all rights reserved.
// This source code is licensed under the license found in the
// LICENSE file in the root directory of this source tree.

use crate::asr::{InMsg, OutMsg};
use crate::metrics::asr as metrics;
use crate::metrics::errors as error_metrics;
use crate::metrics::warmup as warmup_metrics;
use crate::protocol::CloseCode;
use crate::AsrStreamingQuery as Query;
use anyhow::{Context, Result};
use axum::extract::ws;
use candle::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::task;
use tokio::time::{timeout, Duration};

enum PipelineEvent {
    Reset(usize),
    Marker(Marker),
}

const FRAME_SIZE: usize = 1920;
const SEND_PING_EVERY: Duration = Duration::from_secs(10);
const POST_RETRY_DELAY: Duration = Duration::from_millis(100);
const POST_MAX_RETRIES: usize = 1000;

#[derive(Debug, PartialEq, Eq, Clone)]
struct Marker {
    channel_id: ChannelId,
    batch_idx: usize,
    step_idx: usize,
    marker_id: i64,
}

impl std::cmp::PartialOrd for Marker {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for Marker {
    // We use reverse ordering as this will be embedded in a max heap.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.step_idx.cmp(&other.step_idx).reverse()
    }
}

type InSend = std::sync::mpsc::Sender<InMsg>;
type InRecv = std::sync::mpsc::Receiver<InMsg>;
type OutSend = tokio::sync::mpsc::UnboundedSender<OutMsg>;
type OutRecv = tokio::sync::mpsc::UnboundedReceiver<OutMsg>;

/// Unique identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChannelId(usize);

impl ChannelId {
    fn new() -> Self {
        // https://users.rust-lang.org/t/idiomatic-rust-way-to-generate-unique-id/33805
        use std::sync::atomic;
        static COUNTER: atomic::AtomicUsize = atomic::AtomicUsize::new(1);
        Self(COUNTER.fetch_add(1, atomic::Ordering::Relaxed))
    }
}

struct Channel {
    id: ChannelId,
    in_rx: InRecv,
    out_tx: OutSend,
    data: VecDeque<f32>,
    steps: usize,
}

impl Channel {
    fn new(in_rx: InRecv, out_tx: OutSend) -> Result<Self> {
        metrics::OPEN_CHANNELS.inc();
        Ok(Self {
            id: ChannelId::new(),
            in_rx,
            out_tx,
            data: VecDeque::new(),
            steps: 0,
        })
    }

    fn extend_data(&mut self, pcm: &[f32], out_pcm: &mut [f32]) -> bool {
        debug_assert_eq!(out_pcm.len(), FRAME_SIZE);
        if pcm.is_empty() && self.data.len() < FRAME_SIZE {
            return false;
        }
        if self.data.is_empty() && pcm.len() >= FRAME_SIZE {
            out_pcm.copy_from_slice(&pcm[..FRAME_SIZE]);
            self.data.extend(&pcm[FRAME_SIZE..]);
            true
        } else {
            if !pcm.is_empty() {
                self.data.reserve(pcm.len());
            }
            self.data.extend(pcm);
            if self.data.len() >= FRAME_SIZE {
                let cont = self.data.make_contiguous();
                out_pcm.copy_from_slice(&cont[..FRAME_SIZE]);
                self.data.drain(..FRAME_SIZE);
                true
            } else {
                false
            }
        }
    }

    fn send(&self, msg: OutMsg, ref_channel_id: Option<ChannelId>) -> Result<()> {
        // If the channel id has changed compared to the reference. Return Ok(())
        // so as not to disconnect the new user.
        if Some(self.id) != ref_channel_id {
            return Ok(());
        }
        self.out_tx.send(msg)?;
        Ok(())
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        metrics::OPEN_CHANNELS.dec();
        metrics::CONNECTION_NUM_STEPS.observe(self.steps as f64);
    }
}

struct Logger {
    base_path: std::path::PathBuf,
    log_tx: std::sync::mpsc::Sender<(Tensor, Vec<Tensor>)>,
    log_rx: std::sync::mpsc::Receiver<(Tensor, Vec<Tensor>)>,
    log_frequency_s: f64,
}

impl Logger {
    fn new<P: AsRef<std::path::Path>>(
        instance_name: &str,
        log_dir: P,
        log_frequency_s: f64,
    ) -> Result<Self> {
        let since_epoch = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
        let (secs, us) = (since_epoch.as_secs(), since_epoch.subsec_micros());
        let base_path = log_dir.as_ref().join(format!("{instance_name}-asr-{secs}-{us}"));
        let (log_tx, log_rx) = std::sync::mpsc::channel::<(Tensor, Vec<Tensor>)>();
        Ok(Self { base_path, log_tx, log_rx, log_frequency_s })
    }

    fn log_loop(self) {
        tracing::info!(?self.base_path, "starting log loop");
        task::spawn_blocking(move || {
            let mut cnt = 0usize;
            loop {
                std::thread::sleep(std::time::Duration::from_secs_f64(self.log_frequency_s));
                let tokens: Vec<_> = self.log_rx.try_iter().collect();
                if tokens.is_empty() {
                    tracing::info!("no tokens to log");
                    continue;
                }
                let st_filename = self.base_path.with_extension(format!("{cnt}.safetensors"));
                tracing::info!(?st_filename, "writing logs");
                let write = move || {
                    let (text_tokens_captured_tensors, audio_tokens_captured_tensors): (
                        Vec<_>,
                        Vec<_>,
                    ) = tokens.into_iter().unzip();
                    let text_tokens_captured: Vec<Vec<u32>> = text_tokens_captured_tensors
                        .into_iter()
                        .map(|t| t.to_vec1::<u32>().unwrap_or_default())
                        .collect();
                    let audio_tokens_captured: Vec<Vec<Vec<u32>>> = audio_tokens_captured_tensors
                        .into_iter()
                        .map(|v| {
                            v.into_iter().map(|t| t.to_vec1::<u32>().unwrap_or_default()).collect()
                        })
                        .collect();
                    let num_steps = text_tokens_captured.len();
                    if num_steps == 0 {
                        return Ok(());
                    }
                    let batch_size = text_tokens_captured[0].len();
                    let text_tokens_flat: Vec<u32> =
                        text_tokens_captured.into_iter().flatten().collect();
                    let text_tokens =
                        Tensor::from_vec(text_tokens_flat, (batch_size, num_steps), &Device::Cpu)?;

                    let num_codebooks = audio_tokens_captured[0].len();
                    let audio_tokens_flat: Vec<u32> =
                        audio_tokens_captured.into_iter().flatten().flatten().collect();
                    let audio_tokens = Tensor::from_vec(
                        audio_tokens_flat,
                        (batch_size, num_codebooks, num_steps),
                        &Device::Cpu,
                    )?;

                    let st_content = std::collections::HashMap::from([
                        ("text", text_tokens.to_dtype(DType::I64)?),
                        ("audio", audio_tokens.to_dtype(DType::I64)?),
                    ]);
                    candle::safetensors::save(&st_content, st_filename)?;
                    Ok::<_, anyhow::Error>(())
                };
                if let Err(err) = write() {
                    tracing::error!(?err, "failed to write safetensors");
                }
                cnt += 1;
            }
        });
    }
}

struct BatchedAsrInner {
    channels: Channels,
    active_indices: Arc<Mutex<VecDeque<usize>>>,
    free_indices: Arc<Mutex<VecDeque<usize>>>,
    asr_delay_in_tokens: usize,
    temperature: f64,
    lm: moshi::lm::LmModel,
    audio_tokenizer: moshi::mimi::Mimi,
    text_tokenizer: std::sync::Arc<sentencepiece::SentencePieceProcessor>,
}

fn warmup(
    state: &mut moshi::asr::State,
    conditions: Option<&moshi::conditioner::Condition>,
) -> Result<()> {
    let dev = state.device().clone();
    let pcm = vec![0f32; FRAME_SIZE * state.batch_size()];
    let pcm = Tensor::from_vec(pcm, (state.batch_size(), 1, FRAME_SIZE), &dev)?;
    let mask = moshi::StreamMask::new(vec![true; state.batch_size()], &dev)?;
    for _ in 0..2 {
        let _asr_msgs = state.step_pcm(pcm.clone(), conditions, &mask, |_, _, _| ())?;
    }
    dev.synchronize()?;
    Ok(())
}

impl BatchedAsrInner {
    fn start_model_loop(
        self,
        conditioning_delay: Option<f32>,
        conditioning_learnt_padding: bool,
        batch_size: usize,
        logger: Option<&Logger>,
        warmup_enabled: bool,
    ) -> Result<()> {
        let conditions = match self.lm.condition_provider() {
            None => None,
            Some(cp) => match (conditioning_delay, conditioning_learnt_padding) {
                (Some(delay), false) => {
                    let conditions = cp.condition_cont("delay", -delay)?;
                    tracing::info!(?conditions, "generated conditions");
                    Some(conditions)
                }
                (None, true) => {
                    let conditions = cp.learnt_padding("delay")?;
                    tracing::info!(?conditions, "generated conditions");
                    Some(conditions)
                }
                (Some(_), true) => anyhow::bail!(
                    "conditioning_delay/conditioning_learnt_padding cannot be both set"
                ),
                (None, false) => {
                    anyhow::bail!("conditioning_delay/conditioning_learnt_padding is required")
                }
            },
        };
        let mut state = moshi::asr::State::new(
            batch_size,
            self.asr_delay_in_tokens,
            self.temperature,
            self.audio_tokenizer.clone(),
            self.lm.clone(),
        )?;
        let log_tx = logger.map(|v| v.log_tx.clone());
        let dev = state.device().clone();

        struct PipelineMsg {
            audio_tokens: Tensor,
            mask: moshi::StreamMask,
            channel_ids: Vec<Option<ChannelId>>,
            new_markers: Vec<Marker>,
            resets: Vec<usize>,
            has_data: bool,
        }

        let (pipeline_tx, pipeline_rx) = std::sync::mpsc::sync_channel::<PipelineMsg>(100);
        let asr_inner = Arc::new(self);
        let asr_inner_encoder = asr_inner.clone();
        let asr_inner_post = asr_inner.clone();

        let asr_delay_in_tokens = state.asr_delay_in_tokens;
        let mut mimi_tokenizer = state.audio_tokenizer.clone();

        let dev_encoder = dev.clone();
        #[cfg(feature = "cuda")]
        let mut pinned_batch_pcm = if let Device::Cuda(cuda_dev) = &dev_encoder {
            unsafe { cuda_dev.cuda_stream().context().alloc_pinned::<f32>(FRAME_SIZE * batch_size) }
                .ok()
        } else {
            None
        };

        #[cfg(not(feature = "cuda"))]
        let mut pinned_batch_pcm: Option<Vec<f32>> = None;

        let mut new_markers = Vec::new();
        let mut resets = Vec::new();

        let _encoder_handle = crate::utils::spawn_blocking("encoder_loop", move || {
            let mut step_idx = 0;
            let mut batch_pcm_vec = vec![0f32; FRAME_SIZE * batch_size];
            let mut channel_ids = vec![None; batch_size];
            let mut mask = vec![false; batch_size];
            loop {
                new_markers.clear();
                resets.clear();

                #[cfg(feature = "cuda")]
                let batch_pcm: &mut [f32] = if let Some(p) = pinned_batch_pcm.as_mut() {
                    match p.as_mut_slice() {
                        Ok(slice) => slice,
                        Err(_) => &mut batch_pcm_vec,
                    }
                } else {
                    &mut batch_pcm_vec
                };

                #[cfg(not(feature = "cuda"))]
                let batch_pcm: &mut [f32] = {
                    let _ = &mut pinned_batch_pcm;
                    &mut batch_pcm_vec
                };

                asr_inner_encoder.pre_process_pipelined(
                    asr_delay_in_tokens,
                    step_idx,
                    &mut new_markers,
                    &mut resets,
                    batch_pcm,
                    &mut mask,
                    &mut channel_ids,
                );

                let with_data = mask.iter().any(|&v| v);
                if with_data || !resets.is_empty() || !new_markers.is_empty() {
                    let mask_obj = moshi::StreamMask::new(mask.clone(), &dev_encoder)?;
                    let pcm = {
                        #[cfg(feature = "cuda")]
                        {
                            Tensor::from_slice(batch_pcm, (batch_size, 1, FRAME_SIZE), &dev_encoder)
                        }
                        #[cfg(not(feature = "cuda"))]
                        {
                            Tensor::from_slice(batch_pcm, (batch_size, 1, FRAME_SIZE), &dev_encoder)
                        }
                    }?;
                    let audio_tokens = mimi_tokenizer.encode_step(&pcm.into(), &mask_obj)?;
                    if let Some(audio_tokens) = audio_tokens.into_option() {
                        if pipeline_tx
                            .send(PipelineMsg {
                                audio_tokens,
                                mask: mask_obj,
                                channel_ids: channel_ids.clone(),
                                new_markers: new_markers.clone(),
                                resets: resets.clone(),
                                has_data: true,
                            })
                            .is_err()
                        {
                            break;
                        }
                    } else if !resets.is_empty() || !new_markers.is_empty() {
                        let empty_tokens = Tensor::zeros(
                            (batch_size, mimi_tokenizer.config().quantizer_n_q, 0),
                            DType::U32,
                            &dev_encoder,
                        )?;
                        if pipeline_tx
                            .send(PipelineMsg {
                                audio_tokens: empty_tokens,
                                mask: mask_obj,
                                channel_ids: channel_ids.clone(),
                                new_markers: new_markers.clone(),
                                resets: resets.clone(),
                                has_data: false,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    step_idx += 1;
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }
            Ok(())
        });

        struct PostProcessMsg {
            asr_msgs: Vec<moshi::asr::AsrMsg>,
            step_idx: usize,
            new_markers: Vec<Marker>,
            mask: moshi::StreamMask,
            channel_ids: Vec<Option<ChannelId>>,
        }
        let (post_tx, post_rx) = std::sync::mpsc::sync_channel::<PostProcessMsg>(100);

        let _post_handle = crate::utils::spawn_blocking("post_process_loop", move || {
            let mut markers = BinaryHeap::new();
            for msg in post_rx {
                let PostProcessMsg { asr_msgs, step_idx, new_markers, mask, channel_ids } = msg;
                for m in new_markers {
                    markers.push(m);
                }
                asr_inner_post.post_process(
                    asr_msgs,
                    step_idx,
                    &mut markers,
                    &mask,
                    &channel_ids,
                )?;
            }
            Ok(())
        });

        crate::utils::spawn_blocking("model_loop", move || {
            if warmup_enabled {
                let start = Instant::now();
                tracing::info!("warming-up the asr");
                let res = warmup(&mut state, conditions.as_ref());
                let elapsed = start.elapsed().as_secs_f64();
                warmup_metrics::DURATION.observe(elapsed);
                match &res {
                    Ok(_) => {
                        warmup_metrics::SUCCESS.inc();
                        tracing::info!(duration_ms = (elapsed * 1000.0), "warmup completed");
                    }
                    Err(err) => {
                        warmup_metrics::FAILURE.inc();
                        tracing::error!(duration_ms = (elapsed * 1000.0), ?err, "warmup failed");
                    }
                }
                res?;
            } else {
                tracing::info!("skipping warmup (disabled)");
                warmup_metrics::SKIPPED.inc();
            }
            tracing::info!("starting asr loop {batch_size}");
            let mut step_idx = 0;
            for msg in pipeline_rx {
                let PipelineMsg {
                    audio_tokens,
                    mask,
                    channel_ids,
                    new_markers,
                    resets,
                    has_data,
                } =
                    msg;

                for bid in resets {
                    if let Err(err) = state.reset_batch_idx(bid) {
                        tracing::error!(?err, bid, "failed to reset batch");
                    }
                }

                if has_data {
                    let mask_obj = mask;
                    let start_time = std::time::Instant::now();
                    let asr_msgs = state.step_tokens(
                        &audio_tokens,
                        conditions.as_ref(),
                        &mask_obj,
                        |_, text_tokens, audio_tokens| {
                            if let Some(log_tx) = log_tx.as_ref() {
                                if let Err(err) =
                                    log_tx.send((text_tokens.clone(), audio_tokens.to_vec()))
                                {
                                    tracing::error!(?err, "failed to send log");
                                }
                            }
                        },
                    )?;
                    let elapsed = start_time.elapsed().as_secs_f64();
                    metrics::MODEL_STEP_DURATION.observe(elapsed);
                    tracing::info!(step_idx, "{:.2}ms", elapsed * 1000.);
                    step_idx += 1;

                    if post_tx
                        .send(PostProcessMsg {
                            asr_msgs,
                            step_idx,
                            new_markers,
                            mask: mask_obj,
                            channel_ids,
                        })
                        .is_err()
                    {
                        break;
                    }
                } else if !new_markers.is_empty()
                    && post_tx
                        .send(PostProcessMsg {
                            asr_msgs: vec![],
                            step_idx,
                            new_markers,
                            mask,
                            channel_ids,
                        })
                        .is_err()
                {
                    break;
                }
            }
            Ok(())
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn pre_process_pipelined(
        &self,
        asr_delay_in_tokens: usize,
        step_idx: usize,
        new_markers: &mut Vec<Marker>,
        resets: &mut Vec<usize>,
        batch_pcm: &mut [f32],
        mask: &mut [bool],
        channel_ids: &mut [Option<ChannelId>],
    ) {
        use rayon::prelude::*;

        mask.fill(false);
        channel_ids.fill(None);

        let active_indices: Vec<usize> = {
            let guard = self.active_indices.lock().unwrap();
            guard.iter().copied().collect()
        };

        if active_indices.is_empty() {
            return;
        }


        let active_set: std::collections::HashSet<usize> = active_indices.iter().copied().collect();

        let todo: Vec<(usize, bool, Option<ChannelId>, Vec<PipelineEvent>)> = batch_pcm
            .par_chunks_mut(FRAME_SIZE)
            .enumerate()
            .filter(|(bid, _)| active_set.contains(bid))
            .map(|(bid, out_pcm)| {
                out_pcm.fill(0.0);
                let mut guard = self.channels[bid].lock().unwrap();
                let channel = &mut *guard;
                let c = match channel.as_mut() {
                    Some(c) => c,
                    None => return (bid, false, None, vec![]),
                };

                if c.out_tx.is_closed() {
                    return (bid, false, Some(c.id), vec![PipelineEvent::Reset(usize::MAX)]);
                }

                let mut events = Vec::new();
                let mut mask_val = false;
                use std::sync::mpsc::TryRecvError;
                loop {
                    match c.in_rx.try_recv() {
                        Ok(InMsg::Init) => {
                            if c.out_tx.send(OutMsg::Ready).is_err() {
                                events.push(PipelineEvent::Reset(usize::MAX));
                                break;
                            }
                            events.push(PipelineEvent::Reset(bid));
                        }
                        Ok(InMsg::Marker { id }) => {
                            tracing::info!(bid, id, "received marker");
                            let current_data = c.data.len() / FRAME_SIZE;
                            let marker_step_idx = step_idx + asr_delay_in_tokens + current_data;
                            events.push(PipelineEvent::Marker(Marker {
                                channel_id: c.id,
                                batch_idx: bid,
                                step_idx: marker_step_idx,
                                marker_id: id,
                            }));
                        }
                        Ok(InMsg::OggOpus { .. }) => {
                            tracing::warn!("OggOpus message received in pre-process, should have been decoded in handle_socket");
                        }
                        Ok(InMsg::Audio { pcm }) => {
                            if c.extend_data(&pcm, out_pcm) {
                                c.steps += 1;
                                mask_val = true;
                            }
                        }
                        Ok(InMsg::Ping) => {}
                        Err(TryRecvError::Empty) => {
                            if c.extend_data(&[], out_pcm) {
                                c.steps += 1;
                                mask_val = true;
                            }
                            break;
                        }
                        Err(TryRecvError::Disconnected) => {
                            events.push(PipelineEvent::Reset(usize::MAX));
                            break;
                        }
                    }
                }
                (bid, mask_val, Some(c.id), events)
            })
            .collect();

        for (bid, mask_val, cid, events) in todo {
            channel_ids[bid] = cid;
            mask[bid] = mask_val;
            for event in events {
                match event {
                    PipelineEvent::Reset(usize::MAX) => {}
                    PipelineEvent::Reset(bid) => resets.push(bid),
                    PipelineEvent::Marker(m) => new_markers.push(m),
                }
            }
        }

        // Clean up closed channels
        let mut active_guard = self.active_indices.lock().unwrap();
        let mut free_guard = self.free_indices.lock().unwrap();

        let mut i = 0;
        while i < active_guard.len() {
            let bid = active_guard[i];
            let mut guard = self.channels[bid].lock().unwrap();
            let should_remove = match guard.as_ref() {
                Some(c) => c.out_tx.is_closed(),
                None => true,
            };
            if should_remove {
                *guard = None;
                active_guard.remove(i);
                free_guard.push_back(bid);
            } else {
                i += 1;
            }
        }
    }

    fn post_process(
        &self,
        asr_msgs: Vec<moshi::asr::AsrMsg>,
        step_idx: usize,
        markers: &mut BinaryHeap<Marker>,
        mask: &moshi::StreamMask,
        ref_channel_ids: &[Option<ChannelId>],
    ) -> Result<()> {
        for asr_msg in asr_msgs.into_iter() {
            match asr_msg {
                moshi::asr::AsrMsg::Word { tokens, start_time, batch_idx } => {
                    let msg = OutMsg::Word {
                        text: self.text_tokenizer.decode_piece_ids(&tokens)?,
                        start_time,
                    };
                    let mut channel = self.channels[batch_idx].lock().unwrap();
                    if let Some(c) = channel.as_ref() {
                        if c.send(msg, ref_channel_ids[batch_idx]).is_err() {
                            *channel = None;
                        }
                    }
                }
                moshi::asr::AsrMsg::EndWord { stop_time, batch_idx } => {
                    let msg = OutMsg::EndWord { stop_time };
                    let mut channel = self.channels[batch_idx].lock().unwrap();
                    if let Some(c) = channel.as_ref() {
                        if c.send(msg, ref_channel_ids[batch_idx]).is_err() {
                            *channel = None;
                        }
                    }
                }
                moshi::asr::AsrMsg::Step { step_idx, prs } => {
                    for (batch_idx, channel_mutex) in self.channels.iter().enumerate() {
                        if !mask.is_active(batch_idx) {
                            continue;
                        }
                        let mut channel = channel_mutex.lock().unwrap();
                        if let Some(ch) = channel.as_mut() {
                            let prs = prs.iter().map(|p| p[batch_idx]).collect();
                            let msg = OutMsg::Step { step_idx, prs, buffered_pcm: ch.data.len() };
                            if ch.send(msg, ref_channel_ids[batch_idx]).is_err() {
                                *channel = None;
                            }
                        }
                    }
                }
            }
        }
        while let Some(m) = markers.peek() {
            if m.step_idx <= step_idx {
                let mut channel = self.channels[m.batch_idx].lock().unwrap();
                if let Some(c) = channel.as_ref() {
                    if c.send(OutMsg::Marker { id: m.marker_id }, Some(m.channel_id)).is_err() {
                        *channel = None;
                    }
                }
                markers.pop();
            } else {
                break;
            }
        }
        Ok(())
    }
}

type Channels = Arc<Vec<Mutex<Option<Channel>>>>;

pub struct BatchedAsr {
    channels: Channels,
    active_indices: Arc<Mutex<VecDeque<usize>>>,
    free_indices: Arc<Mutex<VecDeque<usize>>>,
    config: crate::AsrConfig,
    batch_size: usize,
}

impl BatchedAsr {
    pub fn new(
        batch_size: usize,
        asr: &crate::AsrConfig,
        config: &crate::Config,
        dev: &Device,
        warmup_enabled: bool,
    ) -> Result<Self> {
        let dtype = crate::utils::model_dtype(asr.dtype_override.as_deref(), dev)?;
        let vb_lm =
            unsafe { VarBuilder::from_mmaped_safetensors(&[&asr.lm_model_file], dtype, dev)? };
        let lm = moshi::lm::LmModel::batched(
            batch_size,
            &asr.model,
            moshi::nn::MaybeQuantizedVarBuilder::Real(vb_lm),
        )?;
        let audio_tokenizer = {
            let vb = unsafe {
                candle_nn::VarBuilder::from_mmaped_safetensors(
                    &[&asr.audio_tokenizer_file],
                    DType::F32,
                    dev,
                )?
            };
            let mut cfg = moshi::mimi::Config::v0_1(Some(asr.model.audio_codebooks));
            // The mimi transformer runs at 25Hz.
            cfg.transformer.max_seq_len = asr.model.transformer.max_seq_len * 2;
            moshi::mimi::Mimi::batched(batch_size, cfg, vb)?
        };
        let text_tokenizer = sentencepiece::SentencePieceProcessor::open(&asr.text_tokenizer_file)
            .with_context(|| asr.text_tokenizer_file.clone())?;
        let channels = (0..batch_size).map(|_| Mutex::new(None)).collect::<Vec<_>>();
        let channels = Arc::new(channels);
        let free_indices = Arc::new(Mutex::new((0..batch_size).collect::<VecDeque<_>>()));
        let active_indices = Arc::new(Mutex::new(VecDeque::with_capacity(batch_size)));

        let asr_delay_in_tokens =
            asr.conditioning_delay.map_or(asr.asr_delay_in_tokens, |v| (v * 12.5) as usize + 1);
        let batched_asr = BatchedAsrInner {
            asr_delay_in_tokens,
            temperature: asr.temperature.unwrap_or(0.0),
            lm,
            audio_tokenizer,
            text_tokenizer: text_tokenizer.into(),
            channels: channels.clone(),
            active_indices: active_indices.clone(),
            free_indices: free_indices.clone(),
        };
        let logger = match asr.log_frequency_s {
            Some(s) => Some(Logger::new(&config.instance_name, &config.log_dir, s)?),
            None => None,
        };
        batched_asr.start_model_loop(
            asr.conditioning_delay,
            asr.conditioning_learnt_padding,
            batch_size,
            logger.as_ref(),
            warmup_enabled,
        )?;
        if let Some(logger) = logger {
            logger.log_loop()
        }
        Ok(Self { channels, active_indices, free_indices, config: asr.clone(), batch_size })
    }

    fn channels(&self) -> Result<Option<(usize, InSend, OutRecv)>> {
        let mut free_guard = self.free_indices.lock().unwrap();
        if let Some(batch_idx) = free_guard.pop_front() {
            let mut guard = self.channels[batch_idx].lock().unwrap();
            let (in_tx, in_rx) = std::sync::mpsc::channel::<InMsg>();
            let (out_tx, out_rx) = tokio::sync::mpsc::unbounded_channel::<OutMsg>();
            let c = Channel::new(in_rx, out_tx)?;
            *guard = Some(c);
            let mut active_guard = self.active_indices.lock().unwrap();
            active_guard.push_back(batch_idx);
            return Ok(Some((batch_idx, in_tx, out_rx)));
        }
        Ok(None)
    }

    pub async fn handle_query(&self, query: axum::body::Bytes) -> Result<Vec<OutMsg>> {
        tracing::info!("batched-asr post query");
        let (batch_idx, in_tx, mut out_rx) = {
            let mut num_tries = 0;
            loop {
                match self.channels() {
                    Ok(Some(x)) => break x,
                    Ok(None) => {
                        num_tries += 1;
                        if num_tries > POST_MAX_RETRIES {
                            tracing::error!("no free channels after 1000 tries");
                            anyhow::bail!("no free channels");
                        }
                        tokio::time::sleep(POST_RETRY_DELAY).await;
                    }
                    Err(err) => {
                        tracing::error!(?err, "no free channels");
                        Err(err)?
                    }
                }
            }
        };
        tracing::info!(batch_idx, "batched-asr channel");
        in_tx.send(InMsg::Init)?;
        let (pcm, sample_rate) = crate::utils::pcm_decode(query)?;
        let pcm = if sample_rate == 24000 {
            pcm
        } else {
            kaudio::resample(&pcm, sample_rate as usize, 24000)?
        };
        in_tx.send(InMsg::Audio { pcm })?;
        in_tx.send(InMsg::Marker { id: 0 })?;
        in_tx.send(InMsg::Audio { pcm: vec![0f32; 240000] })?;
        let mut msgs = vec![];
        while let Some(msg) = out_rx.recv().await {
            match msg {
                OutMsg::Marker { .. } => break,
                OutMsg::Error { .. } | OutMsg::Word { .. } | OutMsg::EndWord { .. } => {
                    msgs.push(msg)
                }
                OutMsg::Ready | OutMsg::Step { .. } => {}
            }
        }
        Ok(msgs)
    }

    pub async fn handle_socket(&self, socket: ws::WebSocket, query: Query) -> Result<()> {
        use futures_util::{SinkExt, StreamExt};
        use serde::Serialize;

        tracing::info!(?query, "batched-asr ws query");
        metrics::CONNECT.inc();

        let (mut sender, receiver) = socket.split();
        let (batch_idx, in_tx, mut out_rx) = match self.channels()? {
            Some(v) => v,
            None => {
                tracing::error!(
                    error_type = "capacity",
                    module = "batched_asr",
                    "no free channels - server at capacity"
                );
                error_metrics::record_connection_error("capacity", "batched_asr");
                // Send error message in protocol format
                let mut msg = vec![];
                OutMsg::Error { message: "Server at capacity - no free channels available".into() }
                    .serialize(
                        &mut rmp_serde::Serializer::new(&mut msg)
                            .with_human_readable()
                            .with_struct_map(),
                    )?;
                sender.send(ws::Message::binary(msg)).await?;
                // Close with proper close code
                crate::utils::close_with_reason(
                    &mut sender,
                    CloseCode::ServerAtCapacity,
                    Some("No free channels available, please retry later"),
                )
                .await?;
                anyhow::bail!("no free channels")
            }
        };
        tracing::info!(batch_idx, "batched-asr channel");
        in_tx.send(InMsg::Init)?;
        let mut decoder = kaudio::ogg_opus::Decoder::new(24000, FRAME_SIZE)?;

        crate::utils::spawn("recv_loop", async move {
            let mut receiver = receiver;
            // There are two timeouts here:
            // - The short timeout handles the case where the client does not answer the regular pings.
            // - The long timeout handles the case where the client does not send valid data for a
            // long time.
            let mut last_message_received = std::time::Instant::now();
            let short_timeout_duration = SEND_PING_EVERY * 6;
            let long_timeout_duration = std::time::Duration::from_secs(120);
            loop {
                use ws::Message;
                let msg = match timeout(short_timeout_duration, receiver.next()).await {
                    Ok(Some(msg)) => msg,
                    Ok(None) => break,
                    Err(_) => {
                        tracing::info!(?batch_idx, "recv loop short timeout");
                        break;
                    }
                };
                if last_message_received.elapsed() > long_timeout_duration {
                    tracing::info!(?batch_idx, "recv loop long timeout");
                    break;
                }
                let msg = match msg? {
                    Message::Binary(x) => x,
                    // ping messages are automatically answered by tokio-tungstenite as long as
                    // the connection is read from.
                    Message::Ping(_) | Message::Pong(_) | Message::Text(_) => continue,
                    Message::Close(_) => break,
                };
                last_message_received = std::time::Instant::now();
                let msg: InMsg = match rmp_serde::from_slice(&msg) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!(
                            ?batch_idx,
                            error = %e,
                            msg_len = msg.len(),
                            "failed to deserialize InMsg, skipping message"
                        );
                        continue;
                    }
                };

                match msg {
                    InMsg::OggOpus { data } => {
                        match decoder.decode(&data) {
                            Ok(Some(pcm)) => {
                                in_tx.send(InMsg::Audio { pcm: pcm.to_vec() })?;
                            }
                            Ok(None) => {}
                            Err(err) => tracing::error!(?err, "oggopus decoding error"),
                        }
                    }
                    m => in_tx.send(m)?,
                }
            }
            Ok::<_, anyhow::Error>(())
        });
        crate::utils::spawn("send_loop", async move {
            use bytes::BufMut;

            let mut chunk_buf = bytes::BytesMut::with_capacity(8 * 1024);
            let mut chunk_buf_spare = bytes::BytesMut::with_capacity(8 * 1024);
            let mut sender = sender;
            loop {
                // The recv method is cancel-safe so can be wrapped in a timeout.
                let msg = timeout(SEND_PING_EVERY, out_rx.recv()).await;
                let msg = match msg {
                    Ok(None) => break,
                    Err(_) => ws::Message::Ping(vec![].into()),
                    Ok(Some(msg)) => {
                        chunk_buf.clear();
                        {
                            let mut w = (&mut chunk_buf).writer();
                            msg.serialize(
                                &mut rmp_serde::Serializer::new(&mut w)
                                    .with_human_readable()
                                    .with_struct_map(),
                            )?;
                        }
                        std::mem::swap(&mut chunk_buf, &mut chunk_buf_spare);
                        let bytes = chunk_buf_spare.split().freeze();
                        ws::Message::Binary(bytes)
                    }
                };
                sender.send(msg).await?;
            }
            Ok::<(), anyhow::Error>(())
        });
        Ok(())
    }

    pub fn config(&self) -> &crate::AsrConfig {
        &self.config
    }

    pub fn total_slots(&self) -> usize {
        self.batch_size
    }

    pub fn used_slots(&self) -> usize {
        self.channels.iter().filter(|v| v.lock().unwrap().is_some()).count()
    }
}
