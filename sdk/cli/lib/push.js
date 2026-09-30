// v2.12 — `sentori-cli push *` commands.
//
// Wraps the v2.7 admin REST + v2.7 ingest endpoints so operators can
// drive credential CRUD, ad-hoc sends, and receipt lookups from the
// terminal without touching curl.
//
// `push send`     POST /v1/push/send         (ingest Bearer)
// `push receipt`  GET  /v1/push/receipts/:id (ingest Bearer)
// `push creds list`    GET    /admin/api/projects/:id/push/credentials   (admin Bearer)
// `push creds set`     PUT    /admin/api/projects/:id/push/credentials   (admin Bearer)
// `push creds delete`  DELETE /admin/api/projects/:id/push/credentials/:provider
import { readFileSync } from 'node:fs';
const VALID_PROVIDERS = new Set(['apns', 'fcm', 'webpush', 'hcm', 'mipush']);
function joinUrl(base, path) {
    return `${base.replace(/\/+$/, '')}${path}`;
}
async function bearerFetch(url, token, init) {
    const resp = await fetch(url, {
        ...init,
        headers: {
            Authorization: `Bearer ${token}`,
            'Content-Type': 'application/json',
            ...(init?.headers ?? {}),
        },
    });
    if (!resp.ok) {
        const detail = await resp.text().catch(() => '');
        throw new Error(`${resp.status} ${resp.statusText}${detail ? ` — ${detail.slice(0, 300)}` : ''}`);
    }
    const txt = await resp.text();
    return (txt ? JSON.parse(txt) : null);
}
/** Parse a CLI flag value that may be `@file.json` (read from disk
 *  and parse) or a literal JSON string. */
export function parseJsonArg(raw, kind) {
    if (raw.startsWith('@')) {
        const path = raw.slice(1);
        const body = readFileSync(path, 'utf-8');
        try {
            return JSON.parse(body);
        }
        catch (e) {
            throw new Error(`${kind} file ${path} is not valid JSON: ${e.message}`);
        }
    }
    try {
        return JSON.parse(raw);
    }
    catch (e) {
        throw new Error(`${kind} arg is not valid JSON: ${e.message}`);
    }
}
// ── credential CRUD ───────────────────────────────────────────────
export async function pushCredsList(cfg) {
    return bearerFetch(joinUrl(cfg.apiUrl, `/admin/api/projects/${cfg.projectId}/push/credentials`), cfg.token);
}
export async function pushCredsSet(cfg, provider, config, secret) {
    if (!VALID_PROVIDERS.has(provider)) {
        throw new Error(`invalid provider '${provider}'; expected one of ${[...VALID_PROVIDERS].join('/')}`);
    }
    return bearerFetch(joinUrl(cfg.apiUrl, `/admin/api/projects/${cfg.projectId}/push/credentials`), cfg.token, {
        body: JSON.stringify({ provider, config, secret }),
        method: 'PUT',
    });
}
export async function pushCredsDelete(cfg, provider) {
    await bearerFetch(joinUrl(cfg.apiUrl, `/admin/api/projects/${cfg.projectId}/push/credentials/${provider}`), cfg.token, { method: 'DELETE' });
}
export async function pushSend(cfg, opts) {
    const payload = {
        to: opts.to,
        title: opts.title,
        body: opts.body,
        data: opts.data,
        idempotencyKey: opts.idempotencyKey,
    };
    if (opts.priority || opts.ttl != null) {
        payload.options = { priority: opts.priority, ttl: opts.ttl };
    }
    const resp = await bearerFetch(joinUrl(cfg.apiUrl, '/v1/push/send'), cfg.token, {
        body: JSON.stringify(payload),
        method: 'POST',
    });
    if (!resp.tickets?.length) {
        throw new Error('server returned no tickets');
    }
    return resp.tickets[0];
}
export async function pushReceipt(cfg, sendId) {
    return bearerFetch(joinUrl(cfg.apiUrl, `/v1/push/receipts/${encodeURIComponent(sendId)}`), cfg.token);
}
//# sourceMappingURL=push.js.map