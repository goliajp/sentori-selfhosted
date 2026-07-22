import { jsx as _jsx } from "react/jsx-runtime";
import { Suspense } from 'react';
import { SentoriErrorBoundary } from './SentoriErrorBoundary.js';
/**
 * `<Suspense>` + `<SentoriErrorBoundary>` composed together. Any
 * error thrown during render, whether it's a synchronous throw or a
 * rejected promise surfaced through Suspense, is caught by the
 * inner boundary and forwarded to `captureError`.
 *
 * Use when you want a one-liner around a data-fetching subtree —
 * the loading state and the error state share the same fallback by
 * default; pass `errorFallback` if they need to differ.
 *
 *     <SentoriSuspense fallback={<Skeleton />} errorFallback={<ErrorCard />}>
 *       <UserProfile />
 *     </SentoriSuspense>
 */
export function SentoriSuspense({ children, errorFallback, fallback, }) {
    return (_jsx(SentoriErrorBoundary, { fallback: errorFallback ?? fallback, children: _jsx(Suspense, { fallback: fallback, children: children }) }));
}
//# sourceMappingURL=SentoriSuspense.js.map