import { type ReactNode } from 'react';
type FallbackRender = (props: {
    error: Error;
    reset: () => void;
}) => ReactNode;
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
export declare function SentoriSuspense({ children, errorFallback, fallback, }: {
    children: ReactNode;
    /** Optional separate fallback for caught errors. Falls back to
     *  `fallback` if not provided. */
    errorFallback?: FallbackRender | ReactNode;
    /** Loading state shown by the inner `<Suspense>`. */
    fallback: ReactNode;
}): import("react/jsx-runtime").JSX.Element;
export {};
//# sourceMappingURL=SentoriSuspense.d.ts.map