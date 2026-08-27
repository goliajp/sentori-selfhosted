---
title: Source map upload from CI
description: GitHub Actions / GitLab CI / Vercel build hook recipes
---

# Source map upload from CI

The CLI is `@goliapkg/sentori-cli` (npm). `sentori-cli upload sourcemap`
takes a release name + files or directories and POSTs them to your
Sentori instance. Directories are scanned (one level) for `.map` /
`.js` / `.jsbundle` / `.bundle` / `.hbc` files; the server dedupes by
sha256, so re-runs are cheap. No install needed in CI — `npx` it:

```bash
npx @goliapkg/sentori-cli@latest upload sourcemap \
  --release "myapp@1.2.3+456" \
  --token "$SENTORI_TOKEN" \
  --api-url "$SENTORI_API_URL" \
  dist/assets/
```

`--token` falls back to `$SENTORI_TOKEN`, `--api-url` to
`$SENTORI_API_URL` (default `https://sentori.golia.jp`; for a
self-hosted instance, your host). `--ingest-url` is accepted as an
alias for `--api-url`. `--dry-run` lists what would be uploaded.

> **The release string must match.** `--release` here has to be byte-for-byte
> what the SDK reports via `init({ release })` (e.g. `myapp@1.2.3+456`).
> If they differ the dashboard silently can't symbolicate — see the
> "no source map for release X" / "release mismatch" hints on the issue
> page.

Below: how to wire this into the common CI surfaces (the recipes below
install the CLI globally; `npx` works too).

## GitHub Actions

`.github/workflows/deploy.yml`:

```yaml
name: Deploy
on:
  push:
    branches: [main]
jobs:
  build-and-upload:
    runs-on: ubuntu-latest
    env:
      SENTORI_TOKEN: ${{ secrets.SENTORI_TOKEN }}
      SENTORI_INGEST_URL: https://ingest.sentori.golia.jp
      RELEASE: myapp@${{ github.ref_name }}+${{ github.run_number }}
    steps:
      - uses: actions/checkout@v4
      - uses: oven-sh/setup-bun@v2
      - run: bun install --frozen-lockfile
      - run: bun run build
      - name: Install sentori-cli
        run: |
          npm install -g @goliapkg/sentori-cli
      - name: Upload sourcemaps
        run: sentori-cli upload sourcemap --release "$RELEASE" dist/assets/
      - name: Notify of deploy
        run: |
          curl -fsS -X POST "$SENTORI_INGEST_URL/v1/deploys" \
            -H "Authorization: Bearer $SENTORI_TOKEN" \
            -H "Content-Type: application/json" \
            -d "{\"release\":\"$RELEASE\",\"environment\":\"prod\"}"
      # ... actual deploy step (Vercel CLI / AWS S3 sync / Fly deploy / ...)
```

Notes:

- `actions/checkout@v4` is sufficient — we don't need `fetch-depth:
  0`. The release name comes from `github.ref_name` + `run_number`.
- The install step caches naturally on subsequent runs because the
  GHA runner image is fresh per job but `curl` is fast (~1 s).
- Put the upload **before** the deploy step so any release the user
  sees is already symbolicatable. If the upload fails (network /
  ingest down), failing the deploy job is the safe default.

## GitLab CI

`.gitlab-ci.yml`:

```yaml
stages: [build, deploy]

variables:
  SENTORI_INGEST_URL: "https://ingest.sentori.golia.jp"

build:
  stage: build
  image: oven/bun:1
  script:
    - bun install --frozen-lockfile
    - bun run build
  artifacts:
    paths: [dist/]
    expire_in: 1 day

upload-sourcemaps:
  stage: deploy
  image: oven/bun:1
  needs: [build]
  script:
    - npm install -g @goliapkg/sentori-cli
    - export PATH="$HOME/.sentori/bin:$PATH"
    - export RELEASE="myapp@$CI_COMMIT_REF_NAME+$CI_PIPELINE_IID"
    - sentori-cli upload sourcemap --release "$RELEASE" dist/assets/
    - |
      curl -fsS -X POST "$SENTORI_INGEST_URL/v1/deploys" \
        -H "Authorization: Bearer $SENTORI_TOKEN" \
        -H "Content-Type: application/json" \
        -d "{\"release\":\"$RELEASE\",\"environment\":\"prod\"}"
  rules:
    - if: '$CI_COMMIT_BRANCH == "main"'
