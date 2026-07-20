import { setLogLevel } from '@goliapkg/sentori-core';
import { setConfig } from './config.js';
import { installBrowserHooks } from './hooks/browser.js';
import { installFetchInstrumentation } from './hooks/fetch.js';
import { installNodeHooks } from './hooks/node.js';
import { installXhrInstrumentation } from './hooks/xhr.js';
import { startRuntimeMetricsTimer } from './runtime-metrics.js';
import { startSession } from './session-tracker.js';
import { flushSpans, startSpanFlush } from './transport.js';
const SDK_VERSION = '2.3.0';
/**
 * Configure the SDK and (by default) wire global error handlers.
 *
 * Browser: window 'error' + 'unhandledrejection' → captureError.
 * Node: process 'uncaughtException' + 'unhandledRejection' → captureError.
 *
 * Pass `enableGlobalHooks: false` if you want to drive captures
 * manually (e.g. tests, or a host that owns its own crash plumbing).
 *
 * Phase 26 sub-B: also opens a session and binds platform lifecycle
 * (pagehide on browser, beforeExit on Node) so we ship a session ping
 * on close. `enableGlobalHooks: false` disables both error hooks and
 * session lifecycle so test harnesses can drive everything manually.
 */
export function initSentori(options) {
    // v2.3 — set log level FIRST so any startup-time logger calls
    // are gated correctly. Default 'warn' from logger.ts; an explicit
    // host setting overrides.
    setLogLevel(options.logLevel);
    setConfig(options);
    if (options.enableGlobalHooks === false) {
        fireOnReady(options);
        return;
    }
    // Browser comes first because both globals can exist in some
    // bundlers' shims; we want browser semantics on the web.
    if (!installBrowserHooks())
        installNodeHooks();
    // Phase 35 sub-B + follow-up: instrument both transports so every
    // outbound request emits an http.client span + propagates the W3C
    // traceparent header. fetch covers `fetch()` callers; xhr covers
    // axios (default `xhr` adapter) and any older XHR-based client.
    installFetchInstrumentation();
    installXhrInstrumentation();
    startSession();
    // Drain finished spans to /v1/spans:batch on a timer, plus once more
    // on page-hide so the last batch isn't lost when the tab closes.
    startSpanFlush();
    if (typeof addEventListener === 'function') {
        addEventListener('pagehide', () => {
            void flushSpans();
        });
    }
    // v2.1 W2 — opt-in runtime metrics flusher. Off by default in
    // JS since the auto-instrument modules (FPS / heap / network
    // bytes) are RN-only in 2.1.0; web hosts that want to push
    // metrics today can flip this on and call `emitMetric()`
    // directly.
    if (options.capture?.runtimeMetrics === true) {
        startRuntimeMetricsTimer();
    }
    fireOnReady(options);
}
function fireOnReady(options) {
    if (!options.onReady)
        return;
    // JS SDK has no native module — `native` is omitted; the shared
    // `ReadyInfo` type marks it optional for exactly this case.
    // `coldStartMs` is also RN-only.
    const info = { sdkVersion: SDK_VERSION };
    try {
        options.onReady(info);
    }
    catch {
        // Host's onReady threw. NEVER rule — swallow.
    }
}
//# sourceMappingURL=init.js.map