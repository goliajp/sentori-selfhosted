# @goliapkg/sentori-cli

Node-based CLI for [Sentori](https://sentori.golia.jp). Two surfaces:

- **Source-map upload** for web bundlers + Hermes (React Native) builds
- **Issue triage** (list / resolve / silence / comment) — usable as
  a Model Context Protocol (MCP) server so Claude / Cursor / Aider
  can act on your error inbox

## Install

```sh
bun add -D @goliapkg/sentori-cli
# or invoke transiently with bunx / npx
```

## Source-map upload

```sh
sentori-cli upload sourcemap \
  --release "my-app@1.2.3" \
  --token "$SENTORI_TOKEN" \
  ./dist
```

React Native (Android, raw maps still on disk):

```sh
sentori-cli react-native upload \
  --release "$APPLICATION_ID@$VERSION+$BUILD" \
  --metro-map  android/app/build/intermediates/sourcemaps/react/release/index.android.bundle.packager.map \
  --hermes-map android/app/build/intermediates/sourcemaps/react/release/index.android.bundle.compiler.map \
  --bundle     android/app/build/generated/assets/react/release/index.android.bundle
```

## Issue triage (MCP)

```sh
sentori-cli mcp serve --project "$PROJECT_UUID" --token "$SK_TOKEN"
```

Then point a MCP-compatible agent (Claude Code config / Cursor / etc.)
at the spawned stdio server.

→ Full reference: `sentori-cli --help`
→ Recipes: [sentori.golia.jp/docs/recipes/sourcemap-upload](https://sentori.golia.jp/docs/recipes/sourcemap-upload)

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) OR
[MIT](../../LICENSE-MIT).
