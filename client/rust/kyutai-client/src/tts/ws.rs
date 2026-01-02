use crate::tts::error::Result;
use crate::tts::protocol::InMsg;
use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use kyutai_client_core::ws::{WsStream, connect_ws, build_ws_url};
use tokio_tungstenite::tungstenite::Message;
use url::Url;

pub struct TtsClientBuilder {
    url: String,
    auth_token: Option<String>,
}

impl TtsClientBuilder {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            auth_token: None,
        }
    }

    pub fn auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    pub async fn connect(self) -> Result<TtsSession> {
        let url = Url::parse(&self.url).map_err(|e| crate::tts::error::TtsError::Message(e.to_string()))?;
        let ws_url = build_ws_url(
            url.as_str(),
            "",
            &[],
            self.auth_token.as_deref()
        ).context("Failed to build WS URL").map_err(|e| crate::tts::error::TtsError::Message(e.to_string()))?;

        let stream = connect_ws(&ws_url, None).await.map_err(|e| crate::tts::error::TtsError::Ws(e.to_string()))?;

        Ok(TtsSession { stream })
    }
}

pub struct TtsSession {
    stream: WsStream,
}

impl TtsSession {
    pub async fn send_text(&mut self, text: &str) -> Result<()> {
        self.stream.send(Message::Text(text.into())).await.map_err(|e| crate::tts::error::TtsError::Ws(e.to_string()))?;
        // Some servers expect a binary message with 0u8 to signal end of text or start of request
        self.stream.send(Message::Binary(vec![0u8].into())).await.map_err(|e| crate::tts::error::TtsError::Ws(e.to_string()))?;
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<Option<InMsg>> {
        while let Some(msg) = self.stream.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    let in_msg = rmp_serde::from_slice::<InMsg>(&data)
                        .map_err(|e| crate::tts::error::TtsError::Serialization(e.to_string()))?;
                    return Ok(Some(in_msg));
                }
                Ok(Message::Text(text)) => {
                    // Sometimes JSON is sent?
                    if let Ok(in_msg) = serde_json::from_str::<InMsg>(&text) {
                        return Ok(Some(in_msg));
                    }
                }
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => continue,
                Ok(Message::Close(_)) => return Ok(None),
                Err(e) => return Err(crate::tts::error::TtsError::Ws(e.to_string())),
                _ => continue,
            }
        }
        Ok(None)
    }
}
