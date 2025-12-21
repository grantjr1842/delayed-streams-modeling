use anyhow::Result;
use url::Url;

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
