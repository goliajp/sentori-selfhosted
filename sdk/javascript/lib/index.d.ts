export { addBreadcrumb, clearBreadcrumbs, getBreadcrumbs } from './breadcrumbs.js';
export { captureError, captureException, captureMessage, captureStep, getUser, setTag, setTags, setUser, } from './capture.js';
export { initSentori } from './init.js';
export { startSpan, startTrace, withScopedSpan, type SpanContextLike, type StartSpanOptions, } from '@goliapkg/sentori-core';
export { RuntimeMetricBuffer, drainRuntimeMetricsForFlush, emitMetric, rebufferRuntimeMetrics, type RuntimeMetricPoint, } from '@goliapkg/sentori-core';
export { flushRuntimeMetrics, startRuntimeMetricsTimer, stopRuntimeMetricsTimer, } from './runtime-metrics.js';
export type { CaptureMessageOptions, MessageLevel, TrailStep, } from '@goliapkg/sentori-core';
export type { Breadcrumb, BreadcrumbType, CaptureExtras, Event, Frame, InitOptions, ReadyInfo, SentoriError, Tags, User, } from './types.js';
/** v2.3 — logger surface re-exported from core so hosts can
 *  `import { setLogLevel } from '@goliapkg/sentori-javascript'`
 *  per design §3 ("Production override"). `setLogTransport` lets
 *  hosts route Sentori-internal lines into their own log
 *  aggregator. */
export { getLogLevel, type LogLevel, logger, type LogTransport, setLogLevel, setLogTransport, } from '@goliapkg/sentori-core';
export { registerWeb, unregisterWeb, readCachedIpt, type RegisterWebOptions, type RegisterWebResult, } from './push.js';
//# sourceMappingURL=index.d.ts.map