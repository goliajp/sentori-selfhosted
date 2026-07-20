# Sentori v0.1 — `core/` (石头 + 钢筋)

跨 `saas/` + `self-hosted/` 共享的 Rust workspace。

按 [cement-stone methodology](https://github.com/goliajp/global-config/blob/master/methodology/steel-cement-stone.md) 分类:

- **石头 (stone)** — 业务无关、跨项目可用、semver 严格、cov > 95% + bench + mutation
  - `crates/license-jwt/` — JWT signer + verifier + grace + revoke
  - `crates/privacy-salt/` — SHA256 namespacing
  - `crates/issue-fingerprint/` — error group key hashing
  - `crates/event-ringbuffer/` — lock-free MPSC ingest buffer
  - `crates/stripe-webhook-verify/` — Stripe signature verification
  - `crates/sourcemap-resolver/` `crates/dwarf-resolver/` `crates/proguard-resolver/` — symbolication
  - `crates/cookie-session/` — secure cookie + bcrypt + CSRF
  - `crates/rate-limiter/` — sliding window with valkey
  - `crates/geoip-reader/` — maxminddb wrapper
  - `crates/secrets-vault/` — AES-256-GCM + HKDF

- **钢筋 (steel)** — 业务领域感知不绑死业务流, cov > 85%
  - `crates/workspace-identity/` — workspace_members + project_user_visibility 模型
  - `crates/auth-session/` — session middleware + login + email verify
  - `crates/attachment-store/` — local-fs / S3 trait + impl
  - `crates/event-pipeline/` — ingest → ringbuffer → grouping → store
  - `crates/issue-store/` — issue table + denorm + regression detection
  - `crates/span-store/` — distributed tracing storage
  - `crates/push-provider/` — APNs/FCM/Web Push/HCM/MiPush trait + impls
  - `crates/replay-store/` — wireframe blob + delta encoding
  - `crates/runtime-metrics/` — partition lifecycle + rollup cron
  - `crates/cert-monitor/` — TLS expiry watch
  - `crates/notifier/` — email + Slack/Linear/Jira routing
  - `crates/integration-traits/` — Linear/Slack/Jira outbound
  - `crates/audit-event/` — project-scoped audit log
  - `crates/alert-rule/` — basic alert evaluation
  - `crates/saved-view/` — user-scoped saved query
  - `crates/tenant-scoping/` — schema-per-tenant middleware (saas-only 但 lib 归 core 因 reusable)
  - `crates/billing/` — subscription state machine + license JWT mapping

## 不在 `core/`

- **水泥 (cement)** — saas-specific 业务流在 `saas/server/src/handlers/*.rs`;
  selfhosted-specific 在 `self-hosted/server/src/handlers/*.rs`。

## 工艺标准

每 crate ship 前必须 ✅ 全 acceptance per `.claude/state/v0.1-execution-plan.md` §C build checklist:

- `cargo build/test/clippy(-D warnings)/fmt/audit/deny/doc/doctest` 全 green
- 石头: line cov > 95% + branch > 90% + mutation > 85% + criterion bench baseline
- 钢筋: line cov > 85% + integration test 覆盖 happy + 2 error path
- module-level rustdoc (50+ 行讲业务上下文)

CI workflow `.github/workflows/v0.1-core.yml` 每 commit 验全部。
