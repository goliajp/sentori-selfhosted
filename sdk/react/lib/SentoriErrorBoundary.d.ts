import { type ErrorInfo, type ReactNode } from 'react';
type FallbackRender = (props: {
    error: Error;
    reset: () => void;
}) => ReactNode;
type Props = {
    children: ReactNode;
    /**
     * Rendered after an error is caught. Either a plain ReactNode
     * (most common — a static error screen) or a render-prop that
     * receives the error and a `reset` callback so the fallback can
     * offer a retry button.
     */
    fallback: FallbackRender | ReactNode;
    /** Optional additional logging hook. Runs after Sentori capture. */
    onError?: (error: Error, info: ErrorInfo) => void;
    /**
     * Shallow-compared on update. Any change resets the boundary,
     * letting parents recover from a caught error by passing fresh
     * keys (e.g. a route path, a query key, a user id).
     */
    resetKeys?: unknown[];
};
export declare function SentoriErrorBoundary(props: Props): import("react/jsx-runtime").JSX.Element;
export {};
//# sourceMappingURL=SentoriErrorBoundary.d.ts.map