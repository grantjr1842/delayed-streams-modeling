use crate::stt::error::{Result, SttError};
use crate::stt::protocol::{InMsg, OutMsg, decode_out_msg, encode_in_msg, encode_in_msg_into};
use crate::stt::transcript::TranscriptAssembler;
use crate::stt::types::{SttEvent, Utterance};

use futures_util::{SinkExt, StreamExt};
use futures_util::stream::SplitStream;
use std::collections::VecDeque;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep, sleep_until, timeout};
use tokio_tungstenite::tungstenite::Message;
use kyutai_client_core::ws::{WsStream, connect_ws, build_ws_url};
type WsRead = SplitStream<WsStream>;

const SHUTDOWN_FLUSH_MARKER_ID: i64 = i64::MIN + 1;
const SHUTDOWN_FLUSH_CHUNK_SAMPLES: usize = 1920;
const SHUTDOWN_FLUSH_CHUNK_DELAY: Duration = Duration::from_millis(80);
const SHUTDOWN_FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
enum SendCmd {
    Msg(InMsg),
    Raw(Vec<u8>),
    Close,
}

enum RecvOutcome {
    Closed { code: u16, reason: String },
    Error(String),
    Eof,
}

fn is_retryable_close_code(code: u16) -> bool {
    matches!(code, 4000 | 4004 | 4005 | 4006 | 1012 | 1013)
}

fn close_code_message(code: u16, reason: &str) -> String {
    let reason = reason.trim();
    let reason_suffix = if reason.is_empty() {
        String::new()
    } else {
        format!(" (reason: {reason})")
    };

    match code {
        4000 => format!("server at capacity (close code 4000){reason_suffix}"),
        4001 => format!("authentication failed (close code 4001){reason_suffix}"),
        4002 => format!("session timeout (close code 4002){reason_suffix}"),
        4003 => format!("invalid message (close code 4003){reason_suffix}"),
        4004 => format!("rate limited (close code 4004){reason_suffix}"),
        4005 => format!("resource unavailable (close code 4005){reason_suffix}"),
        4006 => format!("client timeout (close code 4006){reason_suffix}"),
        other => format!("websocket closed (code {other}){reason_suffix}"),
    }
}

// Replaced by kyutai_client_core::ws::connect_ws

