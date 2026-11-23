//! HTTP with polite retries.
use serde::de::DeserializeOwned;
use std::io::Read;
use std::sync::OnceLock;
use std::time::Duration;

pub struct Body {
    pub status: u16,
    pub length: Option<u64>,
    pub reader: Box<dyn Read + Send>,
}

fn agent() -> &'static ureq::Agent {
    static A: OnceLock<ureq::Agent> = OnceLock::new();
    A.get_or_init(|| {
        ureq::Agent::config_builder()
            .user_agent(concat!("RunIt/", env!("CARGO_PKG_VERSION")))
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(20)))
            .timeout_recv_response(Some(Duration::from_secs(60)))
            .tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
            .build()
            .into()
    })
}

/// Open a URL, following redirects. `from` and `to` ask for a byte range.
/// Tries twice more when the connection drops or the server says it is busy.
pub fn open(url: &str, from: u64, to: Option<u64>) -> Result<Body, String> {
    let mut tries = 0;
    loop {
        let mut r = agent().get(url);
        if from > 0 || to.is_some() {
            r = r.header("Range", format!("bytes={from}-{}", to.map_or(String::new(), |t| t.to_string())));
        }
        let resp = match r.call() {
            Ok(resp) => resp,
            // A connection kept open from earlier may have been closed at the other end.
            Err(_) if tries < 2 => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(300 * tries));
                continue;
            }
            Err(e) => return Err(format!("could not reach the server: {e}")),
        };
        let status = resp.status().as_u16();
        if matches!(status, 429 | 503) && tries < 2 {
            let wait = resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(2u64);
            std::thread::sleep(Duration::from_secs(wait.min(30)) + Duration::from_millis(100 * (tries + 1)));
            tries += 1;
            continue;
        }
        if !(200..300).contains(&status) {
            return Err(match status {
                404 | 410 => "this file is not available right now".into(),
                401 | 403 => "this file needs permission from its publisher".into(),
                429 | 503 => "the server is busy, try again in a minute".into(),
                s => format!("the server answered {s}"),
            });
        }
        let length = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
        return Ok(Body { status, length, reader: Box::new(resp.into_body().into_reader()) });
    }
}

pub fn text(url: &str) -> Result<String, String> {
    let mut s = String::new();
    open(url, 0, None)?.reader.take(64 << 20).read_to_string(&mut s).map_err(|e| format!("the connection dropped: {e}"))?;
    Ok(s)
}

pub fn json<T: DeserializeOwned>(url: &str) -> Result<T, String> {
    serde_json::from_str(&text(url)?).map_err(|e| format!("unexpected answer from the server: {e}"))
}

/// Percent-encode a query value or path segment.
pub fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}
