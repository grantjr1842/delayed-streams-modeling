// Copyright (c) Kyutai, all rights reserved.
// This source code is licensed under the license found in the
// LICENSE file in the root directory of this source tree.

use anyhow::{Context, Result};
use axum::extract::ws;
use candle::{DType, Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use moshi::tts_streaming::Speaker;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WordWithTimestamps {
    pub text: String,
    pub start_s: f64,
    pub stop_s: f64,
}

pub struct Model {
    lm: moshi::lm::LmModel,
    audio_tokenizer: moshi::mimi::Mimi,
    text_tokenizer: std::sync::Arc<sentencepiece::SentencePieceProcessor>,
    speaker_encoder: moshi::tts_streaming::SpeakerEncoder,
    ca_srcs: std::collections::HashMap<String, Tensor>,
    dynamic_ca_srcs: std::sync::Mutex<DynamicVoiceCache>,
    tts_config: moshi::tts_streaming::Config,
    instance_name: String,
    voice_dir: std::path::PathBuf,
    log_dir: std::path::PathBuf,
    log_tokens: bool,
    // Dummy way to ensure that only a single inference can happen.
    pub(crate) mutex: tokio::sync::Mutex<()>,
}

struct DynamicVoiceCache {
    order: std::collections::VecDeque<String>,
    map: std::collections::HashMap<String, Tensor>,
    max_entries: usize,
}

impl DynamicVoiceCache {
    fn new(max_entries: usize) -> Self {
        Self {
            order: std::collections::VecDeque::new(),
            map: std::collections::HashMap::new(),
            max_entries,
        }
    }

    fn get(&mut self, key: &str) -> Option<Tensor> {
        self.map.get(key).cloned()
    }

    fn insert(&mut self, key: String, value: Tensor) {
        if self.map.contains_key(&key) {
            self.map.insert(key, value);
            return;
        }
        self.order.push_back(key.clone());
        self.map.insert(key, value);
        while self.map.len() > self.max_entries {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            } else {
                break;
            }
        }
    }
}

pub enum Encoder {
    OggOpus(kaudio::ogg_opus::Encoder),
    OggOpusMessagePack(kaudio::ogg_opus::Encoder),
    Pcm,
    PcmMessagePack,
}

enum LogMessage {
    Text(String),
    Slice(u32, Vec<u32>),
}

#[derive(serde::Serialize)]
struct QueryWithTexts<'a, Q: serde::Serialize> {
    #[serde(flatten)]
    query: &'a Q,
    texts: Vec<String>,
}

#[derive(Clone)]
struct LogSender(std::sync::mpsc::Sender<LogMessage>);
struct Logger(std::sync::mpsc::Receiver<LogMessage>);

fn logger() -> (LogSender, Logger) {
    let (log_tx, log_rx) = std::sync::mpsc::channel();
    (LogSender(log_tx), Logger(log_rx))
}

impl LogSender {
    fn send(&self, msg: LogMessage) {
        let _err = self.0.send(msg);
    }

    fn send_text(&self, text: String) {
        self.send(LogMessage::Text(text));
    }

    fn send_slice(&self, idx: u32, slice: Vec<u32>) {
        self.send(LogMessage::Slice(idx, slice));
    }
}

