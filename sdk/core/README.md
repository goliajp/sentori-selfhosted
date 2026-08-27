# @goliapkg/sentori-core

Wire-format types, ring buffers (breadcrumbs + spans), uuid v7, trace
context, stack parsers — the shared core every `@goliapkg/sentori-*`
SDK builds on top of.

You almost never import this directly. Pick the framework adapter
that fits your app:

| Framework | Package |
|---|---|
| React (web) | `@goliapkg/sentori-react` |
| React Native | `@goliapkg/sentori-react-native` |
| Next.js | `@goliapkg/sentori-next` |
| Vue 3 | `@goliapkg/sentori-vue` |
| Svelte / SvelteKit | `@goliapkg/sentori-svelte` |
| SolidJS | `@goliapkg/sentori-solid` |
| Expo | `@goliapkg/sentori-expo` |
| Plain JS / Node | `@goliapkg/sentori-javascript` |

→ Docs: [sentori.golia.jp/docs](https://sentori.golia.jp/docs)
→ Wire protocol: [sentori.golia.jp/docs/protocol](https://sentori.golia.jp/docs/protocol)

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) OR
[MIT](../../LICENSE-MIT).
