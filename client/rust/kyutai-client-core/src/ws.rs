use anyhow::Result;
use url::Url;

#[cfg(feature = "ws")]
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
#[cfg(feature = "ws")]
use tokio::net::TcpStream;
#[cfg(feature = "ws")]
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
#[cfg(feature = "ws")]
use tokio_tungstenite::tungstenite::http::HeaderValue;
#[cfg(feature = "ws")]
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;

#[cfg(feature = "ws")]
pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub fn build_ws_url(
    base: &str,
    path: &str,
    query: &[(&str, &str)],
    token: Option<&str>,
) -> Result<Url> {
    let mut url = Url::parse(base)?;
    if !path.is_empty() {
        url.set_path(path);
    }

    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in query {
            pairs.append_pair(key, value);
        }
        if let Some(token) = token {
            pairs.append_pair("token", token);
        }
    }

    Ok(url)
}

#[cfg(feature = "ws")]
pub async fn connect_ws(
    url: &Url,
    auth_token: Option<&str>,
) -> Result<WsStream> {
    let mut req = url
        .to_string()
        .into_client_request()?;

    if let Some(token) = auth_token {
        let header_value = HeaderValue::from_str(&format!("Bearer {token}"))?;
        req.headers_mut().insert(AUTHORIZATION, header_value);
    }

    let (ws_stream, _resp) = connect_async(req).await?;

    Ok(ws_stream)
}

pub fn redact_ws_url(url: &Url) -> String {
    let mut url = url.clone();
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| {
            if k == "token" {
                (k.to_string(), "REDACTED".to_string())
            } else {
                (k.to_string(), v.to_string())
            }
        })
        .collect();

    url.query_pairs_mut().clear().extend_pairs(pairs);
    url.to_string()
}
