// Browser-side Next entry point. Used from a Next "use client" file
// in app/layout.tsx — pairs with serverInit() in instrumentation.ts.

import { initSentori } from '@goliapkg/sentori-javascript'

import { resolveConfig, type SentoriNextConfig } from './config.js'

let _initialised = false

/**
 * Initialise the JS SDK once on the browser. Idempotent across
 * Next.js's React Refresh / fast-reload / route transitions.
 *
 *     // app/layout.tsx
 *     'use client'
 *     import { clientInit } from '@goliapkg/sentori-next/client'
 *     clientInit()
 *     export default function RootLayout({ children }) { ... }
 *
 * With NEXT_PUBLIC_SENTORI_TOKEN, NEXT_PUBLIC_SENTORI_RELEASE, and
 * NEXT_PUBLIC_SENTORI_ENVIRONMENT set, no arguments are needed.
 */
export function clientInit(cfg: SentoriNextConfig = {}): void {
  if (_initialised) return
  try {
    initSentori(resolveConfig('client', cfg))
    _initialised = true
  } catch (e) {
    // Warn — never error — so we don't add red noise to the host app's
    // console. Sentori must be "free upside": init failure must be
    // silent-ish, never a crash signal.
    // eslint-disable-next-line no-console
    console.warn('[sentori-next] client init failed', e)
  }
}

export { SentoriProvider, SentoriErrorBoundary, useSentori, useCaptureError } from '@goliapkg/sentori-react'
