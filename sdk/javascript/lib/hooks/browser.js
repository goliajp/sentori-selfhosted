import { coerceError } from '@goliapkg/sentori-core';
import { captureError } from '../capture.js';
import { endSession } from '../session-tracker.js';
let installed = false;
/**
 * Wire window.onerror + unhandledrejection so uncaught browser errors
 * land as Sentori events automatically. Idempotent — safe to call
 * twice; the second call no-ops.
 */
export function installBrowserHooks() {
    if (installed)
        return true;
    const w = globalThis;
    if (typeof w.addEventListener !== 'function')
        return false;
    // `coerceError` keeps the actual thrown value visible — plain objects
    // come through as JSON instead of `[object Object]`, primitives as
    // their printed value, `{name, message}`-shaped throws preserve both
    // fields. See @goliapkg/sentori-core/coerce-error.
    const onError = (e) => {
        const err = e.error;
        if (err !== undefined) {
            captureError(coerceError(err));
        }
        else if (typeof e.message === 'string') {
            captureError(new Error(e.message));
        }
    };
    const onRejection = (e) => {
        captureError(coerceError(e.reason));
    };
    w.addEventListener('error', onError);
    w.addEventListener('unhandledrejection', onRejection);
    // Phase 26 sub-B: pagehide is the right unload event in modern
    // browsers (fires on bfcache → background, full unload, and tab
    // close). beforeunload is unreliable on mobile Safari.
    w.addEventListener('pagehide', () => endSession());
    installed = true;
    return true;
}
/** Test helper — resets the idempotency latch. */
export function _resetBrowserHooksForTesting() {
    installed = false;
}
//# sourceMappingURL=browser.js.map