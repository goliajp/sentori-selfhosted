import { type ReactNode } from 'react';
import type { SentoriContextValue, SentoriReactConfig } from './types.js';
/**
 * Initialises the JS SDK once on mount and exposes capture / breadcrumb
 * helpers via context. Safe to mount multiple times in dev (StrictMode
 * double-mount): the JS SDK's own idempotency guards take care of it
 * but we also dedupe here via a ref.
 *
 * Drop this near the root of the React tree (above any
 * `<SentoriErrorBoundary>`):
 *
 *     <SentoriProvider config={{ token, release, ingestUrl, environment }}>
 *       <App />
 *     </SentoriProvider>
 */
export declare function SentoriProvider({ children, config, }: {
    children: ReactNode;
    config: SentoriReactConfig;
}): import("react/jsx-runtime").JSX.Element;
export declare function useSentoriCtx(): SentoriContextValue;
//# sourceMappingURL=SentoriProvider.d.ts.map