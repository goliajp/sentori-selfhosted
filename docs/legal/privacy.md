# Sentori — Privacy Policy

> **Draft template — Phase 16 sub-D placeholder.** Have a lawyer in
> your jurisdiction review and adjust before linking from marketing.
> Replace `<…>` placeholders with real entity / contact details.

_Last updated: <YYYY-MM-DD>_

This policy explains what data Sentori collects when you use the
hosted service at `sentori.golia.jp` (the "Service"), how we use it,
and what your rights are. It does not cover self-hosted Sentori
deployments — when you run Sentori on your own infrastructure, this
policy does not apply and you are the data controller.

## 1. Who we are

The Service is operated by `<entity name, address>` ("we", "us"). For
data-protection questions: `<privacy@golia.jp>`.

## 2. What data we collect

### From you, the dashboard user

- **Account data**: email address, password hash (argon2id), creation
  timestamp, IP address and user agent of each session.
- **Org data**: org slug, org name, your role, invites you sent or
  received.

### From your applications, via the SDK

- **Error events**: type, message, stack frames, breadcrumbs, tags,
  release identifier, environment, platform, device OS / version /
  locale / model, app version / build, anonymous user identifier.
- **What we deliberately do not collect**: real-name email addresses,
  phone numbers, IP addresses, or other directly identifying user
  fields. The protocol's `user` shape is `{ id?, anonymous? }` —
  anything outside that is rejected with `validationFailed` and never
  persisted. See `server/src/event.rs::User`.

## 3. How we use it

- Render your dashboard.
- Group, deduplicate, and notify you of errors (per project recipient
  settings).
- Enforce monthly free-tier quotas (event counts only — no payload
  inspection).
- Operate the Service securely (rate limiting, abuse mitigation).
- Send you transactional email: email verification, org invites,
  quota warnings.

We do not sell, rent, or trade personal data. We do not run
ads, analytics SDKs, or cross-site trackers on the dashboard.

## 4. Where it lives

- Postgres on `<region>` (Hetzner). Daily encrypted backups in
  Cloudflare R2 (`<region>`), 30-day retention.
- Logs in Grafana Cloud Loki (`<region>`), 30-day retention.
- TLS via Let's Encrypt, terminated at Caddy on the application VM.

## 5. How long we keep it

- Event payloads: free tier — 30 days.
- Audit / login logs: 90 days.
- Account data: until you delete your account, then up to 30 days of
  backup tail.

## 6. Your rights

If you are an EEA / UK resident under GDPR, or a California resident
under CCPA / CPRA, you can:

- Request a copy of all data attributable to your account (use
  `GET /api/orgs/{slug}/export` while logged in, or email us).
- Request deletion (delete every org you own from the dashboard, then
  email us if you also want server-side audit traces purged).
- Object to processing.
- Lodge a complaint with your local data-protection authority.

## 7. Sub-processors

- Cloudflare (DNS, R2 backups, edge CDN).
- Hetzner (compute).
- Let's Encrypt (TLS certificate authority).
- Grafana Cloud (Loki + Prometheus).
- Better Stack (uptime monitoring).
- An SMTP provider TBD (transactional mail).

## 8. Changes

If this policy materially changes, we'll email account owners at least
14 days before the change takes effect.
