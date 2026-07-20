// Phase 45 sub-B — Vue Router auto-trace navigation.
//
//     import { createRouter } from 'vue-router'
//     import { setupTraceNavigation } from '@goliapkg/sentori-vue/router'
//
//     const router = createRouter({ ... })
//     setupTraceNavigation(router)
//
// On every route push, we open a `vue.navigation` span keyed by the
// destination path, mark it active, and finish it on the next
// `afterEach`. Sentori spans / fetch / xhr instrumentation in
// `@goliapkg/sentori-javascript` automatically nest into it so each
// screen's network requests cluster into one trace.
import { setActiveSpan, startSpan } from '@goliapkg/sentori-core';
import { captureStep } from '@goliapkg/sentori-javascript';
let _active = null;
export function setupTraceNavigation(router) {
    router.beforeEach((to, from) => {
        // Finish any still-open span from the previous transition that
        // afterEach didn't reach (route guard rejected, etc.).
        if (_active) {
            _active.finish({ status: 'ok' });
            _active = null;
        }
        const name = `${from.path || '/'} → ${to.path || '/'}`;
        const span = startSpan('vue.navigation', {
            name,
            parent: null,
            tags: { 'nav.from': from.path || '/', 'nav.to': to.path || '/' },
        });
        _active = span;
        setActiveSpan(span);
        // Phase 46 — also record into the session-trail buffer; no-op
        // unless `init({ capture: { sessionTrail: true } })`.
        captureStep(`route:${to.path || '/'}`, {
            breadcrumb: { type: 'navigation', message: name },
        });
    });
    router.afterEach(() => {
        if (_active) {
            _active.finish({ status: 'ok' });
            _active = null;
        }
    });
}
//# sourceMappingURL=router.js.map