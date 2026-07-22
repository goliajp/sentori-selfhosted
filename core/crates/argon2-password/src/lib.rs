//! # `sentori-argon2-password` — Argon2id password hasher
//!
//! Stone-tier (石头) crate. Wraps the `RustCrypto`
//! [`argon2`](https://docs.rs/argon2) crate with a typed
//! surface, mirroring `sentori-cookie-session::PasswordHash`'s
//! bcrypt shape:
//!
//! - [`PasswordHash::hash`] — hash at the default OWASP-2025
//!   parameters (Argon2id, m=19 MiB, t=2, p=1). Returns a
//!   PHC-format string (`$argon2id$v=19$m=19456,t=2,p=1$…`)
//!   suitable for direct storage in a `users.password_hash`
//!   column.
//! - [`PasswordHash::hash_with_params`] — same, but with a
//!   custom [`Params`].
//! - [`PasswordHash::verify`] — verify a candidate password
//!   against a stored PHC string. Constant-time on success and
//!   failure, time leakage limited to the cost factor that is
//!   already public in the stored string.
//!
//! ## Why this stone exists alongside S9's bcrypt `PasswordHash`
//!
//! Both algorithms are accepted by every contemporary security
//! review. They are NOT interchangeable in three places:
//!
//! 1. **Memory-hardness.** bcrypt's cost is purely CPU; an
//!    attacker with a GPU farm scales linearly with cores.
//!    Argon2id costs memory (`m_cost` KiB per parallel lane),
//!    which is ~10× more expensive on GPUs than CPUs.
//! 2. **Cost knobs.** bcrypt has one knob (cost factor 4..=31).
//!    Argon2 has three (memory, iterations, parallelism).
//!    Tuning Argon2 for a specific p99 latency budget is
//!    finer-grained than bcrypt's exponential rounds.
//! 3. **Input ceiling.** bcrypt silently uses only the first
//!    72 bytes of the password (we surface this as a hard
//!    error in S9). Argon2 has no such limit and we accept
//!    arbitrary-length input here (capped at
//!    [`MAX_PASSWORD_BYTES`] solely as a `DoS` defence).
//!
//! Pick one per call-site by importing the corresponding
//! crate. If you need to migrate from one to the other, do it
//! at the 钢筋 layer (`auth-session` opportunistic re-hash on
//! verify-success).
//!
//! ## Wire format (locked)
//!
//! Hashes are PHC-format
//! ([RFC-equivalent](https://github.com/P-H-C/phc-string-format)):
//! `$argon2id$v=19$m=<KiB>,t=<iters>,p=<lanes>$<b64-salt>$<b64-hash>`
//!
//! - Salt: 16 random bytes from the OS CSPRNG, per call.
//! - Hash: 32 bytes (the [`argon2`] default).
//! - Version: 0x13 (Argon2 1.3, the only spec-compliant one).
//!
//! Verifying a hash recovers the parameters from the string —
//! older / slower / faster hashes all keep verifying after a
//! [`Params`] bump.
//!
//! ## Quick start
//!
//! ```rust
//! use sentori_argon2_password::PasswordHash;
//!
//! # fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let stored = PasswordHash::hash("hunter2")?;
//! assert!(PasswordHash::verify("hunter2", &stored)?);
//! assert!(!PasswordHash::verify("hunter3", &stored)?);
//! # Ok(()) }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]

mod error;
mod hash;
mod params;

pub use error::{PasswordError, PasswordResult};
pub use hash::{MAX_PASSWORD_BYTES, PasswordHash};
pub use params::Params;
