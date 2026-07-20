//! [`AuthOptions`] — tuning knobs for [`crate::AuthService`].

use time::Duration;

/// Tuning knobs. Construct via [`AuthOptions::default`] for
/// production defaults; override individual fields where needed.
#[derive(Debug, Clone, Copy)]
pub struct AuthOptions {
    /// Minimum password length (in chars, not bytes — counted as
    /// `str::chars().count()`). Default 8.
    pub password_min_chars: usize,

    /// Session TTL. Default 30 days.
    pub session_ttl: Duration,

    /// Email-verify token TTL. Default 24 hours.
    pub email_verify_ttl: Duration,

    /// Password-reset token TTL. Default 2 hours.
    pub password_reset_ttl: Duration,

    /// Cookie name used by [`crate::axum::build_session_cookie`]
    /// and [`crate::axum::require_user`]. Default `sentori_session`.
    pub cookie_name: &'static str,

    /// If true, cookies are emitted with the `Secure` flag.
    /// Disable for `http://localhost` dev only.
    pub cookie_secure: bool,
}

impl Default for AuthOptions {
    fn default() -> Self {
        Self {
            password_min_chars: 8,
            session_ttl: Duration::days(30),
            email_verify_ttl: Duration::hours(24),
            password_reset_ttl: Duration::hours(2),
            cookie_name: "sentori_session",
            cookie_secure: true,
        }
    }
}
