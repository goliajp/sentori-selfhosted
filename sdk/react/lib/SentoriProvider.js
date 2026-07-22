import { jsx as _jsx } from "react/jsx-runtime";
import { addBreadcrumb, captureError, captureException, initSentori, setUser, } from '@goliapkg/sentori-javascript';
import { createContext, useContext, useMemo, useRef, useState } from 'react';
const Ctx = createContext(null);
let _tags = null;
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
export function SentoriProvider({ children, config, }) {
    const initialisedRef = useRef(false);
    const [initialised, setInitialised] = useState(false);
    if (!initialisedRef.current) {
        initialisedRef.current = true;
        try {
            initSentori(config);
            setInitialised(true);
        }
        catch (e) {
            // Misconfiguration (bad token shape, missing fields). Warn — never
            // error — so we don't add red noise to the host app's console.
            // The rest of the tree should still render.
            // eslint-disable-next-line no-console
            console.warn('[sentori-react] init failed', e);
        }
    }
    const value = useMemo(() => ({
        addBreadcrumb: (type, data) => {
            addBreadcrumb({ data: data ?? {}, type });
        },
        captureError: (err, extras) => {
            captureError(err, mergeExtras(extras));
        },
        captureException: (err, extras) => {
            captureException(err, mergeExtras(extras));
        },
        initialised,
        setTags: (tags) => {
            _tags = tags;
        },
        setUser,
    }), [initialised]);
    return _jsx(Ctx.Provider, { value: value, children: children });
}
/**
 * Merge provider-scoped tags into per-call extras. Per-call wins over
 * provider-scoped on conflict (matches Sentry's semantics).
 */
function mergeExtras(extras) {
    if (!_tags)
        return extras;
    return { ...extras, tags: { ..._tags, ...(extras?.tags ?? {}) } };
}
export function useSentoriCtx() {
    const ctx = useContext(Ctx);
    if (!ctx) {
        throw new Error('[sentori-react] hook used outside <SentoriProvider>. ' +
            'Wrap your app at or above the component that calls useSentori / useCaptureError.');
    }
    return ctx;
}
//# sourceMappingURL=SentoriProvider.js.map