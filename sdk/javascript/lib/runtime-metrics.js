// v2.1 W2 part 5 — JS (web) runtime metrics flusher.
//
// Web parallel of the RN flusher. Drains core's runtime-metrics
// ring every 30s and POSTs to /v1/runtime-metrics:batch. Hosts
// (or framework adapters) call emitMetric() from their own
// instrumentation; core's ring is shared across all SDKs in the
// bundle so a single drain handles every emitter.
//
// What's intentionally NOT here (yet — ships in 2.1.2 web-vitals
// patch):
//   - LCP / CLS / INP via the `web-vitals` library — adds a peer
//     dep + browser-only init wiring
//   - performance.memory polling — works in Chromium only and
//     needs a tiny module; same shape as RN heap.ts. Hosts who
//     want it today can call emitMetric directly from a
//     setInterval.
//
// The transport itself is shipped now so any host (framework
// adapter, plain JS, opt-in custom polling) can push runtime
// metrics via emitMetric and have them land on the server
// pipeline.
import { drainRuntimeMetricsForFlush, rebufferRuntimeMetrics, reportInternal, } from '@goliapkg/sentori-core';
import { getConfig } from './config.js';
const FLUSH_INTERVAL_MS = 30_000;
const SDK_HEADER = 'sentori-javascript/runtime-metrics/1.0';
let _timer = null;
/** POST a batched set of runtime-metric points to
 *  /v1/runtime-metrics:batch. Returns true on 2xx so the caller
 *  can leave the batch drained; returns false on anything else
 *  so the caller rebuffer-and-retries. */
async function sendRuntimeMetricsBatch(ingestUrl, token, metrics) {
    if (metrics.length === 0)
        return true;
    const url = `${ingestUrl.replace(/\/+$/, '')}/v1/runtime-metrics:batch`;
    try {
        const resp = await fetch(url, {
            body: JSON.stringify({ metrics }),
            headers: {
                Authorization: `Bearer ${token}`,
                'Content-Type': 'application/json',
                'Sentori-Sdk': SDK_HEADER,
            },
            keepalive: true,
            method: 'POST',
        });
        return resp.ok;
    }
    catch {
        return false;
    }
}
/** Atomic drain + POST + rebuffer-on-failure. Per the NEVER rule:
 *  never throws, never rejects. */
export async function flushRuntimeMetrics() {
    const cfg = getConfig();
    if (!cfg)
        return;
    const batch = drainRuntimeMetricsForFlush();
    if (batch.length === 0)
        return;
    const ok = await sendRuntimeMetricsBatch(cfg.ingestUrl, cfg.token, batch);
    if (!ok) {
        rebufferRuntimeMetrics(batch);
        reportInternal('runtime-metrics.flush', new Error('runtime-metrics POST failed'));
    }
}
/** Idempotent start of the 30 s flush timer. Called from init()
 *  when `runtimeMetrics: true` is set. */
export function startRuntimeMetricsTimer() {
    if (_timer !== null)
        return;
    _timer = setInterval(() => {
        void flushRuntimeMetrics();
    }, FLUSH_INTERVAL_MS);
    _timer.unref?.();
}
/** Stop the periodic flush. Idempotent. Used by tests + by hosts
 *  that want to opt out mid-session. */
export function stopRuntimeMetricsTimer() {
    if (_timer !== null) {
        clearInterval(_timer);
        _timer = null;
    }
}
//# sourceMappingURL=runtime-metrics.js.map