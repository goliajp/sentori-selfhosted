# @goliapkg/sentori-svelte

Svelte 4 / 5 / SvelteKit SDK for [Sentori](https://sentori.golia.jp).
Wires the global error hooks + the SvelteKit `handleError` adapter
in one call.

## Install

```sh
bun add @goliapkg/sentori-svelte @goliapkg/sentori-javascript
```

## Use

```ts
// src/lib/sentori.ts
import { initSentoriSvelte } from '@goliapkg/sentori-svelte'

export const { handleError } = initSentoriSvelte({
  token: 'st_pk_…',
  release: 'my-app@1.2.3',
  environment: 'prod',
})
```

```ts
// src/hooks.client.ts
export { handleError } from '$lib/sentori'
```

```ts
// src/hooks.server.ts
export { handleError } from '$lib/sentori'
```

→ Full guide: [sentori.golia.jp/docs/sdk-svelte](https://sentori.golia.jp/docs/sdk-svelte)
→ Sentry drop-in: [sentori.golia.jp/docs/sentry-compat](https://sentori.golia.jp/docs/sentry-compat)

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) OR
[MIT](../../LICENSE-MIT).
