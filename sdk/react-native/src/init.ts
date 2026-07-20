import { setLogLevel } from '@goliapkg/sentori-core';

import { type ReadyInfo, setConfig } from './config';
import { installGlobalHandler } from './handlers/global';
import { installLifecycleHandler } from './handlers/lifecycle';
import { installPromiseHandler } from './handlers/promise';
import { installNetworkHandler } from './handlers/network';
import { getBundleInfo } from './bundle-info';
import {
  markLaunchCompleted,
  runLaunchCrashGuard,
} from './launch-crash-guard';
import { getInstallId } from './install-id';
import { startMetricsTimer } from './metrics';
import {
  emitColdStart,
  markColdStartT0,
  startRuntimeMetricsTimer,
} from './runtime-metrics';
import { startFpsInstrument } from './runtime-metrics-fps';
import { startHeapInstrument } from './runtime-metrics-heap';
import { startNetworkBytesInstrument } from './runtime-metrics-network';
import { startTrackTimer } from './track';
import { drainNativePending, markNativeJsBridgeReady, setNativeConfig } from './native';
import { getColdStartMs } from './mobile-vitals';
import { startHeartbeat, type HeartbeatOptions } from './heartbeat';
import { startSpan } from '@goliapkg/sentori-core';
import { startControlChannel } from './control-channel';
import { startLongTaskMonitor } from './long-task-monitor';
import { startNetworkTypeWatch } from './netinfo';
import { startPreCrashSentinel, type PreCrashChannel } from './pre-crash-sentinel';
import { startReplay } from './replay';
import { startSampleProfiler } from './sample-profiler';
import { startSession } from './session-tracker';
import {
  drainOfflineQueue,
  enqueue,
  startTransport,
  uploadAttachment,
} from './transport';
import type { AttachmentKind, AttachmentMeta, AttachmentSource, Event } from './types';

declare const __DEV__: boolean | undefined;

