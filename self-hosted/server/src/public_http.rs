//! Public HTTP egress: validate every DNS answer, pin the connection, and bound all work.
//! Special ranges: <https://www.iana.org/assignments/iana-ipv4-special-registry/>
//! and <https://www.iana.org/assignments/iana-ipv6-special-registry/> (2026-10-10).
//! Deliberately excludes special-purpose ranges even where an anycast exception exists.
use reqwest::{Client, Url, header::LOCATION};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(matches!(a, 0 | 10 | 127)
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192
                    && (b == 168 || (b == 0 && matches!(c, 0 | 2)) || (b == 88 && c == 99)))
                || (a == 198 && (matches!(b, 18 | 19) || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}

pub fn parse(value: &str) -> Result<Url, &'static str> {
    if value.len() > 2048 {
        return Err("invalid_url");
    }
    let url = Url::parse(value).map_err(|_| "invalid_url")?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.host_str().is_none()
        || !matches!(url.port_or_known_default(), Some(80 | 443))
    {
        return Err("invalid_url");
    }
    let host = url.host_str().ok_or("invalid_url")?;
    let host = host.trim_matches(['[', ']']);
    if let Ok(ip) = host.parse::<IpAddr>() {
        if !public_ip(ip) {
            return Err("non_public_address");
        }
    } else if !host.contains('.')
        || [
            "localhost",
            ".localhost",
            ".local",
            ".internal",
            ".invalid",
            ".test",
        ]
        .iter()
        .any(|suffix| host.trim_end_matches('.').ends_with(suffix))
    {
        return Err("non_public_address");
    }
    Ok(url)
}

async fn pinned_client(url: &Url) -> Result<Client, &'static str> {
    let host = url
        .host_str()
        .ok_or("invalid_url")?
        .trim_matches(['[', ']']);
    let port = url.port_or_known_default().ok_or("invalid_url")?;
    let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| "dns_failed")?
        .take(33)
        .collect();
    if addresses.is_empty() || addresses.len() > 32 || addresses.iter().any(|a| !public_ip(a.ip()))
    {
        return Err("non_public_address");
    }
    // No ambient proxy can bypass the pinned destination. A new client per hop avoids
    // connection pooling across DNS generations. TLS still validates the original hostname.
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(host, &addresses)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .user_agent("sentori-monitor/5")
        .build()
        .map_err(|error| {
            tracing::warn!(%error, "public HTTP client initialization failed");
            "http_client_failed"
        })
}

pub async fn probe(value: &str, timeout: Duration) -> Result<u16, &'static str> {
    tokio::time::timeout(timeout, async {
        let mut url = parse(value)?;
        for hop in 0..=3 {
            let client = pinned_client(&url).await?;
            let mut response = client
                .get(url.clone())
                .send()
                .await
                .map_err(|_| "request_failed")?;
            if response.status().is_redirection() {
                if hop == 3 {
                    return Err("too_many_redirects");
                }
                let location = response
                    .headers()
                    .get(LOCATION)
                    .and_then(|h| h.to_str().ok())
                    .ok_or("invalid_redirect")?;
                let next = parse(url.join(location).map_err(|_| "invalid_redirect")?.as_str())?;
                if url.scheme() == "https" && next.scheme() != "https" {
                    return Err("insecure_redirect");
                }
                url = next;
                continue;
            }
            let code = response.status().as_u16();
            let mut size = 0;
            while let Some(chunk) = response.chunk().await.map_err(|_| "body_failed")? {
                size += chunk.len();
                if size > 65536 {
                    return Err("response_too_large");
                }
            }
            return Ok(code);
        }
        Err("too_many_redirects")
    })
    .await
    .map_err(|_| "timeout")?
}

pub struct PostFailure {
    pub code: &'static str,
    pub may_have_sent: bool,
}
pub struct PostResponse {
    pub status: u16,
    pub body: Vec<u8>,
}
/// No redirects for writes: authentication and signing material stay at the configured origin.
pub async fn post(
    value: &str,
    headers: reqwest::header::HeaderMap,
    body: Vec<u8>,
    read_body: bool,
) -> Result<PostResponse, PostFailure> {
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        let url = parse(value).map_err(|code| PostFailure {
            code,
            may_have_sent: false,
        })?;
        let client = pinned_client(&url).await.map_err(|code| PostFailure {
            code,
            may_have_sent: false,
        })?;
        post_with_client(&client, url, headers, body, read_body).await
    })
    .await;
    result.unwrap_or(Err(PostFailure {
        code: "timeout",
        may_have_sent: true,
    }))
}
async fn post_with_client(
    client: &Client,
    url: Url,
    headers: reqwest::header::HeaderMap,
    body: Vec<u8>,
    read_body: bool,
) -> Result<PostResponse, PostFailure> {
    let mut response = client
        .post(url)
        .headers(headers)
        .body(body)
        .send()
        .await
        .map_err(|e| PostFailure {
            code: "request_failed",
            may_have_sent: !e.is_connect(),
        })?;
    let status = response.status().as_u16();
    // Webhook acknowledgement is the HTTP status, not an optional response body.
    // Discard it immediately so a truncated body cannot repeat an acknowledged notification.
    if !read_body || !response.status().is_success() {
        return Ok(PostResponse {
            status,
            body: Vec::new(),
        });
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| PostFailure {
        code: "body_failed",
        may_have_sent: true,
    })? {
        if body.len() + chunk.len() > 65536 {
            return Err(PostFailure {
                code: "response_too_large",
                may_have_sent: true,
            });
        }
        body.extend_from_slice(&chunk);
    }
    Ok(PostResponse { status, body })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_addresses_are_never_public() {
        for value in [
            "0.1.2.3",
            "10.1.2.3",
            "100.100.100.200",
            "127.0.0.1",
            "169.254.169.254",
            "172.16.0.1",
            "172.31.255.255",
            "192.0.0.9",
            "192.0.2.1",
            "192.88.99.1",
            "192.168.1.1",
            "198.18.1.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "::ffff:8.8.8.8",
            "64:ff9b::808:808",
            "fc00::1",
            "fe80::1",
            "ff02::1",
            "2001::1",
            "2001:20::1",
            "2001:db8::1",
            "2002:0808:0808::1",
            "3fff::1",
            "5f00::1",
        ] {
            assert_eq!(value.parse().map(public_ip), Ok(false), "{value}");
        }
        for value in [
            "8.8.8.8",
            "1.1.1.1",
            "2001:4860:4860::8888",
            "2606:4700:4700::1111",
        ] {
            assert_eq!(value.parse().map(public_ip), Ok(true), "{value}");
        }
    }

    #[test]
    fn rejects_url_bypasses_and_credentials() {
        for value in [
            "file:///etc/passwd",
            "http://2130706433",
            "http://0x7f000001",
            "http://127.1",
            "http://[::ffff:127.0.0.1]",
            "http://user:pass@example.com",
            "http://example.com:22",
            "http://localhost.",
            "http://metadata.google.internal",
            "http://example.com/#secret",
        ] {
            assert!(parse(value).is_err(), "{value}");
        }
        assert!(parse("https://example.com/health?ready=1").is_ok());
    }
}
