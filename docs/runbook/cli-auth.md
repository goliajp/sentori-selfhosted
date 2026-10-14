# CLI authentication

For installation and the complete release workflow, see the [CLI guide](../cli.md).
This page describes current token scopes, not the retired v0 token kinds.

## Choose the destination and token

Create a token in the target project's token settings and select **API** scope.
The token is shown once. Store it in your CI secret store; do not put the plaintext
in source control, the CLI configuration, an application bundle or a screenshot.

| Scope | Use |
| --- | --- |
| `ingest` | Application SDK events and telemetry |
| `api` | CLI uploads, artifact inventory and project API operations |

An ingest token is refused by artifact endpoints with `admin_token_required`.
The token prefix is not a substitute for its recorded scope. A project token
cannot choose another project by passing a project ID.

For **Cloud**, use the destination workspace's runtime URL and a token from its
project. The account/billing website and its session cookie are not CLI
credentials. For **self-hosted**, use the public base URL of that instance.
Supply HTTPS in CI; loopback HTTP is intended for local development only.

## Unified workflow

`sentori init` writes `tokenEnv: "SENTORI_TOKEN"`, never the secret. `doctor`,
`upload all` and `verify` read the variable named by `tokenEnv`; these commands do
not accept `--token` or implicitly use `SENTORI_ADMIN_TOKEN`.

```sh
export SENTORI_TOKEN='your-api-scope-token'
export SENTORI_RELEASE='my-app@1.2.3+42'
npx --no-install sentori doctor --config sentori.config.json
npx --no-install sentori upload all --config sentori.config.json --strict
npx --no-install sentori verify --config sentori.config.json
```

Keep the URL, configuration and release identical across all three commands.
URL precedence is `--api-url`, `SENTORI_API_URL`, then config `apiUrl`.
An expired/revoked token, wrong workspace or rejected quota is a failure;
ordinary uploads may still exit zero to protect the host build. Use `--strict`
in a dedicated required upload job, and check `verify` independently.

## Existing individual commands

The individual `upload sourcemap`, `upload dsym`, `upload mapping` and
`upload srcbundle` commands use `--token`, then `SENTORI_TOKEN`. They do not read
the unified configuration. Use `--api-url` or `SENTORI_API_URL` explicitly.
`--ingest-url` remains an alias; it does not mean the separate ingest host is
necessarily the right destination.

Issue/MCP commands retain the legacy fallback order `--token`,
`SENTORI_ADMIN_TOKEN`, `SENTORI_TOKEN`. `SENTORI_ADMIN_TOKEN` is an environment
variable name, not a separate server-side token kind. Prefer `SENTORI_TOKEN`
with API scope when configuring new integrations. `probes sync` uses the same
credential order as individual uploads.

## Rotate a CI credential

1. Create a new API-scope token in the same project with a recognizable CI label.
2. Replace the secret in the CI secret store.
3. Run `doctor`, upload a build's debug files and require `verify` to pass.
4. Revoke the old token from the project's token list.

Revocation prevents future operations; it does not remove artifacts already
uploaded. Keep application ingest tokens separate from build-time API tokens.
