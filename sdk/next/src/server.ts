// Server-side Next entry point. Used from instrumentation.ts'
// register() function. The JS SDK's Node hooks (uncaughtException +
// unhandledRejection) are wired here; route-handler errors are
// captured via the onRequestError export below.

import { coerceError } from '@goliapkg/sentori-core'
import { captureError, initSentori } from '@goliapkg/sentori-javascript'

import { resolveConfig, type SentoriNextConfig } from './config.js'

let _initialised = false

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
export function serverInit(cfg: SentoriNextConfig = {}): void {
  if (_initialised) return
  try {
    initSentori(resolveConfig('server', cfg))
    _initialised = true
  } catch (e) {
    // Warn — never error — so we don't add red noise to the host app's
    // logs. Sentori must be "free upside": init failure must be
    // silent-ish, never a crash signal.
    // eslint-disable-next-line no-console
    console.warn('[sentori-next] server init failed', e)
  }
}

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
  routePath?: string
  routeType?: 'app' | 'pages' | 'route'
  routerKind?: 'App Router' | 'Pages Router'
  // Next 15+ adds runtime here; older versions leave it undefined.
  runtime?: 'edge' | 'nodejs'
}

export type RequestErrorRequest = {
  headers?: Record<string, string | string[] | undefined>
  method?: string
  path?: string
  url?: string
}

export async function onRequestError(
  err: Error | unknown,
  request: RequestErrorRequest,
  context?: RequestErrorContext,
): Promise<void> {
  // `coerceError` JSON-stringifies plain-object throws so the dashboard
  // shows the real payload instead of `[object Object]`.
  const error = coerceError(err)
  captureError(error, {
    tags: {
      'next.method': request?.method ?? '',
      'next.route': context?.routePath ?? request?.path ?? request?.url ?? '',
      'next.routeType': context?.routeType ?? '',
      'next.runtime': context?.runtime ?? 'unknown',
      source: 'next.requestError',
    },
  })
}
