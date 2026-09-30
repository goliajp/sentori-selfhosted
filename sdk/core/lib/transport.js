// Event transport — the only place an SDK talks to the network.
//
// Quiet by default, complete when it matters: events batch on a 5 s
// timer or a 10-deep queue, whichever first; assert pass-counts
// piggyback on whatever batch goes out next (never their own request);
// failures back off and finally persist to an offline queue drained on
// next launch. Nothing here ever throws into the host app.
//
// This lived in `sdk/react-native` and was reachable only from there.
// The web and mini-program SDKs need the same queueing, the same
// backoff, the same refusal handling and the same offline behaviour —
// and three copies of a retry loop diverge, with the copies nobody is
// currently looking at diverging silently. What is actually
// platform-specific is small and is injected: how to make a request,
// where to persist, what to call ourselves.
import { degradePlatform, refusalIsAboutPlatform } from './platform-degrade.js';
import { logger } from './logger.js';
const FLUSH_INTERVAL_MS = 5_000;
const BATCH_SIZE = 10;
const MAX_RETRY = 3;
const MAX_PERSISTED = 1000;
// The native transports have capped the in-memory queue at 500 since
// they were written; this one had no bound at all.
const MAX_QUEUED = 500;
/** Sessions are small and rare; losing one moves a published number. */
const MAX_QUEUED_SESSIONS = 200;
/** Where the offline queue lives. One key, one SDK. */
export const PENDING_STORAGE_KEY = '@sentori/pending';
export function createTransport(host) {
    const setT = host.setTimeout ?? ((fn, ms) => setTimeout(fn, ms));
    const clearT = host.clearTimeout ?? ((h) => clearTimeout(h));
    let queue = [];
    let sessions = [];
    let dropped = 0;
    let assertStats = new Map();
    let flushTimer = null;
    let started = false;
    const pendingAttachments = new Map();
    /** Count a loss. Silence here is what makes a dropped event
     *  indistinguishable from an idle client. */
    const countDropped = (n) => {
        if (n > 0)
            dropped += n;
    };
    const arm = (ms) => {
        flushTimer = setT(() => {
            flushTimer = null;
            void flush();
        }, ms);
    };
    const enqueue = (event) => {
        queue.push(event);
        if (queue.length > MAX_QUEUED) {
            countDropped(queue.length - MAX_QUEUED);
            queue = queue.slice(-MAX_QUEUED);
        }
        if (queue.length >= BATCH_SIZE)
            void flush();
        else if (!flushTimer)
            arm(FLUSH_INTERVAL_MS);
    };
    /**
     * Count an assert outcome. Pass-counts NEVER become events — they
     * aggregate here and ride the next batch envelope (the liveness
     * ledger without a heartbeat flood).
     */
    const countAssert = (name, ok, release) => {
        const key = `${name}${release}`;
        const cur = assertStats.get(key) ?? { name, release, passDelta: 0, failDelta: 0 };
        if (ok)
            cur.passDelta += 1;
        else
            cur.failDelta = (cur.failDelta ?? 0) + 1;
        assertStats.set(key, cur);
        // Stats with no event traffic still ship eventually, on a lazy
        // timer six times the batch interval.
        if (!flushTimer && queue.length === 0)
            arm(FLUSH_INTERVAL_MS * 6);
    };
    /**
     * A finished session, riding the next envelope.
     *
     * Kept apart from the event queue: a session is the denominator of
     * the crash-free rate, and dropping one when the event queue
     * overflows would move the rate in the flattering direction. The cap
     * is its own, and generous — a session is a few dozen bytes and an
     * app produces one per foreground, not one per error.
     */
    const queueSession = (ping) => {
        sessions.push(ping);
        while (sessions.length > MAX_QUEUED_SESSIONS)
            sessions.shift();
    };
    const start = () => {
        started = true;
    };
    const sleep = (ms) => new Promise((r) => setT(() => r(), ms));
    const sendOnce = async (envelope, cfg) => {
        const resp = await host.fetch(`${cfg.ingestUrl}/v1/events:batch`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                Authorization: `Bearer ${cfg.token}`,
                'Sentori-Sdk': host.sdkLabel,
            },
            body: JSON.stringify(envelope),
        });
        if (resp.status === 429) {
            let retryAfterMs = 5000;
            try {
                const j = (await resp.json());
                if (typeof j.retryAfterMs === 'number')
                    retryAfterMs = j.retryAfterMs;
            }
            catch {
                // ignore body parse error
            }
            await sleep(retryAfterMs);
            throw new Error('rate-limited');
        }
        if (resp.status >= 500)
            throw new Error(`server-${resp.status}`);
        // A batch answers 200 with one outcome per event, and a rejected
        // event carries `error` there rather than in the status. Reading
        // only the status is how an event the server refused counted as
        // delivered: retrying it would crashloop — `invalid_payload` is
        // permanent — but not counting it made a refusal look like a quiet
        // minute. It matters most where the two halves drift apart: a
        // self-hosted server that has not been upgraded refuses a platform
        // value its SDK already sends, and every event goes missing with
        // nothing saying so.
        try {
            const body = (await resp.json());
            countDropped((body.outcomes ?? []).filter((o) => o && o.error).length);
            // If the platform is what it refused, this server is older than
            // this SDK. Nothing we resend will change its mind, so the rest
            // of the session goes out under a value every shipped server
            // takes — losing the runtime's name, which is recoverable,
            // rather than every event, which is not.
            if (refusalIsAboutPlatform(body.outcomes))
                degradePlatform((m) => logger.warn(m));
        }
        catch {
            // A 2xx we cannot parse tells us nothing about the items; the
            // events are gone either way and guessing a number would be
            // worse than the gap.
        }
    };
    const sendWithRetry = async (envelope, cfg) => {
        let attempt = 0;
        let delayMs = 1000;
        for (;;) {
            try {
                await sendOnce(envelope, cfg);
                return;
            }
            catch (e) {
                attempt += 1;
                if (attempt >= MAX_RETRY)
                    throw e;
                await sleep(delayMs);
                delayMs *= 2;
            }
        }
    };
    const persist = async (events) => {
        if (events.length === 0)
            return;
        const storage = await host.storage();
        // No storage means this batch ends here. It used to end here
        // silently, so a host without the optional peer dependency lost
        // every failed batch and nothing said so.
        if (!storage)
            return countDropped(events.length);
        try {
            const existing = await storage.getItem(PENDING_STORAGE_KEY);
            const prev = existing ? JSON.parse(existing) : [];
            const all = [...prev, ...events];
            const merged = all.slice(-MAX_PERSISTED);
            countDropped(all.length - merged.length);
            await storage.setItem(PENDING_STORAGE_KEY, JSON.stringify(merged));
        }
        catch {
            countDropped(events.length);
        }
    };
    const flush = async () => {
        if (!started)
            return;
        const cfg = host.config();
        if (!cfg)
            return;
        const events = queue.splice(0, queue.length);
        const stats = [...assertStats.values()];
        assertStats = new Map();
        if (flushTimer) {
            clearT(flushTimer);
            flushTimer = null;
        }
        if (events.length === 0 && stats.length === 0 && sessions.length === 0)
            return;
        // Taken and reset as the envelope is built, which is what the two
        // native transports do: a count riding a batch that never arrives
        // is lost with it, and carrying it forward would report it twice.
        const lost = dropped;
        dropped = 0;
        const outgoingSessions = sessions;
        sessions = [];
        const envelope = { events };
        if (outgoingSessions.length > 0)
            envelope.sessions = outgoingSessions;
        if (lost > 0)
            envelope.droppedEvents = lost;
        if (stats.length > 0)
            envelope.assertStats = stats;
        if (cfg.backendHealthUrl)
            envelope.backendHealthUrl = cfg.backendHealthUrl;
        try {
            await sendWithRetry(envelope, cfg);
            // Only now do the events exist server-side — release their
            // queued attachments. Uploading before this point 404s: the
            // attachment races the 5s event batch and always wins.
            for (const ev of events) {
                if (ev.id)
                    void sendQueuedAttachments(ev.id);
            }
        }
        catch {
            // Events survive offline; assert deltas are cheap enough to
            // lose. Their attachments are memory-only and lost with the
            // process — documented; a replay is context, not the crash
            // report itself.
            await persist(events);
        }
    };
    /** Attach a blob to an event that is still in the batch queue. It
     *  uploads right after the batch containing the event lands, so the
     *  server always already knows the event. */
    const queueAttachment = (eventId, kind, blob, opts = {}) => {
        const list = pendingAttachments.get(eventId) ?? [];
        list.push({ kind, blob, source: opts.source ?? 'js' });
        pendingAttachments.set(eventId, list);
    };
    const sendQueuedAttachments = async (eventId) => {
        const list = pendingAttachments.get(eventId);
        if (!list)
            return;
        pendingAttachments.delete(eventId);
        for (const a of list) {
            await uploadAttachment(eventId, a.kind, a.blob, { source: a.source });
        }
    };
    /**
     * Upload one attachment for an already-enqueued event. Returns null
     * on any non-fatal failure — the event still ships without the
     * attachment so the crash itself is never lost.
     */
    const uploadAttachment = async (eventId, kind, blob, opts = {}) => {
        const cfg = host.config();
        if (!cfg)
            return null;
        const url = `${cfg.ingestUrl}/v1/events/${encodeURIComponent(eventId)}/attachments/${encodeURIComponent(kind)}`;
        // Hand-built multipart. React Native's FormData file part wants a
        // `uri`, and its `data:` URI form throws a bare network error on
        // iOS — this shipped untested and every JS attachment silently
        // died. Text payloads (replay/screens NDJSON) embed directly;
        // base64 payloads embed as base64 with the transfer-encoding
        // header so the server knows to decode.
        const boundary = `----sentori-${eventId}`;
        const isText = typeof blob.text === 'string';
        const content = isText ? (blob.text ?? '') : (blob.base64 ?? '');
        const encodingHeader = isText ? '' : 'Content-Transfer-Encoding: base64\r\n';
        const wireBody = `--${boundary}\r\n` +
            `Content-Disposition: form-data; name="file"; filename="${kind}.bin"\r\n` +
            `Content-Type: ${blob.mediaType}\r\n` +
            encodingHeader +
            `\r\n${content}\r\n` +
            `--${boundary}\r\n` +
            `Content-Disposition: form-data; name="source"\r\n` +
            `\r\n${opts.source ?? 'js'}\r\n` +
            `--${boundary}--\r\n`;
        try {
            const resp = await host.fetch(url, {
                body: wireBody,
                headers: {
                    Authorization: `Bearer ${cfg.token}`,
                    'Content-Type': `multipart/form-data; boundary=${boundary}`,
                    'Sentori-Sdk': host.sdkLabel,
                },
                method: 'POST',
            });
            if (resp.status < 200 || resp.status >= 300) {
                logger.warn(`attachment ${kind} upload http_${resp.status}`);
                return null;
            }
            const body = (await resp.json());
            return body.refId ? { ref: body.refId } : null;
        }
        catch (e) {
            logger.warn(`attachment ${kind} upload failed: ${String(e)}`);
            return null;
        }
    };
    const drainOfflineQueue = async () => {
        const storage = await host.storage();
        if (!storage)
            return;
        try {
            const raw = await storage.getItem(PENDING_STORAGE_KEY);
            if (!raw)
                return;
            await storage.removeItem(PENDING_STORAGE_KEY);
            for (const e of JSON.parse(raw))
                queue.push(e);
            await flush();
        }
        catch {
            // best-effort
        }
    };
    return {
        enqueue,
        countAssert,
        queueSession,
        start,
        flush,
        queueAttachment,
        uploadAttachment,
        drainOfflineQueue,
        reset() {
            queue = [];
            sessions = [];
            dropped = 0;
            assertStats = new Map();
            if (flushTimer)
                clearT(flushTimer);
            flushTimer = null;
            started = false;
            pendingAttachments.clear();
        },
        peekQueue: () => queue,
        peekSessions: () => sessions,
        peekDropped: () => dropped,
        peekAssertStats: () => [...assertStats.values()],
    };
}
//# sourceMappingURL=transport.js.map