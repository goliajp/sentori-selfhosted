//! Request-scoped source IP, accepting forwarding only from explicitly trusted peers.
use axum::{
    extract::{ConnectInfo, Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

tokio::task_local! { static SOURCE_IP: Option<IpAddr>; }

/// Exact proxy addresses. Empty configuration trusts no proxy.
#[derive(Debug, Default)]
pub struct TrustedProxies(Vec<IpAddr>);
impl TrustedProxies {
    /// Parse a comma-separated list of IP addresses; invalid configuration fails startup.
    /// # Errors
    /// Returns an error for a non-IP entry (hostnames and broad implicit trust are unsupported).
    pub fn parse(value: &str) -> Result<Self, std::net::AddrParseError> {
        value
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<IpAddr>().map(canonical))
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }

    /// Use the last X-Forwarded-For hop only when the TCP peer is trusted.
    /// Missing or malformed last hops fall back to the actual peer, never an earlier header value.
    #[must_use]
    pub fn resolve(&self, peer: IpAddr, headers: &HeaderMap) -> IpAddr {
        let peer = canonical(peer);
        if !self.0.contains(&peer) {
            return peer;
        }
        headers
            .get_all("x-forwarded-for")
            .iter()
            .next_back()
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.rsplit(',').next())
            .and_then(|s| s.trim().parse::<IpAddr>().ok())
            .map_or(peer, canonical)
    }
}
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v) => v.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        IpAddr::V4(_) => ip,
    }
}
/// Source for this request; background operations have no originating request.
#[must_use]
pub fn current() -> Option<String> {
    SOURCE_IP
        .try_with(|ip| ip.map(|v| v.to_string()))
        .ok()
        .flatten()
}

/// Establish context around the complete handler future; never trust headers without a socket peer.
pub async fn middleware(
    State(policy): State<Arc<TrustedProxies>>,
    req: Request,
    next: Next,
) -> Response {
    let ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|p| policy.resolve(p.0.ip(), req.headers()));
    SOURCE_IP.scope(ip, next.run(req)).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn forwarding_requires_trusted_peer_and_uses_only_last_hop() {
        let policy = TrustedProxies::parse("192.0.2.10, ::1").unwrap();
        let mut h = HeaderMap::new();
        h.insert(
            "x-forwarded-for",
            "203.0.113.99, 198.51.100.7".parse().unwrap(),
        );
        assert_eq!(
            policy
                .resolve("192.0.2.10".parse().unwrap(), &h)
                .to_string(),
            "198.51.100.7"
        );
        assert_eq!(
            policy
                .resolve("192.0.2.11".parse().unwrap(), &h)
                .to_string(),
            "192.0.2.11"
        );
        h.append("x-forwarded-for", "2001:db8::7".parse().unwrap());
        assert_eq!(
            policy
                .resolve("::ffff:192.0.2.10".parse().unwrap(), &h)
                .to_string(),
            "2001:db8::7"
        );
        for bad in ["198.51.100.7,", "198.51.100.7, unknown", "garbage"] {
            h.insert("x-forwarded-for", bad.parse().unwrap());
            h.insert("x-real-ip", "203.0.113.99".parse().unwrap());
            assert_eq!(
                policy
                    .resolve("192.0.2.10".parse().unwrap(), &h)
                    .to_string(),
                "192.0.2.10"
            );
        }
        assert!(TrustedProxies::parse("*").is_err());
        assert!(TrustedProxies::parse("10.0.0.0/8").is_err());
    }
    #[tokio::test]
    async fn context_is_request_scoped_and_does_not_leak() {
        assert!(current().is_none());
        SOURCE_IP
            .scope(Some("192.0.2.1".parse().unwrap()), async {
                assert_eq!(current().as_deref(), Some("192.0.2.1"));
                assert!(tokio::spawn(async { current() }).await.unwrap().is_none());
            })
            .await;
        assert!(current().is_none());
    }
}
