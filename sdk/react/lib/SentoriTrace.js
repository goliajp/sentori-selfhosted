import { useEffect, useMemo, useRef } from 'react';
import { startSpan } from '@goliapkg/sentori-core';
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
export function TraceRender({ children, data, name, op = 'react.render', tags, }) {
    // Lazy-init via useMemo so the span is created exactly once across
    // re-renders. Returning the handle from useMemo also means the
    // effect cleanup captures the same reference.
    const span = useMemo(() => startSpan(op, { data, name: name ?? op, tags }), 
    // We deliberately do NOT include op/name/data/tags in deps —
    // changing them after first render shouldn't reopen the span;
    // the lifespan is "this component instance", not "these props".
    // eslint-disable-next-line react-hooks/exhaustive-deps
    []);
    // The handle is alive across renders but should not be exposed to
    // child components (they make their own spans). We hold it via ref
    // so React's strict-mode-friendly invariants are preserved.
    const spanRef = useRef(span);
    spanRef.current = span;
    useEffect(() => {
        return () => {
            // Finish on unmount. Second call is a no-op (SpanHandle's
            // own contract), so StrictMode double-effects don't double-push.
            spanRef.current?.finish({ status: 'ok' });
            spanRef.current = null;
        };
    }, []);
    return children;
}
//# sourceMappingURL=SentoriTrace.js.map