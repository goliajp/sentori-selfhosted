# Sentori webapp

v0.1 minimal dashboard shell. React 19 + Vite 6 + TS 5.7
+ Tailwind 3.

## Status

`[~]` Phase 5 W1 partial ship 2026-06-21. **UI code
ships, browser-side QA pass pending.** Per CLAUDE.md
铁律 ("UI changes must be browser-validated before
reporting complete"), this is shipped as a code
skeleton — a human must boot the dev server and verify
each page renders / each API call resolves.

## Scope (v0.1 skeleton)

4 pages + 1 layout shell:

| Page | Path | Backend dep |
|---|---|---|
| Login | `/login` | (stub — pending K2 token middleware) |
| Projects | `/projects` | `GET /v1/projects` ✓ |
| Issues | `/projects/:id/issues` | `POST /v1/events/:id` ✓ (ingest tester); list pending |
| Settings | `/settings` | (stub — pending K1 + K12 + K14 UI handlers) |
| Health | `/health` | `GET /healthz` ✓ |

The legacy `web/` tree has 22 React modules — those
ARE NOT ported here. Webapp v0.1 is a fresh minimal
shell that proves end-to-end wiring (browser ↔
self-hosted server). Per-module port lands in v0.2.

## Dev loop

```bash
# Terminal 1: backend
cd self-hosted/docker
docker compose up -d

# Terminal 2: webapp
cd webapp
bun install
bun run dev
# open http://localhost:3000
```

Vite proxies `/v1/*` + `/healthz` to `localhost:8080`
so the dev server works without CORS gymnastics.

## Production build

```bash
bun run build
# emits dist/ — any static HTTP server (Caddy / nginx /
# GitHub Pages) can serve it
```

## QA checklist (human-required)

After `bun run dev`, verify:
- [ ] `/health` renders + auto-refreshes every 10s
- [ ] `/projects` lists projects (or shows empty state)
- [ ] `/projects/:id/issues` test-ingest button posts a
      synthetic event + shows event/issue ids
- [ ] Sidebar nav highlights the active route
- [ ] Login page renders (stub — submission routes to
      /projects)
- [ ] Settings page renders all 4 sections

Failures here are bugs; file in the public mirror.

## Layout

```
src/
  App.tsx              # shell with sidebar + outlet
  main.tsx             # react-router routes
  lib/api.ts           # fetch wrapper for /v1/* + /healthz
  pages/{Login,Projects,Issues,Settings,Health}.tsx
  styles/index.css     # tailwind base + dark-mode root
```

## Not yet wired (v0.1.x ships)

- K2 auth-session HTTP middleware → login flow
- K5 issue listing endpoint → Issues page
- K11 notifier dispatch UI → Settings → Integrations
- K14 alert rule CRUD UI → Settings → Alert rules
- K15 saved view CRUD UI → per-page filter bar
- K16 ACL gate UI → Settings → Members
- K17 billing UI → Settings → Plan (read-only in v0.1
  OSS; Stripe portal jump in saas)

## Eventual ceiling-first targets

- A11y WCAG 2.1 AA
- Storybook 文档化每 component
- Visual regression test (Chromatic OR Playwright snapshot)
- Bundle size budget — 单 component < 10 KB gzip
- Dark / Light 双 mode 测试
- i18n key-based, en / zh-CN / ja 三语

These land progressively as the webapp grows past the
v0.1 skeleton.
