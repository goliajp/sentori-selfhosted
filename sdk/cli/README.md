# Sentori CLI

Configure, collect, upload and verify the debug files from an application build.
The package is `@goliapkg/sentori-cli`; its command is **`sentori`**.
`sentori-cli` remains an alias for existing scripts. The CLI runs on the build
machine and does not add runtime work to the application.

## Install the correct version

This guide requires **CLI 1.6.0**. Check registry availability before updating
your build pipeline. The older 1.5.0 package has only the
`sentori-cli` executable and does not have `init`, `doctor`, `upload all`, ZIP
packaging or exact-file `verify`. Do not install 1.5.0 and run this guide's commands.

Use Node **18.17 or newer**; use a maintained Node release for CI. Pin the CLI as a
development dependency and commit the package lock:

```sh
npm view @goliapkg/sentori-cli@1.6.0 version
npm install --save-dev --save-exact @goliapkg/sentori-cli@1.6.0
npx --no-install sentori --version
```

The last command must print `sentori 1.6.0`. If npm cannot find that version, use
the tested package supplied by the Sentori team, without assuming `latest` has
these features:

```sh
npm install --save-dev --save-exact /absolute/path/goliapkg-sentori-cli-1.6.0.tgz
npx --no-install sentori --version
```

For maintainers preparing that package, from the Sentori repository root with
its pinned Bun installed:

```sh
bun install --frozen-lockfile
bun run --cwd sdk/cli build
npm pack ./sdk/cli --ignore-scripts --pack-destination /tmp
```

Consumers need Node/npm, not Bun. A local dev dependency is not added to an
interactive shell's PATH: use `npx --no-install sentori` as below, or `sentori`
inside an npm script. `--no-install` prevents npx from downloading a different
package if the local executable is missing. The server maintenance binary
`sentori-operator` is a separate tool.

## Compatibility and version identity

| Value | Meaning |
| --- | --- |
| CLI `1.6.0` | Build-tool package version, shown by `sentori --version` |
| Server version | Running server binary, reported by `sentori doctor` |
| SDK version | Independently versioned app library; it need not equal the CLI or server |
| Config `version: 1` | Upload configuration schema, not a product version |
| App `release` | Exact string reported by the SDK, including build number |
| Git tag | Optional source revision; it does not set the app release |

The current server advertises artifact protocol `1`, source formats `zip` and
`json`, and `sha256`/`format` verification on the existing authenticated artifact
inventory API. These fields are additive: old clients keep using that API and
legacy path-to-content JSON remains accepted.

CLI 1.6 checks capabilities rather than guessing from a server version. An older
build can have the same displayed server version without ZIP support. This
repository's server changes must be deployed before using source ZIPs against
that instance; changing the CLI alone is not enough.

| Server response | CLI 1.6 behavior |
| --- | --- |
| Protocol 1, ZIP and verification supported | Unified upload and verification, including sources |
| No capability advertisement | Source-map/dSYM/mapping workflows remain available; a ZIP plan stops before any upload |
| ZIP absent, unsupported protocol or incomplete verification support | Relevant workflow fails with an upgrade/compatibility message before uploading |
| Missing/false `usable`, wrong hash or wrong filename | Exact-file verification fails; stored bytes alone are not success |

Keep an existing CLI 1.5 pipeline pinned until its server is upgraded if it needs
legacy JSON sources. CLI 1.6 does not silently convert ZIPs to JSON or drop sources.
A new CLI also cannot reconstruct missing debug files from an SDK version: the
files must come from the actual application build being diagnosed.

## Cloud and self-hosted destinations

The same package and commands support both editions. Choose the destination
before running `init`:

| Edition | `--api-url` | Credential |
| --- | --- | --- |
| Sentori Cloud | The target workspace's runtime URL, shown when opening that workspace | API-scope token created in that workspace's project |
| Self-hosted | Your instance's public HTTPS base URL | API-scope token created in that instance's project |

The Cloud account/billing website is not the upload destination. Do not use its
login cookie, the SDK ingest hostname, or a token from another workspace. Open
the project, create an API token in its token settings, and supply it through
`SENTORI_TOKEN` or the configured `tokenEnv`. `doctor` must accept that exact
URL/token pair before uploading. Cloud storage/traffic quotas still apply;
429 is a failed upload, not a verified artifact. Resolve the quota, rerun the
same release, and require `verify` to pass. Revoking an API token stops future
CLI operations; rotate CI secrets before revocation.

