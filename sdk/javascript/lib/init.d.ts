import type { InitOptions } from './types.js';
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
export declare function initSentori(options: InitOptions): void;
//# sourceMappingURL=init.d.ts.map