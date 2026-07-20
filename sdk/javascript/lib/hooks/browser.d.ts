/**
 * Wire window.onerror + unhandledrejection so uncaught browser errors
 * land as Sentori events automatically. Idempotent — safe to call
 * twice; the second call no-ops.
 */
export declare function installBrowserHooks(): boolean;
/** Test helper — resets the idempotency latch. */
export declare function _resetBrowserHooksForTesting(): void;
//# sourceMappingURL=browser.d.ts.map