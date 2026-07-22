// v2.8 — server-side Push helper for Next.js apps.
//
// Use from API routes, Server Actions, or any Node / Edge runtime
// piece that needs to send a push. The helper wraps `/v1/push/send`
// + `/v1/push/receipts/{id}` with the same wire shape the SDK
// matrix shares (see `@goliapkg/sentori-core`'s `PushMessage` type).
//
// Edge-safe: pure `fetch`, no Node-only imports. Works under
// `runtime: 'edge'` for App Router + middleware contexts.
//
// Example (App Router server action):
//   'use server'
//   import { sentoriPush } from '@goliapkg/sentori-next/push'
//
//   const push = sentoriPush({
//     ingestUrl: process.env.SENTORI_INGEST_URL!,
//     token: process.env.SENTORI_ADMIN_TOKEN!,
//   })
//
//   export async function notifyComment(iptHandle: string, comment: string) {
//     await push.send({
//       to: iptHandle,
//       title: 'New comment',
//       body: comment.slice(0, 80),
//       data: { kind: 'comment' },
//     })
//   }
const MAX_CONCURRENT_BATCH = 8;
export function sentoriPush(cfg) {
    const fetchImpl = cfg.fetch ?? globalThis.fetch;
    if (!fetchImpl) {
        throw new Error('sentoriPush: no fetch implementation available');
    }
    const base = cfg.ingestUrl.replace(/\/+$/, '');
    async function send(msg) {
        const res = await fetchImpl(`${base}/v1/push/send`, {
            method: 'POST',
            headers: {
                authorization: `Bearer ${cfg.token}`,
                'content-type': 'application/json',
            },
            body: JSON.stringify(msg),
        });
        if (!res.ok) {
            const detail = await res.text().catch(() => '');
            throw new Error(`/v1/push/send HTTP ${res.status}: ${detail.slice(0, 200)}`);
        }
        const body = (await res.json());
        if (!body.tickets || body.tickets.length === 0) {
            throw new Error('server returned no tickets');
        }
        return body.tickets[0];
    }
    async function sendBatch(msgs) {
        // Pool of workers — each picks the next message off `queue`.
        const queue = msgs.slice();
        const results = new Array(msgs.length);
        let nextSlot = 0;
        const workers = [];
        const worker = async () => {
            while (true) {
                const idx = nextSlot++;
                const msg = queue[idx];
                if (!msg)
                    return;
                results[idx] = await send(msg);
            }
        };
        for (let i = 0; i < Math.min(MAX_CONCURRENT_BATCH, msgs.length); i++) {
            workers.push(worker());
        }
        await Promise.all(workers);
        return results;
    }
    async function getReceipt(sendId) {
        const res = await fetchImpl(`${base}/v1/push/receipts/${encodeURIComponent(sendId)}`, {
            headers: { authorization: `Bearer ${cfg.token}` },
        });
        if (!res.ok) {
            const detail = await res.text().catch(() => '');
            throw new Error(`/v1/push/receipts HTTP ${res.status}: ${detail.slice(0, 200)}`);
        }
        return (await res.json());
    }
    function isSentoriPushToken(value) {
        return typeof value === 'string' && /^ipt_[0-9a-fA-F]+$/.test(value);
    }
    return { send, sendBatch, getReceipt, isSentoriPushToken };
}
//# sourceMappingURL=push.js.map