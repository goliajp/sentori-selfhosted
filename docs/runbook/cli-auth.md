# sentori-cli auth — canonical (locked v0.x)

This is the single source of truth for how `@goliapkg/sentori-cli`
authenticates. Linked from the user-facing recipes and from
support tickets so we don't drift across messages.

## What tokens exist

All Sentori tokens share the prefix `st_pk_…`. Kind is enforced
server-side in the `tokens` table:

| Kind | Where issued | Typical use |
|---|---|---|
| `public` | Dashboard → Project Settings → Tokens (default kind on the create form) | SDK `init({ token })` for `POST /v1/events/...` |
| `admin` | Dashboard → Project Settings → Tokens (kind=admin checkbox) | `sentori-cli upload sourcemap`, `sentori-cli issue list/resolve/silence`, any `/admin/api/...` |

Both kinds use the same `st_pk_` prefix on the wire — **you cannot
tell them apart by inspection**. The dashboard's tokens page is the
only place that surfaces kind. We've decided not to embed kind in
the prefix (`st_pk_admin_…` etc.) because revocation already needs
a DB lookup and prefix-embedded metadata grows stale.

## v0.x admin enforcement: lenient

`/admin/api/...` accepts any unrevoked row in `tokens` regardless of
kind. A `public` token works on admin routes today. **This will
change in v1.0** — admin routes will reject `kind=public` with 403.

If you're building a CLI integration today: issue a `kind=admin`
token now even though `kind=public` also works. That way the v1.0
flip is a no-op for you.

## CLI token resolution

`sentori-cli` looks up its bearer token in this exact order:

1. `--token <value>` flag
2. `SENTORI_ADMIN_TOKEN` env
3. `SENTORI_TOKEN` env
4. error: `token: pass --token or set SENTORI_ADMIN_TOKEN / SENTORI_TOKEN`

When you set both `SENTORI_ADMIN_TOKEN` (kind=admin) and
`SENTORI_TOKEN` (kind=public for ingest), the CLI prefers the admin
one — which is what you want, since CLI ops live on the admin path.

## CLI base-URL resolution

`sentori-cli` looks up its API base URL in this exact order:

1. `--api-url <value>` flag (also accepts `--ingest-url` as alias)
2. `SENTORI_API_URL` env
3. default `https://sentori.golia.jp`

Until 2026-08-27 this list had `SENTORI_ADMIN_URL` at step 2 and a
host-stripping `SENTORI_INGEST_URL` at step 3. Neither exists in
`sdk/cli/src/index.ts`; setting the first did nothing and the second
was never read. `scripts/check-env-vars-real.mjs` now holds every
`SENTORI_*` a document names to one the code reads.

v2.4 consolidation collapsed the dashboard / admin-api / docs hosts
into one root. The CLI now hits `https://sentori.golia.jp/admin/api/...`
for everything. The SDK still posts to `ingest.sentori.golia.jp/v1/*`
(kept as its own host so the customer SDK tokens issued before v2.4
keep working without a forced re-init). Legacy hosts
`app.sentori.golia.jp` and `api.sentori.golia.jp` 301-redirect to the
new root, so CLI calls against the old URLs still work — but the
canonical naming is now just `sentori.golia.jp`.

## Recommended call-site pattern (release.ts / build hooks)

```ts
// env-primary with tenant fallback. CI sets the env from secrets;
// local dev / one-off verifies use the tenant config.
const token =
  process.env.SENTORI_ADMIN_TOKEN ??
  process.env.SENTORI_TOKEN ??
  readTenantSentoriToken(tenantId)
if (!token) {
  log.warn(`tenant ${tenantId} has no sentori token configured — skipping upload`)
  return
}
```

Why this shape, not env-only or tenant-only:

- **Env-only** breaks local dev because every contributor would need
  to mirror a CI secret to their shell before testing the release
  pipeline. Onboarding cost compounds.
- **Tenant-only** ships secrets in the repo (even if encrypted, the
  decryption key still has to live somewhere). For prod CI a
  rotated-via-secrets-manager env is the right level.
- **Env primary with tenant fallback** keeps secrets out of the repo
  for production while giving local dev a frictionless path.

## EAS / Expo

`@goliapkg/sentori-expo` ships an EAS post-build hook
(`@goliapkg/sentori-expo/eas-post-build`). EAS Secrets only inject
into the build environment, so the env-primary path lights up
naturally — add `SENTORI_ADMIN_TOKEN` (kind=admin) to EAS Secrets
and the hook picks it up.

## Token rotation playbook

1. Dashboard → Project Settings → Tokens
2. Click `+ New token`, pick kind=admin, give a CI-identifying label
   (e.g. `gh-actions-rotate-2026q2`)
3. Plaintext shows once — paste into the secrets store (GitHub
   Actions secrets / EAS Secrets / Vault). It hashes immediately;
   you can't read it from the dashboard again.
4. Old token row still in DB — revoke from the tokens list when
   the new one is live and you've verified one successful CLI run.

Audit trail of revoke + create operations lives in the org's audit
log if you ever need to prove "this CI run used the token issued at
T1, not the one issued at T0."
