// Phase 45 sub-B — Vue error boundary component.
//
// Vue 3 doesn't ship a built-in ErrorBoundary like React. The
// pattern is to use the `errorCaptured(err, instance, info)`
// lifecycle hook on a wrapper component that returns `false` to
// stop the error from propagating. Our wrapper captures into
// Sentori and renders either the slot's children or a `fallback`
// slot when the subtree threw.
import { coerceError } from '@goliapkg/sentori-core';
import { captureException } from '@goliapkg/sentori-javascript';
import { defineComponent, h, ref } from 'vue';
export const SentoriErrorBoundary = defineComponent({
    name: 'SentoriErrorBoundary',
    props: {
        /** Optional list of error names (`error.name`) to ignore — they
         *  pass through to upper boundaries unchanged. */
        ignore: { type: Array, default: () => [] },
    },
    setup(_props, { slots }) {
        const caughtError = ref(null);
        const reset = () => {
            caughtError.value = null;
        };
        return () => {
            if (caughtError.value) {
                if (slots.fallback) {
                    return slots.fallback({ error: caughtError.value, reset });
                }
                // Default fallback: a hidden span so Vue doesn't render a
                // crashed subtree. Apps that don't pass a fallback opt in
                // to that minimal behaviour.
                return h('span', { 'data-sentori-boundary-error': 'true' });
            }
            return slots.default?.();
        };
    },
    errorCaptured(err, _instance, info) {
        // `coerceError` JSON-stringifies plain-object throws so the
        // dashboard shows the real payload instead of `[object Object]`.
        const e = coerceError(err);
        if (this.ignore.includes(e.name)) {
            return true; // propagate further
        }
        captureException(e, { tags: { 'vue.errorInfo': info } });
        // Switch to fallback render and stop propagation.
        this.$forceUpdate();
        this.caughtError = { value: e };
        return false;
    },
});
//# sourceMappingURL=ErrorBoundary.js.map