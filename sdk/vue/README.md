# @goliapkg/sentori-vue

Vue 3 SDK for [Sentori](https://sentori.golia.jp). Installs as a
plugin; the global error handler captures component-tree errors
automatically.

## Install

```sh
bun add @goliapkg/sentori-vue @goliapkg/sentori-javascript
```

## Use

```ts
// main.ts
import { createApp } from 'vue'
import { sentoriVue } from '@goliapkg/sentori-vue'
import App from './App.vue'

createApp(App)
  .use(sentoriVue, {
    token: 'st_pk_…',
    release: 'my-app@1.2.3',
    environment: 'prod',
  })
  .mount('#app')
```

For manual capture:

```ts
import { sentori } from '@goliapkg/sentori-javascript'
sentori.captureException(err)
```

→ Full guide: [sentori.golia.jp/docs/sdk-vue](https://sentori.golia.jp/docs/sdk-vue)
→ Sentry drop-in: [sentori.golia.jp/docs/sentry-compat](https://sentori.golia.jp/docs/sentry-compat)

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) OR
[MIT](../../LICENSE-MIT).
