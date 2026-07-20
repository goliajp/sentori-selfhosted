import { hashIdentities, logger, TrailBuffer, safeFn, sealTrail, shouldSample, } from '@goliapkg/sentori-core';
import { addBreadcrumb, getBreadcrumbs } from './breadcrumbs.js';
import { getConfig, isInitialized } from './config.js';
import { markSessionErrored } from './session-tracker.js';
import { parseStack } from './stack.js';
import { send, uploadAttachment } from './transport.js';
import { uuidV7 } from './uuid.js';
let _user = null;
/** v2.0 — global scope tags that get merged onto every captured
 *  event (captureException + captureMessage). Set via `setTag` /
 *  `setTags`; reset by passing `null` / `{}`. */
let _scopeTags = {};
/**
 * v2.0 — set a single scope tag that's merged onto every subsequent
 * capture. Per-call `extras.tags` / `opts.tags` win over scope tags.
 *
 *     sentori.setTag('rollout', 'dark-mode-v2')
 *     sentori.captureException(err)  // event.tags carries rollout
 */
export function setTag(key, value) {
    _scopeTags[key] = String(value);
}
/**
 * v2.0 — bulk variant of setTag. Existing tags are merged with
 * the input record; pass `{}` to clear (`Object.assign` style).
 */
export function setTags(record) {
    for (const [k, v] of Object.entries(record))
        _scopeTags[k] = String(v);
}
/** Internal — used by captureError / captureMessage to merge the
 *  global scope tags onto per-event tags. */
function mergeScopeTags(perCall) {
    if (Object.keys(_scopeTags).length === 0)
        return perCall;
    return { ..._scopeTags, ...(perCall ?? {}) };
}
export const __resetScopeForTests = () => {
    _user = null;
    _scopeTags = {};
};
const _trail = new TrailBuffer(30);
/**
 * Phase 46 — record a step into the session-trail buffer. The buffer
 * is a fixed-size FIFO; pushing past capacity drops the oldest.
 * Uploaded as a `sessionTrail` attachment on the next
 * `captureException` only when `init({ capture: { sessionTrail:
 * true } })` is on.
 */
export function captureStep(label, opts) {
    _trail.push({
        ts: Date.now(),
        label,
        ...(opts ?? {}),
    });
}
export function __resetTrailForTests() {
    _trail.clear();
}
export function setUser(input) {
    if (input == null) {
        _user = null;
        return;
    }
    const { linkBy: rawLinkBy, ...stable } = input;
    _user = { ...stable };
    if (rawLinkBy && Object.keys(rawLinkBy).length > 0) {
        void hashIdentities(rawLinkBy)
            .then((linkHashes) => {
            if (_user && _user.id === stable.id) {
                _user = { ..._user, linkHashes };
            }
        })
            .catch((e) => {
            logger.warn('identity', 'linkBy hash failed; identities dropped:', e);
        });
    }
}
export function getUser() {
    return _user;
}
export function captureError(error, extras) {
    if (!isInitialized())
        return;
    const cfg = getConfig();
    // Phase 44 sub-B: client-side sampling. Drop sampled-out events
    // before any work (breadcrumbs / transport).
    if (!shouldSample(cfg.sampling?.errors ?? null)) {
        addBreadcrumb({ data: { kind: 'error', reason: 'sampled-out' }, type: 'custom' });
        return;
    }
    const event = {
        app: { version: parseRelease(cfg.release).version },
        breadcrumbs: getBreadcrumbs(),
        device: detectDevice(),
        environment: cfg.environment,
        error: errorToObject(error),
        fingerprint: extras?.fingerprint,
        id: uuidV7(),
        kind: 'error',
        platform: 'javascript',
        release: cfg.release,
        tags: mergeScopeTags(extras?.tags),
        timestamp: new Date().toISOString(),
        user: extras?.user ?? _user,
    };
    // Phase 26 sub-B: a captured error promotes the current session to
    // `errored` so the next end-of-session ping reports unhealthy.
    markSessionErrored();
    const transportCfg = { ingestUrl: cfg.ingestUrl, token: cfg.token };
    const pipeline = async () => {
        // Phase 46 — seal + upload the session trail (best-effort) before
        // shipping the event so `event.attachments[]` carries the ref the
        // dashboard renders. Trail is cleared after every captureException
        // regardless of upload outcome, to keep "trail per crash" clean.
        if (cfg.capture?.sessionTrail && _trail.size() > 0) {
            const payload = sealTrail(_trail);
            _trail.clear();
            const meta = await uploadAttachment(transportCfg, event.id, 'sessionTrail', {
                body: JSON.stringify(payload),
                mediaType: 'application/json',
            });
            if (meta) {
                if (!event.attachments)
                    event.attachments = [];
                event.attachments.push(meta);
            }
        }
        // v2.3 — host beforeSend hook. Same semantics as RN: sync,
        // null drops, throw / non-event falls back unmodified.
        const finalEvent = applyBeforeSend(event, cfg.beforeSend);
        if (finalEvent === null)
            return;
        await send(transportCfg, finalEvent);
    };
    void pipeline();
}
export const captureException = captureError;
const DEFAULT_MESSAGE_LEVEL = 'info';
/**
 * Manually report an issue without an Error instance.
 *
 * Routes to the dashboard Issues module — distinct from `track`
 * (analytics) and `recordMetric` (numeric). Use for "operator
 * should look at this" signals: a fallback that fired, an unexpected
 * state, a feature flag rollout that crossed a threshold.
 *
 *     sentori.captureMessage('Payment provider returned 500, used fallback')
 *     sentori.captureMessage('Detected impossible state in session reducer', {
 *       level: 'error',
 *       tags: { reducer: 'session' },
 *     })
 *
 * Wrapped in `safeFn` per the NEVER rule — any internal failure is
 * swallowed and (optionally) self-reported; the host app never sees
 * a thrown error.
 */
