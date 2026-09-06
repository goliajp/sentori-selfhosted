# docs/

Source markdown for the protocol spec, getting started, the SDK
guide, self-hosting and troubleshooting. **These files are the
documentation** — read them here or in the OSS mirror.

There was an Astro Starlight site (`docs-site/`) and a marketing site
(`marketing/`) in this repo until 2026-08-10. Nothing routed to
either: `docs.sentori.golia.jp` redirects to the dashboard and
`sentori.golia.jp/docs/` is the SPA's catch-all. Between them they
documented a Sentry compatibility layer this product deliberately does
not have, four web-framework SDKs whose source is not in this repo,
and a Free / Pro / Enterprise pricing page for a product with no
signup and no billing. Thirty thousand files, built by no gate and
served to nobody — the OSS mirror excluded them, so not even a
self-hoster ever saw them. Deleted; the history has them.

## Start here

If you are integrating, in this order:

1. [`getting-started.md`](getting-started.md) — install, first event,
   and a curl that is executed by a gate so it cannot rot.
2. [`getting-started/react-native.md`](getting-started/react-native.md)
   — the SDK path end to end.
3. [`protocol.md`](protocol.md) — the wire schema, batching, token
   format, sourcemap upload.
4. [`errors.md`](errors.md) — every error code this server sends.
   Generated from the handlers; do not edit by hand.
5. [`troubleshooting.md`](troubleshooting.md) — the failure modes
   people actually hit, with the hints the server really sends.

A running instance also serves [`/llms.txt`](../webapp/public/llms.txt),
which carries enough to send a first event without following any link.

## SDK reference

The current API surface ships with the package and is what npm shows:
[`sdk/react-native/README.md`](../sdk/react-native/README.md).

There is one SDK guide because there is one SDK. Sentori watches
mobile apps.

## Self-hosting

- [`self-hosting.md`](self-hosting.md) — environment variables,
  backup / restore, Postgres upgrade notes.
- [`teams.md`](teams.md) — accounts, roles, project assignment.
- [`runbook/backup-restore.md`](runbook/backup-restore.md)
- [`runbook/scaling.md`](runbook/scaling.md)

## Recipes

- [`recipes/sourcemap-upload.md`](recipes/sourcemap-upload.md)
- [`recipes/release-versioning.md`](recipes/release-versioning.md)

## Internal

Not published to the OSS mirror: `design/`, `plans/`, `roadmap/`,
`dogfood/`, `performance/`, `perf-baselines/`, and the rest of
`runbook/`.

## Archive

[`archive/`](archive/) holds pages that describe versions and features
that no longer exist — the pre-v1 SDK reference, guides for web
packages that are not on npm, an error catalogue for an API that was
never built, and one integrator's upgrade notes. Its own README says
what each tree was.

Nothing in `archive/` is documentation. It is kept because deleting
history makes the next person repeat it, and it is a directory rather
than a banner because
[`scripts/check-docs-api-truth.mjs`](../scripts/check-docs-api-truth.mjs)
has to be able to tell the difference mechanically.
