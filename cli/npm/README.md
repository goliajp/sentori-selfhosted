# @goliapkg/sentori-cli

Thin npm wrapper around the [Sentori](https://sentori.golia.jp) Rust CLI. The package downloads the prebuilt binary for your platform on `npm install` (postinstall hook) so `npx @goliapkg/sentori-cli` works the moment you have it in `package.json`.

## Install

```sh
npm install -D @goliapkg/sentori-cli
# or pnpm / yarn

npx @goliapkg/sentori-cli upload sourcemap \
  --release myapp@1.2.3+456 \
  --token st_pk_... \
  --ingest-url https://ingest.sentori.your-host.com \
  dist/bundle.js.map
```

### Using bun

Bun blocks postinstall scripts by default. After `bun add`, run:

```sh
bun pm trust @goliapkg/sentori-cli
```

once to allow the binary download, or use `bun add --trust @goliapkg/sentori-cli` from the start.

Supported platforms:

| OS | Arch |
|----|------|
| Linux | x64, arm64 |
| macOS | x64 (Intel), arm64 (Apple Silicon) |

If your platform isn't covered, the postinstall hook will print a soft warning and you can fall back to:

```sh
cargo install --git https://github.com/goliajp/sentori --bin sentori-cli
```

## Skipping postinstall

`SENTORI_SKIP_DOWNLOAD=1` skips the binary download — useful in monorepo bootstrap or sandbox CI environments where the binary will be vendored separately.

## Verifying the binary

Each release artifact ships with a `.sha256` sidecar published next to the `.tar.gz` on the GitHub Release page; the postinstall script downloads only the tarball but you can manually verify:

```sh
curl -L https://github.com/goliajp/sentori/releases/download/cli-v<VERSION>/sentori-cli-cli-v<VERSION>-darwin-arm64.tar.gz.sha256
```

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) OR
[MIT](../../LICENSE-MIT).
