import { type SentoriNextConfig } from './config.js';
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
export declare function clientInit(cfg?: SentoriNextConfig): void;
export { SentoriProvider, SentoriErrorBoundary, useSentori, useCaptureError } from '@goliapkg/sentori-react';
//# sourceMappingURL=client.d.ts.map