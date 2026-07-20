// App Router helpers for Next.js 14+. Imports from `next/navigation`
// (Next-side), so this module is client-only and is published under
// the `/app-router` subpath so the server bundle never tries to load
// `next/navigation` during instrumentation.
'use client';
import { addBreadcrumb, captureError } from '@goliapkg/sentori-javascript';
import { usePathname } from 'next/navigation';
import { useEffect, useRef } from 'react';
/**
 * Subscribe to App Router pathname transitions and push a `nav`
 * breadcrumb on every change. Mount once in a layout's client
 * component (e.g. a `<Shell>` in `app/layout.tsx`):
 *
 *     'use client'
 *     import { useNextRouter } from '@goliapkg/sentori-next/app-router'
 *     export function Shell({ children }: { children: React.ReactNode }) {
 *       useNextRouter()
 *       return children
 *     }
 *
 * First mount does NOT emit a breadcrumb — only real transitions.
 *
 * Intentionally does not read `useSearchParams()` — that hook
 * requires a Suspense boundary in Next.js 14+ which complicates
 * adoption. Pathname alone covers the breadcrumb story.
 */
export function useNextRouter() {
    const pathname = usePathname();
    const prevRef = useRef(null);
    useEffect(() => {
        const prev = prevRef.current;
        if (prev !== null && prev !== pathname) {
            addBreadcrumb({ data: { from: prev, to: pathname }, type: 'nav' });
        }
        prevRef.current = pathname;
    }, [pathname]);
}
/**
 * Capture an error from an App Router `error.tsx` file. Idiomatic
 * usage:
 *
 *     // app/error.tsx
 *     'use client'
 *     import { useReportNextError } from '@goliapkg/sentori-next/app-router'
 *     export default function ErrorPage({ error, reset }: {
 *       error: Error & { digest?: string }
 *       reset: () => void
 *     }) {
 *       useReportNextError(error)
 *       return (
 *         <div>
 *           <h2>Something went wrong.</h2>
 *           <button onClick={reset}>Try again</button>
 *         </div>
 *       )
 *     }
 *
 * The hook calls captureError exactly once per error instance
 * (subsequent renders with the same error are no-ops). Picks up
 * Next's `digest` field as a tag so the dashboard can correlate
 * the client report with the server error.
 */
export function useReportNextError(error) {
    const seenRef = useRef(null);
    useEffect(() => {
        if (seenRef.current === error)
            return;
        seenRef.current = error;
        captureError(error, {
            tags: {
                'next.digest': error.digest ?? '',
                source: 'next.error.tsx',
            },
        });
    }, [error]);
}
//# sourceMappingURL=app-router.js.map