Self-hosted operators must upgrade their server to a compatible release before
using ZIP sources. Cloud operators deploy the compatible data-plane image;
customers only need to update the CLI. Both use the capability checks below,
without assuming the CLI, SDK and server have matching version numbers.

## Configure once per build target

Run in the application or monorepo root. Choose only the files that target
produces. For a web build that writes maps under `dist`:

```sh
npx --no-install sentori init --api-url https://sentori.your-domain.com \
  --sourcemap dist --sources src
```

For iOS, use `--dsym build/MyApp.app.dSYM --sources Sources`. For Android, use
`--mapping app/build/outputs/mapping/release/mapping.txt --sources app/src/main`.
For React Native, also select the build's final composed JS source map; source
ZIPs cannot replace it. Adjust these example paths to the actual build outputs.

`init` creates `sentori.config.json` and refuses to overwrite it. A minimal web
configuration is:

```json
{
  "version": 1,
  "apiUrl": "https://sentori.your-domain.com",
  "tokenEnv": "SENTORI_TOKEN",
  "artifacts": { "sourcemap": ["dist"] },
  "sources": { "root": ".", "include": ["src", "shared"], "exclude": ["shared/tests"] }
}
```

Remove `shared` if it does not exist. Select only application/shared sources
needed for stack context. Dependencies, hidden paths, common build outputs and
symlinks are excluded; never select an entire monorepo indiscriminately. Keep
`.sentori/` out of version control because it contains the generated source ZIP.
Use `sourceArchive: "sources.zip"` instead of `sources` for an existing ZIP.

Paths in configuration are relative to the configuration file. Source includes
and excludes are relative to `sources.root`. For a configuration in an app
subdirectory that needs shared monorepo files, set `sources.root` to the monorepo
root relative to that configuration and use root-relative include paths.

Use separate configurations such as `sentori.ios.config.json` and
`sentori.android.config.json` when platforms have different releases or outputs.
Pass `--config` to **every** command, including `doctor` and `verify`. The API-scope
token determines the project; these commands do not need a project UUID. Different
projects require their own tokens.

## Check, build, upload, verify

Supply an **API-scope** token through the environment or the CI secret store. An
app's public ingest token cannot upload debug files. Never commit the token;
`init` stores only the environment variable name.

```sh
export SENTORI_TOKEN='your-api-scope-token'
export SENTORI_RELEASE='my-app@1.2.3+42'
npx --no-install sentori doctor
# Build the app here, with the same release string and source revision.
npx --no-install sentori config validate
npx --no-install sentori upload all --dry-run
npx --no-install sentori upload all
npx --no-install sentori verify
```

`doctor` checks authentication and configured server capabilities, even before
build outputs exist. It does not upload files or check build identity.
`config validate` and `upload all --dry-run` check local files and print the plan;
they do not contact the server. `upload all` checks compatibility before the first
write, attempts all selected files, then reads the server inventory back.
`verify` independently requires matching file kinds, names (except server-named
dSYM slices), SHA-256 bytes and successful format validation for the same local
plan. An older file of the same kind does not satisfy it.

Release precedence is `--release`, then `SENTORI_RELEASE`, then config `release`.
URL precedence is `--api-url`, then `SENTORI_API_URL`, then config `apiUrl`.
A Git override `--git-tag` takes precedence over `sources.gitTag`. Repeat the same
overrides for upload and verification; otherwise you may check a different build.
Individual legacy commands take their own arguments and do not read this config.

The app release must exactly match the SDK, including build number; it is never
guessed from `package.json`. Successful byte/format verification does **not** prove
that a binary and source revision match, or that every event can be symbolicated.
Native frames still need matching dSYM/debug IDs or mappings; source context needs
a resolved file path and line number. Supported servers reprocess stored events
after late uploads.

## CI and exit codes

Ordinary upload failures return zero to protect the host build, while printing
what failed. `--strict` opts into a nonzero upload result. `doctor`, `config
validate` and `verify` return nonzero when their checks fail. `--dry-run` is valid
only with `upload all`, so it cannot silently bypass a verification gate.

For a dedicated required debug-artifact job, after the application build:

```sh
# SENTORI_TOKEN and SENTORI_RELEASE are supplied by CI; URL is in the config.
npx --no-install sentori doctor --config sentori.ios.config.json
npx --no-install sentori config validate --config sentori.ios.config.json
npx --no-install sentori upload all --config sentori.ios.config.json --strict
npx --no-install sentori verify --config sentori.ios.config.json
```

