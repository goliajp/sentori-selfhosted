import { type ReactNode } from 'react';
/**
 * Wrap a subtree so its mount-to-unmount lifespan becomes a
 * `react.render` span. Useful for measuring how long a heavy
 * component sat on screen — a data table, a chart, a Suspense'd
 * data fetch:
 *
 *     <TraceRender op="react.render" name="OrdersTable">
 *       <OrdersTable />
 *     </TraceRender>
 *
 * Implementation notes:
 *
 * - The span opens when the component renders for the first time
 *   (in the body of the function, before children render — so child
 *   spans pick this one up as their parent via `activeSpan()` if
 *   they're synchronous; React renders top-down but yields between
 *   commits, so async children won't necessarily attribute to this
 *   span unless they wrap with `withSpan`).
 * - The span closes in a `useEffect` cleanup. That runs at unmount
 *   in normal mode, or twice in StrictMode (mount-unmount-mount).
 *   StrictMode double-invocation just emits the span twice; this is
 *   a known dev-mode artifact and matches how React's profiler
 *   accounts for it.
 * - Re-renders due to prop / state change do NOT restart the span.
 *   The mount/unmount boundary is the lifespan. Callers wanting
 *   per-render timing should use `useRenderTrace` instead (TODO).
 */
export declare function TraceRender({ children, data, name, op, tags, }: {
    children: ReactNode;
    /** Span data, attached at finish time. */
    data?: Record<string, unknown>;
    /** Defaults to `op`. */
    name?: string;
    /** Defaults to `react.render`. */
    op?: string;
    tags?: Record<string, string>;
}): ReactNode;
//# sourceMappingURL=SentoriTrace.d.ts.map