export const captureMessage = safeFn('captureMessage', (message, opts = {}) => {
    if (!isInitialized())
        return;
    if (typeof message !== 'string' || message.length === 0)
        return;
    const cfg = getConfig();
    if (!shouldSample(cfg.sampling?.messages ?? 1.0)) {
        addBreadcrumb({ data: { kind: 'message', reason: 'sampled-out' }, type: 'custom' });
        return;
    }
    const event = {
        app: { version: parseRelease(cfg.release).version },
        breadcrumbs: opts.breadcrumbs ?? getBreadcrumbs(),
        device: detectDevice(),
        environment: cfg.environment,
        id: uuidV7(),
        kind: 'message',
        level: opts.level ?? DEFAULT_MESSAGE_LEVEL,
        message,
        platform: 'javascript',
        release: cfg.release,
        tags: mergeScopeTags(opts.tags),
        timestamp: new Date().toISOString(),
        user: opts.user ?? _user,
    };
    const transportCfg = { ingestUrl: cfg.ingestUrl, token: cfg.token };
    const finalEvent = applyBeforeSend(event, cfg.beforeSend);
    if (finalEvent === null)
        return;
    void send(transportCfg, finalEvent);
});
/**
 * v2.3 — invoke the host's `beforeSend` hook (if any) under the
 * NEVER rule. Returns the (possibly mutated) event, or `null` to
 * drop. Throw / non-event return falls back to the unmodified
 * event with a one-shot warn.
 */
let _beforeSendThrewWarned = false;
function applyBeforeSend(event, hook) {
    if (!hook)
        return event;
    try {
        const result = hook(event);
        if (result === null)
            return null;
        if (typeof result !== 'object' || !result || typeof result.id !== 'string') {
            if (!_beforeSendThrewWarned) {
                _beforeSendThrewWarned = true;
                logger.warn('capture', 'beforeSend returned non-event shape; falling back to unmodified event');
            }
            return event;
        }
        return result;
    }
    catch (e) {
        if (!_beforeSendThrewWarned) {
            _beforeSendThrewWarned = true;
            logger.warn('capture', 'beforeSend threw; falling back to unmodified event', e);
        }
        return event;
    }
}
function errorToObject(error) {
    const causeRaw = error.cause;
    let cause = null;
    if (causeRaw instanceof Error)
        cause = errorToObject(causeRaw);
    return {
        cause,
        message: error.message,
        stack: parseStack(error.stack),
        type: error.name || 'Error',
    };
}
function parseRelease(release) {
    const m = /^(?:[^@]+@)?([^+]+)(?:\+(.+))?$/.exec(release);
    return { build: m?.[2], version: m?.[1] ?? '0.0.0' };
}
function detectDevice() {
    // The server's device.os is a strict enum: `ios | android | web | other`
    // (see docs/protocol.md). Browser → web; Node + everything else → other.
    // The pre-Phase-21 build sent free-form values like "macos" / "windows"
    // which the server quietly rejected with `validationFailed`. Detail
    // about the underlying OS family rides along in `model` instead.
    const w = globalThis.navigator;
    if (w?.userAgent) {
        const networkType = detectNetworkType(w);
        return {
            locale: w.language,
            model: detectBrowserOs(w.userAgent),
            ...(networkType ? { networkType } : {}),
            os: 'web',
            osVersion: '0',
        };
    }
    // Node
    const p = globalThis.process;
    if (p?.platform) {
        return {
            model: p.platform,
            os: 'other',
            osVersion: p.version?.replace(/^v/, '') ?? '0',
        };
    }
    return { os: 'other', osVersion: '0' };
}
/** v0.8.0-c — Network Information API. Implemented in Chrome/Edge for
 *  years, Safari Tech Preview, Firefox flagged-only. We use the
 *  `effectiveType` field — it normalises wifi-vs-mobile reality
 *  ("4g" doesn't always mean cellular, can be a fast wifi link).
 *  `navigator.onLine === false` short-circuits to "offline" before
 *  asking the connection API, which on some browsers returns a stale
 *  type during early offline events. */
function detectNetworkType(nav) {
    if (nav.onLine === false)
        return 'offline';
    const eff = nav.connection?.effectiveType;
    if (eff === '4g' || eff === '3g' || eff === '2g' || eff === 'slow-2g')
        return eff;
    const type = nav.connection?.type;
    if (type === 'wifi')
        return 'wifi';
    return undefined;
}
function detectBrowserOs(ua) {
    if (ua.includes('Mac OS X') || ua.includes('Macintosh'))
        return 'macos';
    if (ua.includes('Windows'))
        return 'windows';
    if (ua.includes('Linux'))
        return 'linux';
    if (ua.includes('Android'))
        return 'android';
    if (ua.includes('iPhone') || ua.includes('iPad'))
        return 'ios';
    return 'web';
}
//# sourceMappingURL=capture.js.map