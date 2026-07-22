import { setActiveSpan, startSpan } from '@goliapkg/sentori-core';
import { captureStep } from '@goliapkg/sentori-javascript';
import { useEffect, useRef } from 'react';
import { useLocation } from 'react-router';
import { useSentoriCtx } from './SentoriProvider.js';
/**
 * Subscribe to `react-router` navigation. On every pathname/search/
 * hash change this:
 *
 *   - pushes a `nav` breadcrumb (`{ from, to }`)
 *   - opens a `react.navigation` span (a fresh trace root) for the new
 *     route and makes it the active span — so any `http.client` /
 *     other spans created while that route is mounted attach to it as
 *     children (one trace per route instead of one per request)
 *
 * Mount once high in the tree (inside the `Router` and inside
 * `SentoriProvider`):
 *
 *     function AppShell() {
 *       useSentoriRouter()
 *       return <Outlet />
 *     }
 *
 * The first render does NOT emit a `nav` breadcrumb (there's no
 * transition) but DOES open the route span for the landing page, so
 * its requests are grouped too. On unmount the open route span is
 * finished and the active span cleared.
 *
 * Peer dependency: `react-router >= 7`. Separate entry point so apps
 * not using react-router don't pay the import cost.
 */
export function useSentoriRouter() {
    const { addBreadcrumb } = useSentoriCtx();
    const location = useLocation();
    const prevRef = useRef(null);
    const openSpanRef = useRef(null);
    const next = location.pathname + location.search + location.hash;
    useEffect(() => {
        const prev = prevRef.current;
        if (prev === next)
            return;
        if (prev !== null)
            addBreadcrumb('nav', { from: prev, to: next });
        openSpanRef.current?.finish({ status: 'ok' });
        const span = startSpan('react.navigation', {
            name: prev ? `${prev} → ${next}` : next,
            parent: null, // each route is its own trace root
            tags: { 'nav.from': prev ?? '', 'nav.to': next },
        });
        openSpanRef.current = span;
        setActiveSpan(span);
        // Phase 46 — session-trail step. No-op unless
        // `init({ capture: { sessionTrail: true } })`.
        captureStep(`route:${next}`, {
            breadcrumb: { type: 'navigation', message: prev ? `${prev} → ${next}` : next },
        });
        prevRef.current = next;
    }, [addBreadcrumb, next]);
    useEffect(() => () => {
        openSpanRef.current?.finish({ status: 'ok' });
        openSpanRef.current = null;
        setActiveSpan(null);
    }, []);
}
//# sourceMappingURL=router.js.map