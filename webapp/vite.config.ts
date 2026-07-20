import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Sentori webapp Vite config.
// - Default dev server proxies /v1 + /healthz to the local
//   self-hosted server on :8080 so the dashboard works in
//   `bun run dev` without CORS gymnastics.
// - Production build emits static assets that any HTTP
//   server (Caddy, nginx, GitHub Pages) can serve.
export default defineConfig({
  plugins: [react()],
  server: {
    port: 3000,
    proxy: {
      '/v1': { target: 'http://localhost:8080', changeOrigin: true },
      '/healthz': { target: 'http://localhost:8080', changeOrigin: true },
    },
  },
  build: {
    target: 'es2022',
    sourcemap: true,
  },
});
