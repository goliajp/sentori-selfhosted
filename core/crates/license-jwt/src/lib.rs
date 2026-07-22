//! # `sentori-license-jwt` — Sentori license JWT signer + verifier
//!
//! 石头级 crate (per cement-stone): 业务无关、跨项目可用、semver 严格。
//! Sentori v0.1 fresh-start 第一个 ship 的 crate。
//!
//! ## 用途
//!
//! 给 sentori-saas (per `.claude/state/product-architecture.html` §05.2) 提供 license 机制:
//!
//! - **签发** — saasadmin 在 GOLIA 私有 infra 用 Ed25519 私钥签发 JWT, 编码
//!   tenant_id / tier / features / expires_at / jti 等 claim。
//! - **校验** — saas server (或 v0.2+ enterprise binary) 中间件用 public key 验 signature +
//!   exp + jti revocation list。
//! - **Grace period** — exp 字段 baked-in grace seconds (saas: 7d, enterprise: 90d), verify
//!   时不需额外逻辑。
//! - **Revoke** — 黑名单 jti 集合, in-memory `HashSet<String>` + db 持久化 (sentori-saas crate
//!   层面 wire valkey broadcast)。
//!
//! ## 算法选择: Ed25519
//!
//! 锁定 EdDSA / Ed25519 (per sprint-0/S4 PoC):
//!
//! - 32-byte public key (最小)
//! - Constant-time signing + verify (timing-attack 安全)
//! - 跟 sentori-react-native SDK 加密栈一致 (ed25519-dalek 已用)
//! - 比 RS256 快 ~3-5x verify, 比 HS256 安全 (asymmetric, 公钥可分发)
//!
//! ## Quick start
//!
//! ```rust
//! use sentori_license_jwt::{Issuer, Verifier, LicenseClaims, Tier};
//! use ed25519_dalek::SigningKey;
//! use time::{Duration, OffsetDateTime};
//! use uuid::Uuid;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Load signing key (production: from vault). Doctest uses a fixed
//! //    32-byte seed — NEVER do this in production.
//! let signing_key = SigningKey::from_bytes(&[7u8; 32]);
//! let verifying_key = signing_key.verifying_key();
//!
//! // 2. Issuer signs license
//! let issuer = Issuer::new(signing_key)?;
//! let tenant_id = Uuid::now_v7();
//! let claims = LicenseClaims::saas_tenant(
//!     tenant_id,
//!     Tier::Pro,
//!     "sub_xxx".into(),
//!     OffsetDateTime::now_utc() + Duration::days(30),
//!     Duration::days(7),  // grace
//! );
//! let token = issuer.sign(&claims)?;
//!
//! // 3. Verifier (server middleware) validates
//! let verifier = Verifier::new(verifying_key);
//! let decoded = verifier.verify(&token)?;
//! assert_eq!(decoded.tenant_id(), Some(tenant_id));
//! assert_eq!(decoded.tier, Tier::Pro);
//! # Ok(())
//! # }
//! ```
//!
//! ## Cement-stone 工艺等级
//!
//! - **石头**: 业务无关 ✓, 跨项目 (saas + enterprise + future products) ✓, semver 严格 ✓
//! - Acceptance per `.claude/state/v0.1-execution-plan.md` §C:
//!   - line cov > 95%, branch > 90%, mutation > 85%
//!   - criterion bench baseline (sign / verify / revoke check per op)
//!   - 完整 rustdoc + doctest (本文件)
//!   - `forbid(unsafe_code)`, `clippy::pedantic + nursery + unwrap_used + todo + unimplemented`
//!
//! ## 设计决策记录
//!
//! - **clock skew leeway**: 30 秒, jsonwebtoken `Validation::leeway` 默认值合理
//! - **issuer string**: 固定 `"sentori.golia.jp"`, 所有 sentori 部署共用一致 issuer 串
//!   (verify 时强制 match — 防外部 token 冒充)
//! - **jti format**: UUID v7 (per `feedback_default_stack` 用户全局偏好)
//! - **Tier serialization**: lowercase string (`"free"`, `"pro"`, `"enterprise-cloud"`,
//!   `"enterprise-self"`) — JSON-friendly + 跟 product-architecture.html §05.4 一致
//! - **Limits**: nullable u64 字段 (`None` = unlimited)
//! - **Features**: `Vec<String>` v0.1 留空, v0.2+ 用于 SSO/SLA/白标 等 feature flag 解锁

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Crate docs intentionally mix English narrative with Chinese context
// notes and brand names ("Sentori", "v0.1", etc.) that clippy's
// doc_markdown heuristic flags as missing-backtick identifiers.
#![allow(clippy::doc_markdown)]

mod claims;
mod error;
mod issuer;
mod verifier;

pub use claims::{Edition, LicenseClaims, LicenseLimits, Tier};
pub use error::{LicenseError, LicenseResult};
pub use issuer::Issuer;
pub use verifier::{RevocationList, Verifier};

/// JWT issuer / audience constant — 所有 sentori 部署共用,
/// verify 时强制 match 防 cross-issuer token 冒充。
pub const SENTORI_ISSUER: &str = "sentori.golia.jp";