fn spawn_recv_task(
    mut ws_read: WsRead,
    out_tx: mpsc::Sender<OutMsg>,
) -> mpsc::Receiver<RecvOutcome> {
    let (done_tx, done_rx) = mpsc::channel(1);

    tokio::spawn(async move {
        let outcome = loop {
            let Some(item) = ws_read.next().await else {
                break RecvOutcome::Eof;
            };

            let msg = match item {
                Ok(msg) => msg,
                Err(e) => break RecvOutcome::Error(format!("websocket transport error: {e}")),
            };

            match msg {
                Message::Binary(bytes) => {
                    let out = match decode_out_msg(bytes.as_ref()) {
                        Ok(out) => out,
                        Err(e) => break RecvOutcome::Error(format!("protocol decode error: {e}")),
                    };

                    if out_tx.send(out).await.is_err() {
                        break RecvOutcome::Error("recv consumer dropped".to_string());
                    }
                }
                Message::Close(frame) => {
                    let (code, reason) = if let Some(frame) = frame {
                        (frame.code.into(), frame.reason.to_string())
                    } else {
                        (1000u16, String::new())
                    };
                    break RecvOutcome::Closed { code, reason };
                }
                _ => {}
            }
        };

        let _ = done_tx.send(outcome).await;
    });

    done_rx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_session(out_rx: mpsc::Receiver<OutMsg>) -> SttSession {
        let (tx, _rx) = mpsc::channel::<SendCmd>(1);

        let send_loop: JoinHandle<Result<()>> = tokio::spawn(async move { Ok(()) });
        let recv_loop: JoinHandle<Result<()>> = tokio::spawn(async move { Ok(()) });
        let keepalive_loop: JoinHandle<Result<()>> = tokio::spawn(async move { Ok(()) });

        SttSession {
            sender: SttSender { tx },
            send_loop,
            recv_loop,
            keepalive_loop,
            out_rx,
        }
    }

    #[tokio::test]
    async fn utterance_is_finalized_after_inactivity() {
        let (out_tx, out_rx) = mpsc::channel::<OutMsg>(16);
        let session = dummy_session(out_rx);
        let mut stream = session
            .into_event_stream()
            .utterance_finalize_delay(Duration::from_millis(10))
            .utterance_partial_interval(Duration::from_millis(0));

        out_tx
            .send(OutMsg::Word {
                text: "hello".to_string(),
                start_time: 0.0,
            })
            .await
            .unwrap();

        match stream.recv().await.unwrap() {
            SttEvent::WordReceived { text, start_ms } => {
                assert_eq!(text, "hello");
                assert_eq!(start_ms, 0);
            }
            other => panic!("unexpected event: {other:?}"),
        }

        out_tx
            .send(OutMsg::EndWord { stop_time: 0.1 })
            .await
            .unwrap();

        match stream.recv().await.unwrap() {
            SttEvent::WordFinalized(word) => {
                assert_eq!(word.word, "hello");
                assert_eq!(word.start_ms, 0);
                assert_eq!(word.end_ms, 100);
            }
            other => panic!("unexpected event: {other:?}"),
        }

        match stream.recv().await.unwrap() {
            SttEvent::UtterancePartial(u) => {
                assert_eq!(u.text, "hello");
            }
            other => panic!("unexpected event: {other:?}"),
        }

        tokio::time::sleep(Duration::from_millis(15)).await;

        match stream.recv().await.unwrap() {
            SttEvent::UtteranceFinal(u) => {
                assert_eq!(u.text, "hello");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SttClientBuilder {
    url: Option<String>,
    auth_token: Option<String>,
    query_token: Option<String>,
    auto_reconnect: bool,
    max_reconnect_attempts: usize,
    reconnect_delay: Duration,
}

impl SttClientBuilder {
    pub fn new() -> Self {
        Self {
            auto_reconnect: false,
            max_reconnect_attempts: 3,
            reconnect_delay: Duration::from_secs(1),
            ..Self::default()
        }
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    pub fn query_token(mut self, token: impl Into<String>) -> Self {
        self.query_token = Some(token.into());
        self
    }

    pub fn auto_reconnect(mut self, max_attempts: usize) -> Self {
        self.auto_reconnect = true;
        self.max_reconnect_attempts = max_attempts;
        self
    }

    pub fn reconnect_delay(mut self, delay: Duration) -> Self {
        self.reconnect_delay = delay;
        self
    }

    pub async fn connect(self) -> Result<SttSession> {
        let url = self
            .url
            .ok_or_else(|| SttError::Message("missing websocket url".to_string()))?;

        let auth_token = self.auth_token;
        let query_token = self.query_token;
        let auto_reconnect = self.auto_reconnect;
        let max_reconnect_attempts = self.max_reconnect_attempts;
        let reconnect_delay = self.reconnect_delay;

        let ws_url = build_ws_url(&url, "", &[], query_token.as_deref())
            .map_err(|e| SttError::Message(e.to_string()))?;
        let ws_stream = connect_ws(&ws_url, auth_token.as_deref())
            .await
            .map_err(|e| SttError::Message(e.to_string()))?;
        let (ws_write, ws_read) = ws_stream.split();
        let (tx, mut rx) = mpsc::channel::<SendCmd>(128);
        let (out_tx, out_rx) = mpsc::channel::<OutMsg>(128);

        let keepalive_tx = tx.clone();
        let ping_bytes = encode_in_msg(&InMsg::Ping)?;

        let send_loop: JoinHandle<Result<()>> = tokio::spawn(async move {
            let url = url;
            let auth_token = auth_token;
            let query_token = query_token;
            let reconnect_delay = reconnect_delay;

            let mut ws_write = ws_write;
            let mut reconnect_attempts = 0usize;
            let mut recv_done_rx = spawn_recv_task(ws_read, out_tx.clone());

            loop {
                tokio::select! {
                    cmd = rx.recv() => {
                        let Some(cmd) = cmd else {
                            break;
                        };

                        match cmd {
                            SendCmd::Msg(msg) => {
                                let mut buf = Vec::new();
                                encode_in_msg_into(&mut buf, &msg)?;
                                ws_write
                                    .send(Message::Binary(buf.into()))
                                    .await
                                    .map_err(|e| SttError::Message(e.to_string()))?;
                            }
                            SendCmd::Raw(bytes) => {
                                ws_write
                                    .send(Message::Binary(bytes.into()))
                                    .await
                                    .map_err(|e| SttError::Message(e.to_string()))?;
                            }
                            SendCmd::Close => {
                                let _ = ws_write.send(Message::Close(None)).await;
                                break;
                            }
                        }
                    }
                    outcome = recv_done_rx.recv() => {
                        let Some(outcome) = outcome else {
                            break;
                        };

                        match outcome {
                            RecvOutcome::Closed { code, reason } => {
                                if code != 1000 {
                                    let message = close_code_message(code, &reason);
                                    if auto_reconnect
                                        && is_retryable_close_code(code)
                                        && reconnect_attempts < max_reconnect_attempts
                                    {
                                        reconnect_attempts += 1;
                                        let _ = out_tx
                                            .send(OutMsg::Error {
                                                message: format!("{message}; reconnecting..."),
                                            })
                                            .await;

                                        sleep(reconnect_delay).await;

                                        let ws_url = build_ws_url(&url, "", &[], query_token.as_deref())
                                            .map_err(|e| SttError::Message(e.to_string()))?;
                                        let ws_stream = match connect_ws(
                                            &ws_url,
                                            auth_token.as_deref(),
                                        )
                                        .await
                                        {
                                            Ok(ws_stream) => ws_stream,
                                            Err(e) => {
                                                let _ = out_tx
                                                    .send(OutMsg::Error {
                                                        message: format!("reconnect failed: {e}"),
                                                    })
                                                    .await;
                                                break;
                                            }
                                        };

                                        let (new_write, new_read) = ws_stream.split();
                                        ws_write = new_write;
                                        recv_done_rx = spawn_recv_task(new_read, out_tx.clone());
                                        continue;
                                    }

                                    let _ = out_tx
                                        .send(OutMsg::Error { message })
                                        .await;
                                }

                                break;
                            }
                            RecvOutcome::Error(message) => {
                                let _ = out_tx
                                    .send(OutMsg::Error { message })
                                    .await;
                                break;
                            }
                            RecvOutcome::Eof => {
                                break;
                            }
                        }
                    }
                }
            }

            Ok(())
        });

        let keepalive_loop: JoinHandle<Result<()>> = tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(5)).await;

                if keepalive_tx
                    .send(SendCmd::Raw(ping_bytes.clone()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Ok(())
        });

        let recv_loop: JoinHandle<Result<()>> = tokio::spawn(async move { Ok(()) });

        Ok(SttSession {
            sender: SttSender { tx },
            send_loop,
            recv_loop,
            keepalive_loop,
            out_rx,
        })
    }
}

pub struct SttSession {
    sender: SttSender,
    send_loop: JoinHandle<Result<()>>,
    recv_loop: JoinHandle<Result<()>>,
    keepalive_loop: JoinHandle<Result<()>>,
    out_rx: mpsc::Receiver<OutMsg>,
}

impl SttSession {
    pub fn sender(&self) -> SttSender {
        self.sender.clone()
    }

    pub async fn recv(&mut self) -> Result<OutMsg> {
        self.out_rx
            .recv()
            .await
            .ok_or_else(|| SttError::Message("recv loop ended".to_string()))
    }

    pub async fn shutdown(self) -> Result<()> {
        let SttSession {
            sender,
            send_loop,
            recv_loop,
            keepalive_loop,
            mut out_rx,
        } = self;

        if sender
            .send(InMsg::Marker {
                id: SHUTDOWN_FLUSH_MARKER_ID,
            })
            .await
            .is_ok()
        {
            let silence_chunk = vec![0.0f32; SHUTDOWN_FLUSH_CHUNK_SAMPLES];

            let _ = timeout(SHUTDOWN_FLUSH_TIMEOUT, async {
                loop {
                    tokio::select! {
                        msg = out_rx.recv() => {
                            let Some(msg) = msg else {
                                break;
                            };

                            if let OutMsg::Marker { id } = msg
                                && id == SHUTDOWN_FLUSH_MARKER_ID
                            {
                                break;
                            }
                        }
                        _ = sleep(SHUTDOWN_FLUSH_CHUNK_DELAY) => {
                            if sender.send(InMsg::Audio { pcm: silence_chunk.clone() }).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            })
            .await;
        }

        let _ = sender.close().await;

        let mut send_loop = send_loop;
        match timeout(Duration::from_secs(2), async {
            loop {
                tokio::select! {
                    join_res = &mut send_loop => {
                        return join_res;
                    }
                    _ = out_rx.recv() => {
                    }
                }
            }
        })
        .await
        {
            Ok(join_res) => {
                join_res.map_err(|e| SttError::Message(e.to_string()))??;
            }
            Err(_) => {
                send_loop.abort();
            }
        }

        let mut recv_loop = recv_loop;
        match timeout(Duration::from_secs(2), &mut recv_loop).await {
            Ok(join_res) => {
                join_res.map_err(|e| SttError::Message(e.to_string()))??;
            }
            Err(_) => {
                recv_loop.abort();
            }
        }

        let mut keepalive_loop = keepalive_loop;
        match timeout(Duration::from_secs(2), &mut keepalive_loop).await {
            Ok(join_res) => {
                join_res.map_err(|e| SttError::Message(e.to_string()))??;
            }
            Err(_) => {
                keepalive_loop.abort();
            }
        }

        Ok(())
    }

    pub fn into_event_stream(self) -> SttEventStream {
        SttEventStream::new(self)
    }
}

pub struct SttEventStream {
    session: SttSession,
    transcript: TranscriptAssembler,
    pending: VecDeque<SttEvent>,
    utterance_text: String,
    utterance_finalize_delay: Duration,
    utterance_deadline: Option<Instant>,
    utterance_partial_min_interval: Duration,
    last_partial_emit: Option<Instant>,
}

impl SttEventStream {
    fn new(session: SttSession) -> Self {
        Self {
            session,
            transcript: TranscriptAssembler::new(),
            pending: VecDeque::new(),
            utterance_text: String::new(),
            utterance_finalize_delay: Duration::from_millis(1500),
            utterance_deadline: None,
            utterance_partial_min_interval: Duration::from_millis(100),
            last_partial_emit: None,
        }
    }

    pub fn utterance_finalize_delay(mut self, delay: Duration) -> Self {
        self.utterance_finalize_delay = delay;
        self
    }

    pub fn utterance_partial_interval(mut self, interval: Duration) -> Self {
        self.utterance_partial_min_interval = interval;
        self
    }

    pub fn sender(&self) -> SttSender {
        self.session.sender()
    }

    pub async fn recv(&mut self) -> Result<SttEvent> {
        loop {
            if let Some(ev) = self.pending.pop_front() {
                return Ok(ev);
            }

            if let Some(deadline) = self.utterance_deadline {
                tokio::select! {
                    _ = sleep_until(deadline) => {
                        if let Some(ev) = self.finalize_utterance() {
                            return Ok(ev);
                        }
                    }
                    msg = self.session.out_rx.recv() => {
                        let msg = msg.ok_or_else(|| SttError::Message("recv loop ended".to_string()))?;
                        self.handle_out_msg(msg);
                    }
                }
            } else {
                let msg = self
                    .session
                    .out_rx
                    .recv()
                    .await
                    .ok_or_else(|| SttError::Message("recv loop ended".to_string()))?;
                self.handle_out_msg(msg);
            }
        }
    }

    pub async fn shutdown(self) -> Result<()> {
        self.session.shutdown().await
    }

    fn handle_out_msg(&mut self, msg: OutMsg) {
        match msg {
            OutMsg::Ready => {
                self.pending.push_back(SttEvent::Ready);
            }
            OutMsg::Word { text, start_time } => {
                self.pending.push_back(SttEvent::WordReceived {
                    text: text.clone(),
                    start_ms: sec_to_ms(start_time),
                });

                if let Some(word) = self.transcript.push_word(text, start_time) {
                    self.push_word_finalized(word);
                }
            }
            OutMsg::EndWord { stop_time } => {
                if let Some(word) = self.transcript.push_end_word(stop_time) {
                    self.push_word_finalized(word);
                }
            }
            OutMsg::Step {
                step_idx,
                prs,
                buffered_pcm,
            } => {
                self.pending.push_back(SttEvent::VadStep {
                    step_idx,
                    prs,
                    buffered_pcm,
                });
            }
            OutMsg::Marker { id } => {
                self.pending.push_back(SttEvent::StreamMarker { id });
            }
            OutMsg::Error { message } => {
                self.pending.push_back(SttEvent::Error { message });
            }
        }
    }

    fn push_word_finalized(&mut self, word: crate::stt::types::WordTiming) {
        let word_text = word.word.clone();
        self.pending.push_back(SttEvent::WordFinalized(word));

        if !self.utterance_text.is_empty() {
            self.utterance_text.push(' ');
        }
        self.utterance_text.push_str(&word_text);
        let now = Instant::now();
        let should_emit = self.utterance_partial_min_interval.is_zero()
            || self
                .last_partial_emit
                .is_none_or(|last| now.duration_since(last) >= self.utterance_partial_min_interval);
        if should_emit {
            self.pending
                .push_back(SttEvent::UtterancePartial(Utterance {
                    text: self.utterance_text.clone(),
                }));
            self.last_partial_emit = Some(now);
        }

        self.utterance_deadline = Some(Instant::now() + self.utterance_finalize_delay);
    }

    fn finalize_utterance(&mut self) -> Option<SttEvent> {
        self.utterance_deadline = None;
        if self.utterance_text.is_empty() {
            return None;
        }

        let text = std::mem::take(&mut self.utterance_text);
        self.last_partial_emit = None;
        Some(SttEvent::UtteranceFinal(Utterance { text }))
    }
}

fn sec_to_ms(s: f64) -> u64 {
    if !s.is_finite() || s.is_sign_negative() {
        return 0;
    }

    (s * 1000.0).round() as u64
}

#[derive(Clone, Debug)]
pub struct SttSender {
    tx: mpsc::Sender<SendCmd>,
}

impl SttSender {
    pub async fn send(&self, msg: InMsg) -> Result<()> {
        self.tx
            .send(SendCmd::Msg(msg))
            .await
            .map_err(|_| SttError::Message("send loop task ended".to_string()))?;
        Ok(())
    }

    pub async fn close(&self) -> Result<()> {
        let _ = self.tx.send(SendCmd::Close).await;
        Ok(())
    }
}
