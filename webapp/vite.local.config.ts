// Isolated local diagnosis environment; never reads the repository's .env.local.
import { fileURLToPath } from 'node:url';
import { readFileSync } from 'node:fs';
import { mergeConfig } from 'vite';
import config from './vite.config.ts';
const instance = process.env.SENTORI_DEV_INSTANCE ?? 'local';
const offset = Number(process.env.SENTORI_DEV_PORT_OFFSET ?? 0);
if (!/^[a-z][a-z0-9-]{0,30}$/.test(instance) || !Number.isInteger(offset) || offset < 0 || offset > 20000) throw new Error('Invalid development instance or ports');
// A caller may have invoked Bun before this config; never inherit its VITE_ values.
for (const key of Object.keys(process.env)) if (key.startsWith('VITE_')) delete process.env[key];

// Only this local config includes the generated instance's owner login.
// Select the two login fields explicitly; database credentials stay server-side.
const { email, password } = JSON.parse(readFileSync(new URL(`../.local/sentori/${instance}/settings.json`, import.meta.url), 'utf8'));
if (typeof email !== 'string' || typeof password !== 'string' || !email || !password) throw new Error('Missing local owner login; run bun run dev:setup');

const proxy = Object.fromEntries(
  ['/hooks', '/v1', '/auth', '/admin/api', '/healthz', '/livez', '/readyz'].map(path => [path, { target: `http://127.0.0.1:${18080 + offset}`, changeOrigin: true }]),
);
export default mergeConfig(config, {
  define: { __LOCAL_DEV_LOGIN__: JSON.stringify({ email, password }) },
  envDir: fileURLToPath(new URL(`../.local/sentori/${instance}/vite-env`, import.meta.url)),
  server: { host: '127.0.0.1', port: 13000 + offset, strictPort: true, proxy },
});
