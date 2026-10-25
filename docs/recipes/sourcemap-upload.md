---
title: Debug files in CI
description: Upload and verify the exact debug files produced by an application release
---

# Debug files in CI

Use [CLI 1.6](../cli.md) to configure one build target, upload its source maps,
native symbols and optional source ZIP, then verify the exact stored bytes.
Commit `sentori.config.json` and a lockfile pinning
`@goliapkg/sentori-cli@1.6.0`. Do not commit the API token or `.sentori/` archives.

For Cloud, `apiUrl` is the target workspace's runtime URL. For self-hosted it is
the instance URL. The Cloud billing site and separate ingest hostname are not
substitutes. [Authentication](../runbook/cli-auth.md) explains API scope and rotation.

## Prepare the application

From the application root, after selecting your actual output paths:

```sh
npm install --save-dev --save-exact @goliapkg/sentori-cli@1.6.0
npx --no-install sentori init --api-url https://sentori.example.com \
  --sourcemap dist --sources src
```

Change `dist` to the build directory that contains your `.map` files. Directory
uploads recurse and preserve relative filenames; two routes named `index.js.map`
remain distinct. JavaScript bundles without maps do not replace source maps.
Use only sources you intend to upload. The source ZIP cannot reconstruct a
missing mapping between minified code and original lines.

Set **one exact release string** in both the application SDK and the upload job,
including any build number. The CLI does not inject it into the app for you.
Build files and source revision must belong to that release.

## GitHub Actions

A dedicated required artifact job can use the following steps after checking
out the application and restoring the exact build outputs:

```yaml
- uses: actions/setup-node@v4
  with:
    node-version: '24'
- run: npm ci
- name: Verify and upload release debug files
  env:
    SENTORI_TOKEN: ${{ secrets.SENTORI_TOKEN }}
    SENTORI_RELEASE: ${{ needs.build.outputs.release }}
  run: |
    npx --no-install sentori doctor
    npx --no-install sentori config validate
    npx --no-install sentori upload all --dry-run
    npx --no-install sentori upload all --strict
    npx --no-install sentori verify
```

This is a step fragment, not a complete workflow: your `build` job must publish
its release value and debug outputs. Restore them at the paths recorded in the
configuration. Use the same commit's sources, or a locally available exact Git
tag with `--git-tag` on both upload and verification. Do not derive a second
release string independently in this job.

## GitLab CI

Pin the CLI in the application's package lock. Have the `build` job retain the
debug outputs and pass the release string through a dotenv artifact:

```yaml
upload-debug-files:
  stage: deploy
  image: node:24
  needs:
    - job: build
      artifacts: true
  script:
    - npm ci
    - npx --no-install sentori doctor
    - npx --no-install sentori upload all --strict
    - npx --no-install sentori verify
```

The upstream build must provide `SENTORI_RELEASE`; set `SENTORI_TOKEN` as a masked
CI/CD secret. Keep this job separate from compilation if a reporting outage
must not interrupt producing the application binary.

## Nonblocking build hooks and Vercel

Run the pinned local executable after the build has produced its debug files:

```json
{
  "scripts": {
    "build": "next build && npm run sentori:upload",
    "sentori:upload": "sentori upload all",
    "sentori:verify": "sentori verify"
  }
}
```

Configure the real source-map output directory and set `SENTORI_TOKEN` and
`SENTORI_RELEASE` in the build environment. Normal upload failures report the
problem but return zero, preserving the host build. Run `sentori:verify` in a
separate verification step; exit zero from the upload hook alone proves nothing.
`doctor`, `config validate` and `verify` intentionally fail on errors. Adding
`--strict` to the upload opts into a required gate.

## Native and React Native builds

- **iOS:** select the `.dSYM` bundle produced by the archived app using
  `--dsym`, and any source directories separately. Source ZIPs cannot replace
  the matching native debug IDs.
- **Android:** select the release's R8 `mapping.txt` using `--mapping`. Use a
  separate configuration if its release string differs from iOS.
- **React Native / Hermes:** put the final composed map in
  `artifacts.sourcemap`. If your build retains separate Metro and Hermes maps,
  compose and upload them with `sentori react-native upload --metro-map ...
  --hermes-map ... --release ...`. Metro maps alone do not resolve Hermes offsets.
- **Expo / EAS:** retain the integration from the
  [React Native guide](../../sdk/react-native/README.md). The CLI runs in the
  build environment with API credentials; do not expose these in public Expo
  application configuration.

Use `--config sentori.ios.config.json` (or the Android configuration) on every
workflow command. All paths are relative to that configuration file. See the
[CLI guide](../cli.md) for complete commands and Git-tag/source ZIP rules.

## What success proves

`verify` requires every planned file's kind, name, SHA-256 and server format
validation to match. An older file of the same kind is insufficient. This does
not prove the debug files came from the app binary or that a particular stack
can resolve; check the matching release and a real event as well.

A failed upload can be retried with the same configuration and release. Servers
reprocess existing events after a late artifact upload. Missing capability,
wrong token, wrong workspace, quota rejection and unreadable files must be fixed
before treating verification as complete.

## Function names in Vite 8

Sentori reads the mapped token's `names` entry when present. A map can restore
file, line and source context while omitting the original function name;
`sourcesContent` alone is not a function-scope map. A nonempty `names` array
does not guarantee a name for every stack position.

For Vite 8's Rolldown build, preserve function and class names in future builds:

```ts
build: {
  sourcemap: 'hidden',
  rolldownOptions: { output: { keepNames: true } },
}
```

This preserves runtime names rather than reconstructing missing names in an
old artifact. Upload the new build's maps with exactly the SDK release string;
keep maps private. `hidden` controls the sourceMappingURL comment, not names.
See [Vite build options](https://vite.dev/config/build-options) and
[Rolldown keepNames](https://rolldown.rs/reference/Interface.OutputOptions#keepnames).