export type InitOptions = {
  /** Project token starting with `st_pk_`. Required. */
  token: string;
  /** Release identifier, e.g. `myapp@1.2.3+456`. Required. */
  release: string;
  /** Environment label. Defaults to `dev` if `__DEV__`, else `prod`. */
  environment?: string;
  /** Override ingestion URL (self-hosted). Default: https://ingest.sentori.golia.jp */
  ingestUrl?: string;
  /** Toggle individual capture sources. All enabled by default. */
  capture?: {
    globalErrors?: boolean;
    promiseRejections?: boolean;
    network?:
      | boolean
      | {
          /** v0.9.0 #11 — auto-extract GraphQL `operationName` from
           *  POST request bodies and use it as the breadcrumb / span
           *  name (instead of `POST /graphql`). Default `true`. */
          graphql?: boolean;
        };
    /** Session tracking: opens a session on init and on each
     *  foreground (`AppState` → `active`), ends it on background.
     *  Drives crash-free rate. Set `false` to opt out. */
    sessions?: boolean;
    /** Capture a screenshot of the current screen on
     *  `captureException`. Opt-in. The capture runs through the
     *  bundled native module — no extra peer dep required since
     *  v0.7.3. To redact PII regions, register a mask query via
     *  `sentori.registerMaskQuery(() => string[])` and put
     *  `nativeID="..."` on the `<View>`s the SDK should black out.
     *  The image is webp q=70 / jpeg q=70 at 480 px max, < 100 KB
     *  typical. */
    screenshot?: boolean;
    /** Phase 46: record the last N steps (route changes, custom
     *  breadcrumbs) leading up to a crash. On `captureException`
     *  the buffer is sealed and uploaded as a `sessionTrail`
     *  attachment. Defaults to false. */
    sessionTrail?: boolean;
    /** v2.0 W3 — when `true`, every `sentori.track(name, props)`
     *  also pushes a `{ type: 'track', data: { name, props } }`
     *  breadcrumb so a subsequent `captureException` /
     *  `captureMessage` carries the customer journey leading up to
     *  the failure. Defaults to `false` to preserve v1 customer
     *  breadcrumb shape on upgrade; recommended `true` for new
     *  integrations. See `docs/recipes/track-and-metrics.md`. */
    trackAutoBreadcrumb?: boolean;
    /** v2.1 W2 — auto-instrument runtime metrics (FPS, JS heap,
     *  cold-start, route nav timing, network bytes). Drains the
     *  shared `@goliapkg/sentori-core` ring to
     *  `/v1/runtime-metrics:batch` every 30 s. Defaults to `true`
     *  in W2 part 2 — cold-start only, ~6 emits per session
     *  per device, ~zero main-thread cost. Higher-cost instruments
     *  (FPS / route-nav) land in W2 part 3 with per-tick perf
     *  budget tests as stop-ship gates per
     *  `.claude/CLAUDE.md` performance bedrock. */
    runtimeMetrics?: boolean;
    /** v0.9.1 +S4 — pre-crash sentinel. Subscribes to JS-thread
     *  frame timing; when ≥ 50% of a 60-frame window misses the
     *  budget (default 32 ms / < 30 fps), emits a `kind: nearCrash`
     *  event proactively so dashboards see the "about-to-die"
     *  signal before an actual crash. */
    preCrashSentinel?: boolean;
    sentinelChannels?: PreCrashChannel[];
    /** v0.9.6 #4 — JS-thread long-task monitor. setInterval(50ms)
     *  tick detects JS thread stalls ≥ 200ms (configurable) and
     *  emits a `sentori.longtask` span. Pairs with
     *  `preCrashSentinel` (slow frames < 32ms) to cover the
     *  "JS thread is stuck" spectrum. */
    longTaskMonitor?: boolean | { thresholdMs?: number };
    /** v0.9.6 #2 — wireframe session replay. Native walks the iOS
     *  UIView / Android View hierarchy at 1 Hz and serializes
     *  visible nodes; captureException flushes the last 60 s as a
     *  `replay` attachment. Set to `'wireframe'` to enable. */
    replay?: 'off' | 'wireframe' | { hz?: number; mode: 'off' | 'wireframe' };
    /** v1.1 #4 升级 — JS sample profiler. setInterval(50ms) idle-tick
     *  sampler aggregates frame counts → emits sentori.profile span
     *  every 60s with `flameData` (frame → tick count). Pairs with
     *  longTaskMonitor (≥200ms outliers) — sample profiler 看 idle
     *  分布、long-task 看 outliers。 */
    sampleProfiler?: boolean | { flushMs?: number; sampleMs?: number };
    /** Analytics v1 — live-presence heartbeat. Foreground 1/min
     *  default; opt out with `false`, or pass an options object
     *  to tune. Powers the Audience > Live dashboard.
     *  Iron-rule budget: < 1 KB / min, < 1 ms / call. */
    heartbeat?: boolean | HeartbeatOptions;
    /** v0.9.0 #3 — launch-crash loop guard. When two consecutive
     *  launches don't reach `markLaunchCompleted()` (typical of an
     *  OTA update with a fatal bug), invoke the host callback with
     *  a 200 ms timeout to decide rollback / reset / continue. */
    launchCrashGuard?: {
      enabled: boolean;
      onLaunchCrashDetected?: (
        info: import('./launch-crash-guard').LaunchCrashInfo,
      ) =>
        | import('./launch-crash-guard').LaunchCrashAction
        | Promise<import('./launch-crash-guard').LaunchCrashAction>;
      threshold?: number;
      timeoutMs?: number;
    };
  };
  /** Phase 44 sub-B: client-side sampling. Each rate is `[0, 1]`;
   *  absent / null keeps everything. Defaults to 1.0 for both
   *  (no drop). Set traces to e.g. 0.1 once the app's at user
   *  volume to keep ingest budget under control without changing
   *  the server-side quota. Decisions are made per-event for
   *  errors and per-trace (all spans together) for traces. */
  sampling?: {
    errors?: null | number;
    traces?: null | number;
    /** v2.0 — sampling rate for `kind: 'message'` events emitted
     *  via `sentori.captureMessage()`. `null` / absent → keep all. */
    messages?: null | number;
  };
  /** v2.3 — canonical sampling field (renamed from `sampling`).
   *  Same shape; if both are passed, `sample` wins. The older
   *  `sampling` stays accepted indefinitely as a back-compat alias —
   *  this is the kind of rename that's only worth doing if it's
   *  also zero-cost for existing callers. */
  sample?: {
    errors?: null | number;
    traces?: null | number;
    messages?: null | number;
  };
  /** v2.3 — Sentori SDK's own console output gate. Default `'warn'`:
   *  SDK is silent unless something is genuinely broken (transport
   *  sustained failure, native module not found, internal SDK
   *  exception). Set to `'silent'` for absolute silence; bump to
   *  `'info'` / `'debug'` when debugging Sentori itself. */
  logLevel?: import('@goliapkg/sentori-core').LogLevel;
  /** v2.3 — fires once after init completes. Use this to know the
   *  SDK is live instead of scanning the console. The `ReadyInfo`
   *  carries native-module bind status + cold-start timing. */
  onReady?: (info: ReadyInfo) => void;
  /** v2.3 — mutate-or-drop hook on each outbound event (sync).
   *  Return the event to ship it (possibly mutated), or `null` to
   *  drop. See `BeforeSendHook` for the throwing / non-event
   *  fallback policy. Used for host-side PII scrubbing the SDK
   *  can't do automatically; server-side privacy_lab still runs
   *  regardless. */
  beforeSend?: import('@goliapkg/sentori-core').BeforeSendHook;
};

