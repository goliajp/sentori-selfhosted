/**
 * Phase 45 sub-B — Vue 3 adapter for Sentori.
 *
 * Plugin shape:
 *
 *     import { createApp } from 'vue'
 *     import sentori from '@goliapkg/sentori-vue'
 *
 *     const app = createApp(App)
 *     app.use(sentori, {
 *       token: 'st_pk_…',
 *       release: 'myapp@1.0.0',
 *       sampling: { errors: 1.0 },
 *     })
 *
 * What `app.use(sentori, opts)` does:
 *   1. forwards `opts` to `@goliapkg/sentori-javascript`'s init
 *   2. wires `app.config.errorHandler` so any error thrown inside
 *      a render / lifecycle bubbles into `captureException`
 *   3. tags every Sentori event with `tags.vue.version` so the
 *      dashboard knows which framework is producing the data
 *
 * Router integration (Vue Router) lives in the `/router` subpath:
 *
 *     import { setupTraceNavigation } from '@goliapkg/sentori-vue/router'
 *     setupTraceNavigation(router)
 */
import type { Plugin } from 'vue';
import { type InitOptions } from '@goliapkg/sentori-javascript';
export type SentoriVueOptions = InitOptions;
declare const plugin: Plugin;
export default plugin;
export { plugin as sentori };
export { addBreadcrumb, captureException, captureException as captureError, captureMessage, captureStep, getUser, setUser, } from '@goliapkg/sentori-javascript';
export type { CaptureMessageOptions, MessageLevel, } from '@goliapkg/sentori-javascript';
export { RuntimeMetricBuffer, drainRuntimeMetricsForFlush, emitMetric, flushRuntimeMetrics, rebufferRuntimeMetrics, startRuntimeMetricsTimer, stopRuntimeMetricsTimer, type RuntimeMetricPoint, } from '@goliapkg/sentori-javascript';
export { SentoriErrorBoundary } from './ErrorBoundary.js';
export { registerWeb, unregisterWeb, readCachedIpt, type RegisterWebOptions, type RegisterWebResult, } from '@goliapkg/sentori-javascript';
export type { PushMessage, PushOptions, PushPriority, PushReceipt, PushTicket, PushTicketStatus, } from '@goliapkg/sentori-core';
//# sourceMappingURL=index.d.ts.map