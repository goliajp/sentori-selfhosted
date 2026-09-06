//! Argon2id cost parameters.

use crate::error::{PasswordError, PasswordResult};

/// Argon2 cost knobs.
///
/// Three parameters; bounds checked at construction time so an
/// out-of-spec value can never reach the argon2 backend.
///
/// ## Picking defaults
///
/// The [`Params::OWASP_2025`] constant is the OWASP Cheat Sheet
/// (April 2025 revision) recommendation for general-purpose
/// password hashing:
/// `m_cost = 19 MiB`, `t_cost = 2`, `p_cost = 1`.
///
/// For online login flows where every ms of CPU matters, use
/// [`Params::INTERACTIVE`] (~50 ms on a 2024 server core):
/// `m_cost = 12 MiB`, `t_cost = 2`, `p_cost = 1`.
///
/// For sensitive offline secrets (key-wrap KEKs, master
/// passwords), use [`Params::SENSITIVE`] (~500 ms):
/// `m_cost = 64 MiB`, `t_cost = 3`, `p_cost = 1`.
///
/// Tune to your hardware. The numbers above are conservative
/// for 2025 server-grade CPUs; raise as Moore's Law allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    /// Memory cost in **KiB** (matches the argon2 crate's
    /// `m_cost` field). Min 8, max 4 GiB.
    pub m_cost: u32,
    /// Iteration / time cost. Min 1.
    pub t_cost: u32,
    /// Parallelism (lane count). Min 1, max 16 — beyond ~4
    /// lanes diminishing returns kick in on commodity CPUs.
    pub p_cost: u32,
}

impl Params {
    /// OWASP 2025 default. `m=19 MiB`, `t=2`, `p=1`.
    pub const OWASP_2025: Self = Self {
        m_cost: 19 * 1024,
        t_cost: 2,
        p_cost: 1,
    };

    /// Interactive login. `m=12 MiB`, `t=2`, `p=1`. ~50 ms / op
    /// on a 2024 server core.
    pub const INTERACTIVE: Self = Self {
        m_cost: 12 * 1024,
        t_cost: 2,
        p_cost: 1,
    };

    /// Sensitive / key-derivation. `m=64 MiB`, `t=3`, `p=1`.
    /// ~500 ms / op on a 2024 server core.
    pub const SENSITIVE: Self = Self {
        m_cost: 64 * 1024,
        t_cost: 3,
        p_cost: 1,
    };

    /// Minimum-cost params for tests that need fast hashes.
    /// **Never** use in production — `m=8 KiB`, `t=1`, `p=1`
    /// runs in microseconds and offers ~zero security margin.
    pub const TEST_FAST: Self = Self {
        m_cost: 8,
        t_cost: 1,
        p_cost: 1,
    };

    /// Validate that `self` is within argon2's accepted ranges.
    ///
    /// # Errors
    ///
    /// [`PasswordError::InvalidParams`] for any out-of-range
    /// knob. See the [`Params`] field docs for bounds.
    pub const fn validate(&self) -> PasswordResult<()> {
        if self.m_cost < 8 {
            return Err(PasswordError::InvalidParams("m_cost < 8 KiB"));
        }
        if self.m_cost > 4 * 1024 * 1024 {
            return Err(PasswordError::InvalidParams("m_cost > 4 GiB"));
        }
        if self.t_cost == 0 {
            return Err(PasswordError::InvalidParams("t_cost == 0"));
        }
        if self.p_cost == 0 {
            return Err(PasswordError::InvalidParams("p_cost == 0"));
        }
        if self.p_cost > 16 {
            return Err(PasswordError::InvalidParams("p_cost > 16"));
        }
        Ok(())
    }

    /// Convert to the argon2 crate's native params type.
    pub(crate) fn to_argon2(self) -> Result<argon2::Params, PasswordError> {
        argon2::Params::new(self.m_cost, self.t_cost, self.p_cost, None).map_err(Into::into)
    }
}

impl Default for Params {
    /// Returns [`Self::OWASP_2025`].
    fn default() -> Self {
        Self::OWASP_2025
    }
}
