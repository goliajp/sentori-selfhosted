import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// Sentori webapp Vite config.
// - Default dev server proxies /v1 + /healthz to the local
//   self-hosted server on :8080 so the dashboard works in
//   `bun run dev` without CORS gymnastics.
// - Production build emits static assets that any HTTP
//   server (Caddy, nginx, GitHub Pages) can serve.
export default defineConfig({
  plugins: [tailwindcss(), react()],
  server: {
    port: 3000,
    // Every prefix the server owns. `/auth` and `/admin/api` were
    // missing, so against the dev server login returned the SPA shell
    // instead of a session and the whole admin surface was
    // unreachable — including the OAuth redirects, which the browser
    // has to follow to the server rather than to Vite.
    proxy: {
      '/v1': { target: 'http://localhost:8080', changeOrigin: true },
      '/auth': { target: 'http://localhost:8080', changeOrigin: true },
      '/admin/api': { target: 'http://localhost:8080', changeOrigin: true },
      '/healthz': { target: 'http://localhost:8080', changeOrigin: true },
      '/livez': { target: 'http://localhost:8080', changeOrigin: true },
      '/readyz': { target: 'http://localhost:8080', changeOrigin: true },
    },
  },
  // `bun run sweep` points at the built bundle rather than the dev
  // server: a sweep against HMR catches whatever half-saved state the
  // editor is in, which twice reported an error that was already fixed.
  preview: {
    port: 5599,
    proxy: {
      '/v1': { target: 'http://localhost:8080', changeOrigin: true },
      '/auth': { target: 'http://localhost:8080', changeOrigin: true },
      '/admin/api': { target: 'http://localhost:8080', changeOrigin: true },
      '/healthz': { target: 'http://localhost:8080', changeOrigin: true },
    },
  },
  build: {
    target: 'es2022',
    sourcemap: true,
  },
});