```

`SENTORI_TOKEN` lives in GitLab's CI/CD variables (Settings →
CI/CD → Variables, mark "Masked"). `CI_PIPELINE_IID` is the
per-project pipeline number — stable + monotonic.

## Vercel build hook

Vercel doesn't expose a separate "post-build" step in the dashboard,
but `package.json#scripts.build` is a normal shell command:

```json
{
  "scripts": {
    "build": "next build && bun run upload-sourcemaps",
    "upload-sourcemaps": "node ./scripts/upload-sourcemaps.mjs"
  }
}
```

`scripts/upload-sourcemaps.mjs`:

```js
#!/usr/bin/env node
import { execSync } from 'node:child_process'

const release = `myapp@${process.env.VERCEL_GIT_COMMIT_SHA?.slice(0, 7) ?? 'local'}`
const token = process.env.SENTORI_TOKEN
if (!token) {
  console.warn('[sentori] SENTORI_TOKEN unset, skipping')
  process.exit(0) // don't fail the build on a missing local token
}

execSync(
  `sentori-cli upload sourcemap --release "${release}" .next/static/chunks/`,
  { stdio: 'inherit' },
)

// Deploy ping — Vercel sets VERCEL_ENV to 'production'|'preview'|'development'
execSync(
  `curl -fsS -X POST "$SENTORI_INGEST_URL/v1/deploys" \
    -H "Authorization: Bearer ${token}" \
    -H "Content-Type: application/json" \
    -d '{"release":"${release}","environment":"${process.env.VERCEL_ENV ?? 'preview'}"}'`,
  { shell: '/bin/bash', stdio: 'inherit' },
)
```

Add `SENTORI_TOKEN` + `SENTORI_INGEST_URL` to Vercel project →
Settings → Environment Variables (scope: Production + Preview).

For source-map generation specifically:

```js
// next.config.js
module.exports = {
  productionBrowserSourceMaps: true,
}
```

## React Native / Expo (Hermes)

A React Native release build is double-minified: Metro bundles your JS
(emitting `*.packager.map`), then Hermes compiles that to bytecode
(emitting `*.hbc.map`). The frames you get in production point at the
*bytecode* offset, so the **composed** map is what Sentori needs.
`sentori-cli react-native upload` does the compose + upload in one step:

```bash
# produce the bundle + the Metro source map (the native iOS/Android
# build then compiles to Hermes and writes main.jsbundle.hbc.map)
npx react-native bundle \
  --platform ios --dev false --entry-file index.js \
  --bundle-output main.jsbundle \
  --sourcemap-output main.jsbundle.packager.map

# compose (Metro + Hermes maps) and upload — one command
npx @goliapkg/sentori-cli@latest react-native upload \
  --release "myapp@$VERSION+$BUILD" --token "$SENTORI_TOKEN" \
  --metro-map main.jsbundle.packager.map \
  --hermes-map main.jsbundle.hbc.map \
  --bundle main.jsbundle
```

Do this once per platform (the iOS and Android bundles differ). The
`--release` value must equal what the app passes to `init({ release })`.

If you'd rather compose by hand and upload separately:

```bash
node node_modules/react-native/scripts/compose-source-maps.js \
  main.jsbundle.packager.map main.jsbundle.hbc.map -o main.jsbundle.map
npx @goliapkg/sentori-cli@latest upload sourcemap \
  --release "myapp@$VERSION+$BUILD" main.jsbundle.map main.jsbundle
```

**Expo / EAS:** `@goliapkg/sentori-expo` ships an EAS post-build hook
(`@goliapkg/sentori-expo/eas-post-build`) that runs step 3 against
`./dist` after `expo export --source-maps`. Wire it from `eas.json`'s
`build.<profile>.hooks.postPublish` with `{ "options": { "release":
"..." } }`, and set `SENTORI_ADMIN_TOKEN` in EAS secrets. For a *Hermes*
EAS build you still need the compose step (1–2 above) before the hook —
run it from a custom build script.

## Verifying

After an upload, the release detail page in the dashboard
(`/org/<slug>/releases/<encoded-release>`) shows:

- a "Source maps: N files" line
- per-file size + sha256
- the first 5 events to land against this release with a "preview
  symbolicated frame" panel

If the upload succeeded but events still show minified frames, check
that the release name on the event matches the upload exactly —
case-sensitive, `+build` suffix included.
