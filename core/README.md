# `core/` — 石头 + 钢筋

`self-hosted/server` 组合的共享 Rust workspace。

按 [cement-stone methodology](https://github.com/goliajp/global-config/blob/master/methodology/steel-cement-stone.md) 分类。下面这份清单和 `core/crates/` 里真实存在的目录一一对应,分类取自每个 crate 自己 `Cargo.toml` 顶部的声明 —— 这份 README 上一版列了 27 个 crate,其中 16 个在 v1 重构里已经删掉,读起来却和真实目录结构一模一样。

- **石头 (stone)** — 业务无关、跨项目可用、semver 严格
  - `crates/privacy-salt/` — HKDF-SHA256 + HMAC-SHA256 per-tenant per-purpose PII hasher
  - `crates/identity-fingerprint/` — scope-salted SHA-256,PII-free 跨会话身份
  - `crates/issue-fingerprint/` — 稳定的 group-key hash
  - `crates/event-ringbuffer/` — bounded lock-free MPMC,drop-oldest 溢出策略
  - `crates/sourcemap-resolver/` `crates/dwarf-resolver/` `crates/proguard-resolver/` — 符号化(JS / Mach-O DWARF / Android R8)
  - `crates/cookie-session/` — HMAC 签名 + AES-GCM 加密 cookie primitives
  - `crates/argon2-password/` — Argon2id password hasher
  - `crates/rate-limiter/` — backend-agnostic sliding-window,进程内 `MemoryBackend`
  - `crates/geoip-reader/` — MaxMind `.mmdb` 纯内存 reader
  - `crates/secrets-vault/` — AES-256-GCM + HKDF 信封加密

- **钢筋 (steel)** — 业务领域感知,不绑死业务流
  - `crates/ingest-token/` — `st_pk_<26 base32>` store + parse + axum Bearer middleware
  - `crates/attachment-store/` — content-addressed blob store(trait + local-fs impl)
  - `crates/push-provider/` — trait + dispatcher + tokens + credentials
  - `crates/notifier/` — Email + Webhook + Mock transports + delivery log

## 不在 `core/`

- **水泥 (cement)** — 具体业务流在 `self-hosted/server/src/handlers/*.rs`。
- 曾经有过一个 `saas/` 面,v1 重构(`94bd8758`,2026-08-01)把它连同十六个 crate 一起删了。

## 门

`core/` 的检查由 `.github/workflows/v0.2-core-check.yml`(push 到 `feature/**` / `fix/**` / `hotfix/**` / `develop` / `master` 且命中 `core/**` 时)和本地 `bun run preflight` 共同覆盖,两边都跑:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo check --tests`
- `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` — 断掉的 intra-doc link 是「文档在指一个不存在的东西」的探测器,这份 README 的上一版就是死在没人做这件事上