Keep this job separate from compilation if upload outages must not block app
builds. For nonblocking upload hooks omit `--strict`; run `verify` separately to
notice missing files. `artifacts check --expect ...` is the older kind-presence
check and cannot replace exact-file `verify`. A receipt with unknown validation
is not a passing gate. After a partial failure, correct the cause and rerun the
same configuration and release.

## Source ZIPs without Git

Preserve project-relative paths so modules with identical basenames remain
distinct. Supported source types include Swift, Kotlin, Java, C/C++, Objective-C,
JS/TS, Vue/Svelte, Rust, Go, Python, Ruby, PHP, C# and Dart.

```sh
npx --no-install sentori sources pack --root . --release 'my-app@1.2.3+42' \
  --exclude shared/tests --output sources.zip src shared
npx --no-install sentori upload srcbundle --api-url https://sentori.your-domain.com \
  --release 'my-app@1.2.3+42' sources.zip --strict
```

The pack command writes a standard ZIP plus `sentori-source-manifest.json`
containing the release, file hashes, sizes and source provenance. It refuses to
overwrite an existing output. Limits: 10,000 files, 512 KiB per file, 64 MiB total
source text. Oversized/unreadable selected files cause an error, not truncation.
A plain ZIP of UTF-8 sources is also accepted; without a manifest, the uploader
is responsible for selecting the correct release. Include generated sources by
explicit file paths when needed; do not archive the whole output directory.

## Pin the source revision

Git is optional. In Git mode `sources.root` must be the repository root, and the
selected tag must already be available locally. Both lightweight and annotated
tags are supported:

```sh
npx --no-install sentori upload all --git-tag v1.2.3 --release 'my-app@1.2.3+42'
npx --no-install sentori verify --git-tag v1.2.3 --release 'my-app@1.2.3+42'
```

This reads `refs/tags/v1.2.3` at its resolved commit and records tag and commit in
the manifest. It does not check out that commit, fetch tags, include dirty files
or guess the latest tag. The app build, debug files and chosen tag must come from
the same revision. Build-generated sources require directory mode from that build
workspace. An existing `sourceArchive` cannot be combined with `--git-tag`.

## Individual uploads and Hermes

These commands remain available alongside the unified workflow:

```sh
npx --no-install sentori upload sourcemap --api-url https://sentori.your-domain.com \
  --release "$SENTORI_RELEASE" ./dist
npx --no-install sentori upload dsym --api-url https://sentori.your-domain.com \
  --release "$SENTORI_RELEASE" ./MyApp.app.dSYM
npx --no-install sentori upload mapping --api-url https://sentori.your-domain.com \
  --release "$SENTORI_RELEASE" ./mapping.txt
npx --no-install sentori react-native upload --api-url https://sentori.your-domain.com \
  --release "$SENTORI_RELEASE" --metro-map metro.map --hermes-map hermes.map
```

A source-map directory preserves relative filenames; individual files retain their
basename. Legacy individual dSYM upload uses macOS `dwarfdump`, or explicit
`--debug-id` and `--arch`; unified `upload all` passes the Mach-O binaries from the
`.dSYM` bundle and lets the server identify slices.

For Hermes, use the final composed map emitted by the build in
`artifacts.sourcemap`. If the build emits separate Metro and Hermes maps, use
`react-native upload` to compose them with the application's installed React
Native tooling. A Metro map alone does not resolve Hermes bytecode offsets.

## Troubleshooting

| Result | Action |
| --- | --- |
| No `sentori` binary / unknown `init` | Check the installed package version; use 1.6.0 or its supplied tarball, with `npx --no-install` |
| `doctor` rejects ZIP capability | Upgrade/deploy the compatible server; CLI installation cannot enable server support |
| 401 / `admin_token_required` | Use an API-scope token for the correct project and instance |
| Local file missing | Build first; resolve paths relative to the configuration file |
| ZIP manifest release mismatch | Repack from the intended build with exactly the SDK release string |
| Missing exact bytes / format not verified | Rerun the same upload, inspect individual failures, and verify again |
| Files verified, frame still unresolved | Check actual release, dSYM/debug identity, composed map, source path and line |

Use `npx --no-install sentori --help` for issue, probe, MCP and push commands.

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) OR
[MIT](../../LICENSE-MIT).
