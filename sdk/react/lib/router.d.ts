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
export declare function useSentoriRouter(): void;
//# sourceMappingURL=router.d.ts.map