impl Logger {
    fn save<P: AsRef<std::path::Path>, T: serde::Serialize>(
        self,
        query: &T,
        log_dir: P,
        instance_name: &str,
    ) -> Result<()> {
        // Use log_rx.iter() to wait on the process loop being done.

        let mut text_tokens = vec![];
        let mut audio_tokens_vec = vec![];
        let mut texts = vec![];
        for elem in self.0.into_iter() {
            match elem {
                LogMessage::Text(text) => {
                    texts.push(text);
                }
                LogMessage::Slice(idx, slice) => {
                    audio_tokens_vec.push(slice);
                    text_tokens.push(idx);
                }
            }
        }
        if audio_tokens_vec.is_empty() {
            return Ok(());
        }
        let text_tokens = text_tokens.into_iter().map(|v| (v, Speaker::Main)).collect::<Vec<_>>();
        let num_steps = audio_tokens_vec.len();
        let num_codebooks = audio_tokens_vec[0].len();
        let audio_tokens_flat: Vec<u32> = audio_tokens_vec.into_iter().flatten().collect();
        let audio_tokens =
            Tensor::from_vec(audio_tokens_flat, (1, num_codebooks, num_steps), &Device::Cpu)?;

        let since_epoch = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
        let (secs, us) = (since_epoch.as_secs(), since_epoch.subsec_micros());
        let base_path = log_dir.as_ref().join(format!("{instance_name}-tts-{secs}-{us}"));
        let json_filename = base_path.with_extension("json");
        let query = QueryWithTexts { query, texts };
        let json_content = serde_json::to_string_pretty(&query)?;
        std::fs::write(json_filename, json_content)?;
        let st_filename = base_path.with_extension("safetensors");
        let text_tokens: Vec<_> = text_tokens.iter().map(|v| v.0 as i64).collect();
        let text_len = text_tokens.len();
        let text_tokens = candle::Tensor::from_vec(text_tokens, text_len, &candle::Device::Cpu)?
            .to_dtype(DType::I64)?;
        let audio_tokens = audio_tokens.to_dtype(DType::I64)?;
        let st_content =
            std::collections::HashMap::from([("text", text_tokens), ("audio", audio_tokens)]);
        candle::safetensors::save(&st_content, st_filename)?;
        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum OutMsg {
    Text { text: String, start_s: f64, stop_s: f64 },
    Audio { pcm: Vec<f32> },
    OggOpus { data: Vec<u8> },
    Error { message: String },
    Ready,
}

#[derive(serde::Serialize)]
#[serde(tag = "type")]
enum OutMsgRef<'a> {
    Audio { pcm: &'a [f32] },
}

impl Encoder {
    pub fn new(format: crate::StreamingOutput) -> Result<Self> {
        match format {
            crate::StreamingOutput::OggOpus => Self::ogg_opus(24000),
            crate::StreamingOutput::OggOpusMessagePack => Self::ogg_opus_message_pack(24000),
            crate::StreamingOutput::Pcm => Ok(Self::pcm()),
            crate::StreamingOutput::PcmMessagePack => Ok(Self::pcm_message_pack()),
        }
    }

    fn ogg_opus(sample_rate: usize) -> Result<Self> {
        Ok(Self::OggOpus(kaudio::ogg_opus::Encoder::new(sample_rate)?))
    }

    fn ogg_opus_message_pack(sample_rate: usize) -> Result<Self> {
        Ok(Self::OggOpusMessagePack(kaudio::ogg_opus::Encoder::new(sample_rate)?))
    }

    fn pcm_message_pack() -> Self {
        Self::PcmMessagePack
    }

    fn pcm() -> Self {
        Self::Pcm
    }

    pub fn header(&self) -> Result<Option<Vec<u8>>> {
        let header = match self {
            Self::OggOpus(oo) => Some(oo.header_data().to_vec()),
            Self::OggOpusMessagePack(oo) => {
                use serde::Serialize;
                let msg = OutMsg::OggOpus { data: oo.header_data().to_vec() };
                let mut buf = vec![];
                msg.serialize(
                    &mut rmp_serde::Serializer::new(&mut buf)
                        .with_human_readable()
                        .with_struct_map(),
                )?;
                Some(buf)
            }
            Self::Pcm => None,
            Self::PcmMessagePack => None,
        };
        Ok(header)
    }

    pub fn encode_word(&self, wwts: WordWithTimestamps) -> Result<Option<Vec<u8>>> {
        if wwts.text.is_empty() {
            return Ok(None);
        }
        let buf = match self {
            Self::Pcm | Self::OggOpus(_) => None,
            Self::OggOpusMessagePack(_) | Self::PcmMessagePack => {
                use serde::Serialize;
                let mut buf = vec![];
                OutMsg::Text { text: wwts.text, start_s: wwts.start_s, stop_s: wwts.stop_s }
                    .serialize(
                        &mut rmp_serde::Serializer::new(&mut buf)
                            .with_human_readable()
                            .with_struct_map(),
                    )?;
                Some(buf)
            }
        };
        Ok(buf)
    }

    pub fn encode(&mut self, pcm: &[f32]) -> Result<Vec<u8>> {
        use serde::Serialize;
        let buf = match self {
            Self::OggOpus(oo) => oo.encode_page(pcm)?,
            Self::OggOpusMessagePack(oo) => {
                let data = oo.encode_page(pcm)?;
                let mut buf = vec![];
                OutMsg::OggOpus { data }.serialize(
                    &mut rmp_serde::Serializer::new(&mut buf)
                        .with_human_readable()
                        .with_struct_map(),
                )?;
                buf
            }
            Self::PcmMessagePack => {
                let mut buf = vec![];
                OutMsgRef::Audio { pcm }.serialize(
                    &mut rmp_serde::Serializer::new(&mut buf)
                        .with_human_readable()
                        .with_struct_map(),
                )?;
                buf
            }
            Self::Pcm => {
                use byteorder::ByteOrder;
                let mut buf = vec![0u8; std::mem::size_of_val(pcm)];
                byteorder::LittleEndian::write_f32_into(pcm, &mut buf);
                buf
            }
        };
        Ok(buf)
    }

    #[allow(dead_code)]
    pub fn encode_msg(&mut self, msg: OutMsg) -> Result<Option<Vec<u8>>> {
        use serde::Serialize;
        let buf = match self {
            Self::OggOpus(_) | Self::Pcm => None,
            Self::OggOpusMessagePack(_) | Self::PcmMessagePack => {
                let mut buf = vec![];
                msg.serialize(
                    &mut rmp_serde::Serializer::new(&mut buf)
                        .with_human_readable()
                        .with_struct_map(),
                )?;
                Some(buf)
            }
        };
        Ok(buf)
    }
}

impl Model {
    pub fn new(tts: &crate::TtsConfig, config: &crate::Config, dev: &Device) -> Result<Self> {
        let dtype = crate::utils::model_dtype(tts.dtype_override.as_deref(), dev)?;
        let model_config = &tts.model;
        let audio_codebooks = model_config.audio_codebooks;
        let audio_tokenizer =
            moshi::mimi::load(&tts.audio_tokenizer_file, Some(audio_codebooks), dev)?;
        let speaker_tokenizer = if tts.speaker_tokenizer_file == tts.audio_tokenizer_file {
            audio_tokenizer.clone()
        } else if tts.speaker_tokenizer_file.is_empty() {
            let vb_lm = unsafe {
                VarBuilder::from_mmaped_safetensors(&[&tts.lm_model_file], DType::F32, dev)?
            };
            let cfg = moshi::mimi::Config::v0_1(None);
            moshi::mimi::Mimi::new(
                cfg,
                vb_lm.pp("condition_provider.conditioners.speaker_wavs.compression_model"),
            )?
        } else {
            moshi::mimi::load(&tts.speaker_tokenizer_file, None, dev)?
        };
        let vb_lm =
            unsafe { VarBuilder::from_mmaped_safetensors(&[&tts.lm_model_file], dtype, dev)? };
        let speaker_encoder = moshi::tts_streaming::SpeakerEncoder::new(
            speaker_tokenizer,
            tts.generation.speaker_cond_dim,
            tts.generation.speaker_cond_n_speakers,
            dtype,
            vb_lm.to_dtype(DType::F32),
        )?;
        let text_tokenizer = sentencepiece::SentencePieceProcessor::open(&tts.text_tokenizer_file)
            .with_context(|| tts.text_tokenizer_file.clone())?;
        let mut ca_srcs = std::collections::HashMap::new();
        for (name, path) in tts.voices.iter() {
            let ca_src = match candle::safetensors::load(path, dev)?.get("ca_src") {
                Some(ca_src) => ca_src.clone(),
                None => anyhow::bail!("missing ca_src tensor in {path}"),
            };
            let ca_src = ca_src.narrow(0, 0, 1)?.to_dtype(dtype)?;
            ca_srcs.insert(name.to_string(), ca_src);
        }
        let lm = moshi::lm::LmModel::new(
            model_config,
            moshi::nn::MaybeQuantizedVarBuilder::Real(vb_lm),
        )?;
        let voice_dir = std::fs::canonicalize(&tts.voice_dir)
            .unwrap_or_else(|_| std::path::PathBuf::from(&tts.voice_dir));
        Ok(Self {
            lm,
            audio_tokenizer,
            text_tokenizer: std::sync::Arc::new(text_tokenizer),
            speaker_encoder,
            ca_srcs,
            dynamic_ca_srcs: std::sync::Mutex::new(DynamicVoiceCache::new(16)),
            tts_config: tts.generation.clone(),
            instance_name: config.instance_name.to_string(),
            log_dir: config.log_dir.clone().into(),
            voice_dir,
            log_tokens: tts.log_tokens,
            mutex: tokio::sync::Mutex::new(()),
        })
    }

    pub async fn handle_socket(
        &self,
        socket: ws::WebSocket,
        query: crate::TtsStreamingQuery,
    ) -> Result<()> {
        use futures_util::{SinkExt, StreamExt};

        let _guard = self.mutex.lock().await;
        let config = &self.tts_config;
        let (log_tx, log_rx) = if self.log_tokens {
            let (tx, rx) = logger();
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };
        let (log_done_tx, log_done_rx) = tokio::sync::oneshot::channel::<()>();

        let log_tx2 = log_tx.clone();
        let log_dir_logger = self.log_dir.clone();
        let instance_name_logger = self.instance_name.clone();
        let query_logger = query.clone();

        let logger_handle = if let Some(rx) = log_rx {
            Some(crate::utils::spawn_blocking("save_tts_logs", move || {
                if let Err(err) = rx.save(&query_logger, &log_dir_logger, &instance_name_logger) {
                    tracing::error!(?err, "cannot save logs")
                };
                let _ = log_done_tx.send(());
                Ok(())
            }))
        } else {
            let _ = log_done_tx.send(());
            None
        };
        let sampling = if query.temperature <= 0. || query.top_k <= 1 {
            candle_transformers::generation::Sampling::ArgMax
        } else {
            candle_transformers::generation::Sampling::TopK {
                k: query.top_k,
                temperature: query.temperature,
            }
        };

        let text_lp = candle_transformers::generation::LogitsProcessor::from_sampling(
            query.seed,
            sampling.clone(),
        );
        let audio_lp =
            candle_transformers::generation::LogitsProcessor::from_sampling(query.seed, sampling);
        let conditions = match self.lm.condition_provider() {
            None => None,
            Some(cp) => {
                let conditions = cp.condition_lut("control", "also_good")?;
                tracing::info!(?conditions, "generated conditions");
                Some(conditions)
            }
        };

        let mut last_text_token = config.text_start_token;
        let ca_src = self.voice_ca_src(query.voice.as_ref(), query.voices.as_ref())?;
        let ca_src = if query.cfg_alpha.is_some() {
            let lp = self.speaker_encoder.empty()?;
            Tensor::cat(&[ca_src, lp], 0)?
        } else {
            ca_src
        };
        let max_seq_len = query.max_seq_len.unwrap_or(2048);
        let mut state = moshi::tts_streaming::State::new(
            self.lm.clone(),
            Some(moshi::transformer::CaSrc::Tokens(ca_src)),
            max_seq_len,
            audio_lp,
            text_lp,
            query.cfg_alpha,
            config.clone(),
        );
        let text_tokenizer = self.text_tokenizer.clone();

        let (mut sender, mut receiver) = socket.split();
        let (in_tx, in_rx) = std::sync::mpsc::channel();
        let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel();
        let text_bos_token = state.config().text_bos_token;
        let text_tokenizer_recv = self.text_tokenizer.clone();
        let recv_loop = tokio::task::spawn(async move {
            let mut inserted_bos = false;
            while let Some(msg) = receiver.next().await {
                let msg = match msg? {
                    ws::Message::Text(x) => {
                        if crate::metrics::stream::enabled() {
                            crate::metrics::stream::TTS_WS_IN_MESSAGES.inc();
                            crate::metrics::stream::TTS_WS_IN_BYTES.inc_by(x.len() as u64);
                        }
                        x
                    }
                    ws::Message::Binary(x) => {
                        if crate::metrics::stream::enabled() {
                            crate::metrics::stream::TTS_WS_IN_MESSAGES.inc();
                            crate::metrics::stream::TTS_WS_IN_BYTES.inc_by(x.len() as u64);
                        }
                        // End of stream, we do not exit the loop so as not to close
                        // the connection.
                        if x.as_ref() == b"\0" {
                            log::info!("received end of stream");
                            in_tx.send(None)?;
                        }
                        continue;
                    }
                    // ping messages are automatically answered by tokio-tungstenite as long as
                    // the connection is read from.
                    ws::Message::Ping(_) | ws::Message::Pong(_) => continue,
                    ws::Message::Close(_) => break,
                };

                for word in msg.split(' ') {
                    if word.is_empty() {
                        continue;
                    }
                    let mut word_tokens: Vec<_> =
                        text_tokenizer_recv.encode(word)?.into_iter().map(|v| v.id).collect();
                    if !inserted_bos {
                        inserted_bos = true;
                        word_tokens.insert(0, text_bos_token)
                    }
                    if let Some(tx) = log_tx2.as_ref() {
                        tx.send_text(word.to_string());
                    }
                    in_tx.send(Some(word_tokens))?;
                }
            }
            tracing::info!("recv loop exited - connection closed");
            Ok::<(), anyhow::Error>(())
        });
        let mut audio_tokenizer = self.audio_tokenizer.clone();
        audio_tokenizer.reset_state();
        let device = state.device().clone();
        let state_cfg = state.config().clone();
        let audio_codebooks = state.audio_codebooks();
        let conditions = conditions.clone();
        let format = query.format;
        enum AudioMessage {
            Tokens(Option<Vec<u32>>, u32, usize),
            Word(WordWithTimestamps),
        }
        let (audio_token_tx, audio_token_rx) = std::sync::mpsc::sync_channel::<AudioMessage>(100);
        let log_tx_audio = log_tx.clone();
        let _audio_processing_loop = tokio::task::spawn_blocking(move || {
            let err = (|| {
                let mut encoder = Encoder::new(format)?;
                if let Some(header) = encoder.header()? {
                    out_tx.send(header)?
                }
                let text_audio_delay_in_tokens = state_cfg.text_audio_delay_in_tokens;
                let acoustic_delay = state_cfg.acoustic_delay;

                for msg in audio_token_rx {
                    match msg {
                        AudioMessage::Word(wwts) => {
                            if let Some(oo) = encoder.encode_word(wwts)? {
                                out_tx.send(oo)?;
                            }
                        }
                        AudioMessage::Tokens(audio_tokens_vec, last_text_token, step_idx) => {
                            if let Some(audio_tokens_vec) = audio_tokens_vec {
                                let cb = audio_tokens_vec.len();
                                // Using Tensor::from_vec is faster.
                                let audio_tokens = candle::Tensor::from_vec(
                                    audio_tokens_vec.clone(),
                                    (1, cb, 1),
                                    &device,
                                )?;
                                if step_idx >= text_audio_delay_in_tokens + acoustic_delay {
                                    let pcm = audio_tokenizer
                                        .decode_step(&audio_tokens.into(), &().into())?;
                                    if let Some(pcm) = pcm.as_option() {
                                        let pcm = pcm.flatten_all()?.to_vec1::<f32>()?;
                                        let oo = encoder.encode(&pcm)?;
                                        out_tx.send(oo)?;
                                    }
                                    if let Some(tx) = log_tx_audio.as_ref() {
                                        tx.send_slice(last_text_token, audio_tokens_vec)
                                    }
                                } else if let Some(tx) = log_tx_audio.as_ref() {
                                    tx.send_slice(last_text_token, audio_tokens_vec)
                                }
                            } else if let Some(tx) = log_tx_audio.as_ref() {
                                let cb = audio_codebooks;
                                let audio_tokens_vec = vec![0u32; cb];
                                tx.send_slice(last_text_token, audio_tokens_vec)
                            }
                        }
                    }
                }
                Ok::<(), anyhow::Error>(())
            })();
            if let Err(err) = err {
                tracing::error!(?err, "audio processing loop exited with error");
            }
        });

        let process_loop = tokio::task::spawn_blocking(move || {
            let err = (|| {
                tracing::info!("starting the inference loop");
                let text_audio_delay_in_tokens = state.config().text_audio_delay_in_tokens;
                let text_eop_token = state.config().text_eop_token;
                let text_pad_token = state.config().text_pad_token;
                let extra_steps = state.config().extra_steps;

                let mut token_idx = 0;
                let mut step_past_last_token = 0;
                // Start with an empty list to trigger the first bos.
                let mut word_tokens = Some(vec![]);

                let mut last_epad_index = 0usize;
                for step_idx in 0..max_seq_len {
                    let allowed_tokens = match word_tokens.as_ref() {
                        None => {
                            step_past_last_token += 1;
                            if step_past_last_token > extra_steps + text_audio_delay_in_tokens {
                                break;
                            }
                            moshi::tts_streaming::AllowedTokens::Pad
                        }
                        Some(word_tokens) => match word_tokens.get(token_idx) {
                            None => moshi::tts_streaming::AllowedTokens::PadOrEpad,
                            Some(id) => moshi::tts_streaming::AllowedTokens::Text(*id),
                        },
                    };
                    last_text_token =
                        state.step(last_text_token, allowed_tokens, conditions.as_ref())?;
                    if last_text_token == text_eop_token {
                        if let Some(vs) = word_tokens {
                            if let Ok(text) = text_tokenizer.decode_piece_ids(&vs) {
                                let start_s = last_epad_index as f64 / 12.5;
                                let stop_s = step_idx as f64 / 12.5;
                                let wwts = WordWithTimestamps { text, start_s, stop_s };
                                audio_token_tx.send(AudioMessage::Word(wwts))?;
                            }
                        }
                        last_epad_index = step_idx;
                        word_tokens = in_rx.recv()?;
                        if word_tokens.is_none() {
                            // We teacher force a pad instead of tho eop for the last word.
                            state.overwrite_last_text_token(text_pad_token)?;
                        }
                        token_idx = 0;
                    } else if last_text_token != text_pad_token {
                        token_idx += 1;
                    }
                    let last_audio_tokens = state.last_audio_tokens();
                    audio_token_tx.send(AudioMessage::Tokens(
                        last_audio_tokens,
                        last_text_token,
                        step_idx,
                    ))?;
                }
                Ok::<(), anyhow::Error>(())
            })();
            match err {
                Err(err) => tracing::error!(?err, "process loop exited"),
                Ok(()) => tracing::info!("process loop exited"),
            }
        });
        let send_loop = tokio::task::spawn(async move {
            use tokio::time::{timeout, Duration};
            loop {
                // The recv method is cancel-safe so can be wrapped in a timeout.
                let msg = timeout(Duration::from_secs(10), out_rx.recv()).await;
                let msg = match msg {
                    Ok(Some(msg)) => {
                        if crate::metrics::stream::enabled() {
                            crate::metrics::stream::TTS_WS_OUT_MESSAGES.inc();
                            crate::metrics::stream::TTS_WS_OUT_BYTES.inc_by(msg.len() as u64);
                        }
                        ws::Message::binary(msg)
                    }
                    Ok(None) => break,
                    Err(_) => ws::Message::Ping(vec![].into()),
                };
                sender.send(msg).await?;
            }
            tracing::info!("send loop exited - connection closed");
            sender.close().await?;
            tracing::info!("send loop exited - connection really closed");
            drop(sender);
            Ok::<(), anyhow::Error>(())
        });
        // select should ensure that all the threads get aborted on timeout.
        // TODO(laurent): this actually doesn't work as expected, and the background threads don't
        // appear to be cancelled properly (at least the websocket connection remains open.
        let sleep = tokio::time::sleep(std::time::Duration::from_secs(360));
        tokio::pin!(sleep);

        let mut recv_handle = recv_loop;
        let mut process_handle = process_loop;
        let mut send_handle = send_loop;

        tokio::select! {
            _ = &mut sleep => {
                tracing::error!("reached timeout");
            }
            _ = &mut recv_handle => {}
            _ = &mut process_handle => {}
            _ = &mut send_handle => {}
        }
        tracing::info!("exiting handle-socket");

        // Wait briefly for aborted tasks to clean up and ensure logs are saved
        let _ = tokio::time::timeout(std::time::Duration::from_millis(500), async {
            recv_handle.abort();
            process_handle.abort();
            send_handle.abort();
            let _ = recv_handle.await;
            let _ = process_handle.await;
            let _ = send_handle.await;
            drop(log_tx); // Close log channel
            if let Some(handle) = logger_handle {
                let _ = handle.await;
            }
            let _ = log_done_rx.await;
        })
        .await;

        Ok(())
    }

    pub fn voice_ca_src(
        &self,
        voice: Option<&String>,
        voices: Option<&Vec<String>>,
    ) -> Result<Tensor> {
        match (voice, voices) {
            (None, None) => anyhow::bail!("either voice or voices has to be set"),
            (Some(_), Some(_)) => {
                anyhow::bail!("voice and voices should not be set at the same time")
            }
            (Some(voice), None) => match self.ca_srcs.get(voice) {
                None => {
                    let voice_dir = &self.voice_dir;
                    let mut pcms = vec![];
                    let (voice, speaker_cond_start_s) = match voice.split_once('+') {
                        None => (voice.as_str(), 0.0),
                        Some((v, delay)) => {
                            let delay = match delay.parse::<f64>() {
                                Ok(delay) => delay,
                                Err(_) => anyhow::bail!(
                                    "unexpected format for delay in {voice}: '{delay}'"
                                ),
                            };
                            (v, delay)
                        }
                    };
                    let path = std::fs::canonicalize(voice_dir.join(voice))?;
                    if !path.starts_with(&voice_dir) {
                        tracing::error!(?voice_dir, ?path, "unable to access voice file");
                        anyhow::bail!("unknown voice file '{voice}'")
                    }
                    let cache_key = format!("{}|{speaker_cond_start_s}", path.to_string_lossy());
                    if let Ok(mut cache) = self.dynamic_ca_srcs.lock() {
                        if let Some(v) = cache.get(&cache_key) {
                            return Ok(v);
                        }
                    }
                    let pcm = speaker_pcm(
                        self.speaker_encoder.sample_rate(),
                        speaker_cond_start_s,
                        self.tts_config.speaker_cond_duration_s,
                        path,
                        self.lm.device(),
                    )?;
                    pcms.push(pcm.clone());
                    pcms.push(pcm);
                    let ca_src = self.speaker_encoder.encode(&pcms)?;
                    if let Ok(mut cache) = self.dynamic_ca_srcs.lock() {
                        cache.insert(cache_key, ca_src.clone());
                    }
                    Ok(ca_src)
                }
                Some(v) => Ok(v.clone()),
            },
            (None, Some(voices)) => {
                let voice_dir = &self.voice_dir;
                let mut pcms = vec![];
                for voice in voices.iter() {
                    let (voice, speaker_cond_start_s) = match voice.split_once('+') {
                        None => (voice.as_str(), 0.0),
                        Some((v, delay)) => {
                            let delay = match delay.parse::<f64>() {
                                Ok(delay) => delay,
                                Err(_) => anyhow::bail!(
                                    "unexpected format for delay in {voice}: '{delay}'"
                                ),
                            };
                            (v, delay)
                        }
                    };
                    let path = std::fs::canonicalize(voice_dir.join(voice))?;
                    if !path.starts_with(&voice_dir) {
                        tracing::error!(?voice_dir, ?path, "unable to access voice file");
                        anyhow::bail!("unknown voice file '{voice}'")
                    }
                    let pcm = speaker_pcm(
                        self.speaker_encoder.sample_rate(),
                        speaker_cond_start_s,
                        self.tts_config.speaker_cond_duration_s,
                        path,
                        self.lm.device(),
                    )?;
                    pcms.push(pcm)
                }
                Ok(self.speaker_encoder.encode(&pcms)?)
            }
        }
    }

    pub fn run(&self, query: &crate::TtsQuery) -> Result<(Vec<u8>, Vec<WordWithTimestamps>)> {
        let config = &self.tts_config;
        let text_audio_delay_in_tokens = config.text_audio_delay_in_tokens;
        let text_bos_token = config.text_bos_token;
        let text_eos_token = config.text_eos_token;
        let text_eop_token = config.text_eop_token;
        let text_pad_token = config.text_pad_token;
        let mut prompt = moshi::tts_streaming::tokenize_prompt(
            &query.text,
            text_bos_token,
            text_eos_token,
            |s| self.text_tokenizer.encode(s).map(|v| v.into_iter().map(|v| v.id).collect()),
        )?;
        // Insert an empty word to start with and trigger the first bos.
        prompt.insert(0, (vec![], Speaker::Other));
        tracing::debug!(?prompt, "starting tts");
        let mut transcript = vec![];
        let (log_tx, log_rx) = if self.log_tokens {
            let (tx, rx) = logger();
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };
        let all_audio_tokens = {
            let start_time = std::time::Instant::now();
            let sampling = if query.temperature <= 0. || query.top_k <= 1 {
                candle_transformers::generation::Sampling::ArgMax
            } else {
                candle_transformers::generation::Sampling::TopK {
                    k: query.top_k,
                    temperature: query.temperature,
                }
            };

            let text_lp = candle_transformers::generation::LogitsProcessor::from_sampling(
                query.seed,
                sampling.clone(),
            );
            let audio_lp = candle_transformers::generation::LogitsProcessor::from_sampling(
                query.seed, sampling,
            );
            let conditions = match self.lm.condition_provider() {
                None => None,
                Some(cp) => {
                    let conditions = cp.condition_lut("control", "also_good")?;
                    tracing::info!(?conditions, "generated conditions");
                    Some(conditions)
                }
            };

            let mut last_text_token = config.text_start_token;
            let ca_src = self.voice_ca_src(query.voice.as_ref(), query.voices.as_ref())?;
            let ca_src = if query.cfg_alpha.is_some() {
                let lp = self.speaker_encoder.empty()?;
                Tensor::cat(&[ca_src, lp], 0)?
            } else {
                ca_src
            };
            let max_seq_len = query.max_seq_len.unwrap_or(2048);
            let config = config.clone();
            let mut state = moshi::tts_streaming::State::new(
                self.lm.clone(),
                Some(moshi::transformer::CaSrc::Tokens(ca_src)),
                max_seq_len,
                audio_lp,
                text_lp,
                query.cfg_alpha,
                config.clone(),
            );
            let mut all_audio_tokens = vec![];
            tracing::info!("starting the inference loop");
            let mut word_idx = 0;
            let mut token_idx = 0;
            let mut step_past_last_token = 0;
            let mut last_epad_index = 0usize;
            for step_idx in 0..max_seq_len {
                let word_tokens = prompt.get(word_idx);
                let allowed_tokens = match word_tokens.as_ref() {
                    None => {
                        step_past_last_token += 1;
                        if step_past_last_token > 5 + text_audio_delay_in_tokens {
                            break;
                        }
                        moshi::tts_streaming::AllowedTokens::Pad
                    }
                    Some(word_tokens) => match word_tokens.0.get(token_idx) {
                        None => moshi::tts_streaming::AllowedTokens::PadOrEpad,
                        Some(id) => moshi::tts_streaming::AllowedTokens::Text(*id),
                    },
                };
                last_text_token =
                    state.step(last_text_token, allowed_tokens, conditions.as_ref())?;
                if last_text_token == text_eop_token {
                    if let Some(vs) = word_tokens {
                        if let Ok(text) = self.text_tokenizer.decode_piece_ids(&vs.0) {
                            let start_s = last_epad_index as f64 / 12.5;
                            let stop_s = step_idx as f64 / 12.5;
                            transcript.push(WordWithTimestamps { text, start_s, stop_s })
                        }
                    }
                    last_epad_index = step_idx;
                    word_idx += 1;
                    token_idx = 0;
                } else if last_text_token != text_pad_token {
                    token_idx += 1;
                }
                if let Some(audio_tokens_vec) = state.last_audio_tokens() {
                    let cb = audio_tokens_vec.len();
                    let audio_tokens = candle::Tensor::from_vec(
                        audio_tokens_vec.clone(),
                        (1, cb, 1),
                        state.device(),
                    )?;
                    if let Some(tx) = log_tx.as_ref() {
                        if step_idx >= text_audio_delay_in_tokens {
                            all_audio_tokens.push(audio_tokens)
                        }
                        tx.send_slice(last_text_token, audio_tokens_vec)
                    } else if step_idx >= text_audio_delay_in_tokens {
                        all_audio_tokens.push(audio_tokens)
                    }
                } else if let Some(tx) = log_tx.as_ref() {
                    let cb = state.audio_codebooks();
                    let audio_tokens_vec = vec![0u32; cb];
                    tx.send_slice(last_text_token, audio_tokens_vec)
                }
            }
            let dt = start_time.elapsed().as_secs_f64();
            let total = all_audio_tokens.len();
            tracing::info!(
                "processed {total} total steps in {dt:.2}s, {:.2} steps/s",
                total as f64 / dt
            );
            Tensor::cat(&all_audio_tokens, candle::D::Minus1)?
        };
        let (_one, _codebooks, total_steps) = all_audio_tokens.dims3()?;
        let mut all_pcm_chunks = vec![];
        let chunk_by = 25;
        let mut mimi = self.audio_tokenizer.clone();
        for start_step in (0..total_steps).step_by(chunk_by) {
            let chunk_steps = usize::min(chunk_by, total_steps - start_step);
            let pcm = mimi.decode_step(
                &all_audio_tokens.narrow(2, start_step, chunk_steps)?.into(),
                &().into(),
            )?;
            if let Some(pcm) = pcm.as_option() {
                all_pcm_chunks.push(pcm.clone())
            }
        }
        // Close the log stream so that log_rx.save does not block.
        std::mem::drop(log_tx);
        if let Some(log_rx) = log_rx {
            if let Err(err) = log_rx.save(&query, &self.log_dir, &self.instance_name) {
                tracing::error!(?err, "cannot save logs")
            };
        }

        let pcm = Tensor::cat(&all_pcm_chunks, 2)?;
        let pcm = pcm.i((0, 0))?.to_vec1::<f32>()?;
        let mut wav = vec![];
        moshi::wav::write_pcm_as_wav(&mut wav, &pcm, 24_000)?;
        Ok((wav, transcript))
    }
}

pub fn speaker_pcm<P: AsRef<std::path::Path>>(
    mimi_sample_rate: f64,
    speaker_cond_start_s: f64,
    speaker_cond_duration_s: f64,
    speaker: P,
    dev: &Device,
) -> Result<Tensor> {
    let (pcm, sample_rate) = kaudio::pcm_decode(speaker)?;
    let pcm = if sample_rate != mimi_sample_rate as u32 {
        kaudio::resample(&pcm, sample_rate as usize, mimi_sample_rate as usize)?
    } else {
        pcm
    };
    let start_pos = (speaker_cond_start_s * mimi_sample_rate) as usize;
    let sample_len = (speaker_cond_duration_s * mimi_sample_rate) as usize;
    let pcm = &pcm[start_pos..start_pos + sample_len];
    let pcm = Tensor::new(pcm, dev)?.reshape((1, 1, ()))?;
    Ok(pcm)
}
