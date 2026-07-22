// Convenience re-export so users can drop a one-liner into their
// instrumentation.ts:
//
//     // instrumentation.ts
//     export { register, onRequestError } from '@goliapkg/sentori-next/instrumentation'
//
// Equivalent to writing the longer form by hand:
//
//     export async function register() {
//       if (process.env.NEXT_RUNTIME === 'nodejs') {
//         const { serverInit } = await import('@goliapkg/sentori-next/server')
//         serverInit()
//       }
//     }
//     export { onRequestError } from '@goliapkg/sentori-next/server'
//
// The dynamic import keeps Next's edge runtime build from pulling in
// Node-only deps when NEXT_RUNTIME === 'edge'.
export async function register() {
    const env = globalThis.process
        ?.env;
    if (env?.NEXT_RUNTIME !== 'nodejs')
        return;
    const { serverInit } = await import('./server.js');
    serverInit();
}
export { onRequestError } from './server.js';
//# sourceMappingURL=instrumentation.js.map