const DEFAULT_INGEST_URL = 'https://ingest.sentori.golia.jp';

export const init = (options: InitOptions): void => {
  if (!options.token || !options.token.startsWith('st_pk_')) {
    throw new Error("Sentori: token is required and must start with 'st_pk_'");
  }
  if (!options.release) {
    throw new Error('Sentori: release is required');
  }

  // v2.3 — set log level FIRST so any startup-time logger calls
  // are gated correctly. Default 'warn' from logger.ts; an explicit
  // host setting overrides.
  setLogLevel(options.logLevel);

  const env =
    options.environment ??
    (typeof __DEV__ !== 'undefined' && __DEV__ ? 'dev' : 'prod');

  // v0.9.0 #3 — launch-crash guard. Fires *before* any other setup so
  // a known-bad bundle can roll back instead of running JS that's
  // about to die again. AsyncStorage-backed; if the host doesn't have
  // it the guard is a no-op.
  const lcg = options.capture?.launchCrashGuard;
  if (lcg?.enabled) {
    void runLaunchCrashGuard(
      lcg,
      options.release,
      getBundleInfo()?.id ?? null,
    );
  }

  setConfig({
    token: options.token,
    release: options.release,
    environment: env,
    ingestUrl: options.ingestUrl ?? DEFAULT_INGEST_URL,
    enabled: true,
    screenshotsEnabled: options.capture?.screenshot === true,
    // v2.3 — `sample` is the canonical name; `sampling` is the
    // back-compat alias. If both are supplied, `sample` wins.
    errorSampleRate: options.sample?.errors ?? options.sampling?.errors ?? null,
    traceSampleRate: options.sample?.traces ?? options.sampling?.traces ?? null,
    messageSampleRate: options.sample?.messages ?? options.sampling?.messages ?? null,
    sessionTrailEnabled: options.capture?.sessionTrail === true,
    // v2.0 W3 — when true, every `track()` also pushes a
    // `type: 'track'` breadcrumb so a subsequent captureException
    // carries the customer journey. Defaults false to preserve v1
    // breadcrumb shape on upgrade.
    trackAutoBreadcrumb: options.capture?.trackAutoBreadcrumb === true,
    // v2.3 — host-side beforeSend hook (sync). Stored on Config so
    // capture.ts can pull it without re-resolving the InitOptions.
    beforeSend: options.beforeSend,
  });

  // Tell the native crash handler about the config so the JSON it writes
  // on the next NSException / Java uncaught carries release + env.
  setNativeConfig({
    token: options.token,
    release: options.release,
    environment: env,
  });
  // v0.9.4 #1 — finalize cold-start measurement. iOS uses the
  // delta from `applicationDidFinishLaunching` to this call;
  // Android uses Process.getStartElapsedRealtime() so the value is
  // computed at this point and cached.
  markNativeJsBridgeReady();
  // Emit a one-off cold-start span. Server aggregates these per
  // release for the Mobile Vitals dashboard. No-op when native
  // module isn't linked.
  const coldMs = getColdStartMs();
  if (coldMs !== null && coldMs > 0 && coldMs < 60_000) {
    const span = startSpan('sentori.cold_start', {
      name: 'cold-start',
      parent: null,
      startNowMs: Date.now() - coldMs,
      tags: { 'vital.kind': 'cold_start' },
    });
    span.finish({ status: 'ok' });
  }

  startTransport();
  // v1.1 +S7 升级 — control channel poll for live-debug flag.
  startControlChannel();
  // v0.8.0-c — start watching network class. No-op if NetInfo isn't
  // installed; events just won't carry `device.networkType` in that
  // case.
  startNetworkTypeWatch();
  // v0.8.3 — drain custom-metric ring every 30 s.
  startMetricsTimer();
  // v1.1 chunk B — drain `sentori.track()` ring every 30 s.
  startTrackTimer();
  // v2.1 W2 — runtime metrics auto-instrument. Defaults on; host
  // can opt out with `capture: { runtimeMetrics: false }`. The
  // ring + emit live in `@goliapkg/sentori-core`; we own the
  // 30 s flusher + the cold-start one-shot here. FPS / heap /
  // route-nav / network instruments ship in W2 part 3 (each
  // gated on its own per-tick perf budget CI test).
  if (options.capture?.runtimeMetrics !== false) {
    markColdStartT0();
    startRuntimeMetricsTimer();
    // Defer the emit one tick so React's first paint settles
    // before we stamp "cold-start ended". 0-delay setTimeout puts
    // the call after the current microtask queue drains.
    setTimeout(emitColdStart, 0);
    // FPS via rAF (per-tick budget < 0.5 ms — see
    // runtime-metrics-fps.ts header). Heap is a 30 s polling
    // tick; no-op when performance.memory isn't exposed (most
    // RN engines today; we ship the wiring anyway so the same
    // SDK works on web targets via @goliapkg/sentori-javascript).
    startFpsInstrument();
    startHeapInstrument();
    // Network bytes counters drain every 30 s. The counters
    // themselves are incremented inline by handlers/network.ts
    // on every fetch round-trip — see the recordNetworkBytes
    // call site there. No-op when `capture.network: false` is
    // set (no fetch patch → no counter increments).
    startNetworkBytesInstrument();
    // Route-nav dwell timing emits inline from `navigation.ts`'s
    // useTraceNavigation state listener — no extra start call
    // needed; the host already mounts that hook for tracing.
  }
  // v1.1 chunk S1 — warm the install-id cache. Fire-and-forget;
  // any event captured before the first resolve simply omits
  // `device.installId`. Subsequent captures pick it up via the
  // sync `peekInstallId()` read in collectDevice().
  void getInstallId();
  // v0.9.1 +S4 — pre-crash sentinel. Off by default; opt-in via
  // `capture.preCrashSentinel: true`.
  if (options.capture?.preCrashSentinel === true) {
    startPreCrashSentinel({
      enabled: true,
      channels: options.capture.sentinelChannels,
    });
  }
  // v0.9.6 #4 — long-task monitor. Off by default.
  const lt = options.capture?.longTaskMonitor;
  if (lt) {
    startLongTaskMonitor({
      enabled: true,
      thresholdMs: typeof lt === 'object' ? lt.thresholdMs : undefined,
    });
  }
  // v0.9.6 #2 — wireframe replay. Off by default.
  const rp = options.capture?.replay;
  if (rp === 'wireframe') {
    startReplay({ mode: 'wireframe' });
  } else if (rp && typeof rp === 'object' && rp.mode === 'wireframe') {
    startReplay({ hz: rp.hz, mode: 'wireframe' });
  }
  // v1.1 #4 升级 — JS sample profiler. Off by default.
  const sp = options.capture?.sampleProfiler;
  if (sp) {
    startSampleProfiler({
      enabled: true,
      flushMs: typeof sp === 'object' ? sp.flushMs : undefined,
      sampleMs: typeof sp === 'object' ? sp.sampleMs : undefined,
    });
  }

  const capture = options.capture ?? {};
  if (capture.globalErrors !== false) installGlobalHandler();
  if (capture.promiseRejections !== false) installPromiseHandler();
  if (capture.network !== false) {
    const netOpts = typeof capture.network === 'object' ? capture.network : undefined;
    installNetworkHandler({ graphql: netOpts?.graphql });
  }
  if (capture.sessions !== false) {
    // Open the cold-start session now (RN doesn't fire an AppState
    // `change` for the initial `active` state), then bind AppState so
    // background ends it and the next foreground opens a fresh one.
    startSession();
    installLifecycleHandler();
  }
  // Analytics v1 — live-presence heartbeat. Foreground 1/min by
  // default; pass `capture.heartbeat = false` to opt out, or
  // `{ intervalMs: 30_000 }` to override. The default budget is well
  // under the "几乎不能造成性能抖动" rule (CLAUDE.md).
  const heartbeatOpt = capture.heartbeat;
  if (heartbeatOpt !== false) {
    startHeartbeat(typeof heartbeatOpt === 'object' ? heartbeatOpt : {});
  }

  // Drain events persisted from previous session (best-effort):
  // - native crashes from <Documents>/sentori/pending/*.json
  // - JS transport offline queue from AsyncStorage
  drainNativePending()
    .then(async (items) => {
      for (const json of items) {
        try {
          const event = JSON.parse(json) as Event & {
            _pendingAttachments?: PendingAttachment[];
          };
          // Phase 42 sub-E.05 / F.09: the native crash handler couldn't
          // upload attachments at crash time (the app was dying); it
          // base64-encoded them into `_pendingAttachments` instead.
          // On next launch we upload each before enqueueing the event,
          // so the dashboard sees the refs in `event.attachments[]`.
          if (event._pendingAttachments && event._pendingAttachments.length > 0) {
            for (const p of event._pendingAttachments) {
              const meta = await uploadAttachment(
                event.id,
                p.kind,
                { base64: p.base64, mediaType: p.mediaType },
                { source: p.source },
              );
              if (meta) {
                if (!event.attachments) event.attachments = [];
                event.attachments.push(meta);
              }
            }
            delete event._pendingAttachments;
          }
          enqueue(event);
        } catch {
          // skip malformed
        }
      }
    })
    .catch(() => {});
  drainOfflineQueue().catch(() => {});

  // v0.9.0 #3 — init reached the end without throwing. Schedule the
  // "launch completed" marker after one tick so any synchronous user
  // code right after `init()` gets to run first; we want the marker to
  // confirm the JS bridge stayed alive, not just that `init()` returned.
  if (lcg?.enabled) {
    setTimeout(() => {
      void markLaunchCompleted(getBundleInfo()?.id ?? null);
    }, 2_000);
  }

  // v2.3 — onReady callback. Fires after setConfig + native bind +
  // transport start are all settled. The drain-pending work is
  // still in flight (it's async) but the SDK is ready to accept
  // new captures. Host wires this to know the SDK is live without
  // scanning the console. Wrapped in try/catch — host callback
  // throwing must not propagate (NEVER rule).
  if (options.onReady) {
    const nativeMod = (() => {
      try {
        // eslint-disable-next-line @typescript-eslint/no-require-imports
        const { native } = require('./native');
        const n = native?.();
        return n ?? null;
      } catch {
        return null;
      }
    })();
    const info: ReadyInfo = {
      sdkVersion: SDK_VERSION,
      coldStartMs: coldMs ?? undefined,
      native: {
        bound: !!nativeMod,
        methods: nativeMod ? Object.keys(nativeMod).sort() : [],
      },
    };
    try {
      options.onReady(info);
    } catch {
      // Host's onReady threw. NEVER rule — swallow.
    }
  }
};

// Bumped on each SDK release; surfaced in onReady payload + the
// future Sentry-compat layer's identification string.
const SDK_VERSION = '2.3.0';

/**
 * Phase 42 sub-E.05: shape of each entry in the native crash JSON's
 * `_pendingAttachments` array. Mirrors what
 * `SentoriCrashHandler.write` writes on iOS and (sub-F) what
 * `SentoriCrashWriter` writes on Android.
 */
type PendingAttachment = {
  base64: string;
  kind: AttachmentKind;
  mediaType: string;
  source: AttachmentSource;
};

// Keep AttachmentMeta in the imports — it's part of the public type
// surface re-exported from this module's bundle.
export type { AttachmentMeta };
