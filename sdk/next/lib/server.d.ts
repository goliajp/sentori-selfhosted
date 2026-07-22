import { type SentoriNextConfig } from './config.js';
/**
 * Initialise the JS SDK on the Node server. Called from
 * instrumentation.ts:
 *
 *     // instrumentation.ts
 *     export async function register() {
 *       if (process.env.NEXT_RUNTIME === 'nodejs') {
 *         const { serverInit } = await import('@goliapkg/sentori-next/server')
 *         serverInit()
 *       }
 *     }
 *
 * Edge runtime is intentionally not initialised here — Next's edge
 * environment lacks `process` and the Node-only Node hooks would
 * throw. Edge errors flow through `onRequestError` below.
 */
export declare function serverInit(cfg?: SentoriNextConfig): void;
/**
 * Next's instrumentation.ts:onRequestError signature, wired to the
 * SDK's captureError. Tags the event with the route + HTTP method
 * + the runtime that caught it ("nodejs" | "edge").
 *
 *     // instrumentation.ts
 *     export { onRequestError } from '@goliapkg/sentori-next/server'
 *
 * Or compose:
 *
 *     export async function onRequestError(err, request, context) {
 *       const { onRequestError } = await import('@goliapkg/sentori-next/server')
 *       await onRequestError(err, request, context)
 *       // your own logging
 *     }
 */
export type RequestErrorContext = {
    routePath?: string;
    routeType?: 'app' | 'pages' | 'route';
    routerKind?: 'App Router' | 'Pages Router';
    runtime?: 'edge' | 'nodejs';
};
export type RequestErrorRequest = {
    headers?: Record<string, string | string[] | undefined>;
    method?: string;
    path?: string;
    url?: string;
};
export declare function onRequestError(err: Error | unknown, request: RequestErrorRequest, context?: RequestErrorContext): Promise<void>;
//# sourceMappingURL=server.d.ts.map