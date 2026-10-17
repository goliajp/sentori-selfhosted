/** Optional entry point: import only after the host opts into real-user measurements. */
import { onCLS, onFCP, onINP, onLCP, onTTFB, type Metric } from 'web-vitals'
import { safeFn, uuidV7 } from '@goliapkg/sentori-core'
import { getConfig } from './config.js'
import { registerEmitHook } from './emit-hooks.js'
import { installNetworkErrors } from './network-errors.js'
import { patchContext } from './scope.js'
import { error } from './verbs.js'
import { SDK_VERSION } from './transport.js'
import { boundedVisits, normalizeRoute, resourceSummary, type Visit } from './performance-model.js'

export type WebMonitoringOptions = {
  /** Probability per document/restored visit, including healthy visits. Required, (0, 1]. */
  sampleRate: number
  /** Prefer router templates, e.g. /orders/:id. Never return customer identifiers. */
  route?: () => string
  /** Report failed script, stylesheet and image loads as errors. Off by default. */
  resourceErrors?: boolean
  /** Report fetch/XHR connectivity failures and HTTP 5xx; ignores aborts, opaque responses and bodies. */
  networkErrors?: boolean
}
export type WebMonitoring = {
  /** Report host-defined route readiness. This is not document LCP or INP. */
  routeReady: (route: string, startedAt: number) => void
  /** Stop reporting and transport. web-vitals retains its bounded document lifecycle observers. */
  stop: () => void
  flush: () => Promise<void>
}
const inert: WebMonitoring = { routeReady() {}, stop() {}, async flush() {} }
let controller: WebMonitoring | undefined

const start = safeFn('startWebMonitoring', (options: WebMonitoringOptions): WebMonitoring => {
  if (controller) return controller
  const config = getConfig()
  if (!config?.enabled || !config.app?.identifier || typeof window === 'undefined'
    || !Number.isFinite(options?.sampleRate) || options.sampleRate <= 0 || options.sampleRate > 1) return inert
  const identifier = config.app.identifier
  let key: string | undefined
  const startup: Array<() => void> = []
  let startupTimer: ReturnType<typeof setTimeout> | undefined
  const startupTick = safeFn('rum.startup', () => {
    try { if (active) startup.shift()?.() }
    finally { startupTimer = active && startup.length ? setTimeout(startupTick, 0) : undefined }
  })
  const defer = (run: () => void) => {
    startup.push(run)
    if (startupTimer === undefined) startupTimer = setTimeout(startupTick, 0)
  }
  let active = true, sending = false, selected = false, view: Visit | undefined
  let pending: Visit[] = [], nextAttempt = 0, failures = 0
  let resources: PerformanceObserver | undefined, tasks: PerformanceObserver | undefined
  let reportedResources = 0
  let request: AbortController | undefined
  const available = new Set(typeof PerformanceObserver === 'undefined' ? [] : PerformanceObserver.supportedEntryTypes)
  const browser = /Firefox\//.test(navigator.userAgent) ? 'Firefox' : /Edg\//.test(navigator.userAgent) ? 'Edge' : /Chrome\//.test(navigator.userAgent) ? 'Chrome' : /Safari\//.test(navigator.userAgent) ? 'Safari' : 'Other'
  function route() { try { return normalizeRoute(options.route?.() ?? location.href, location.origin) } catch { return '/' } }
  function create(navigationType: Visit['navigationType'], path = route()): Visit {
    return {
      id: uuidV7(), revision: 0, startedAt: new Date().toISOString(), appIdentifier: identifier,
      release: config!.release, environment: config!.environment, route: path,
      deviceClass: /Android|Mobi|iPhone|iPad|iPod/i.test(navigator.userAgent) || (/Macintosh/.test(navigator.userAgent) && navigator.maxTouchPoints > 1) ? 'mobile' : 'desktop', browser,
      navigationType, sampleRate: options.sampleRate, metrics: {},
      missing: navigationType === 'spa' ? {} : {
        LCP: available.has('largest-contentful-paint') ? 'not-observed' : 'unsupported',
        INP: available.has('event') ? 'no-interaction-or-not-yet-reported' : 'unsupported',
        CLS: available.has('layout-shift') ? 'not-observed' : 'unsupported',
        FCP: available.has('paint') ? 'not-observed' : 'unsupported',
        TTFB: available.has('navigation') ? 'not-observed' : 'unsupported',
      },
      errorCount: 0, resourceErrorCount: 0,
      diagnostics: { resources: [], longTasks: { count: 0, totalMs: 0, maxMs: 0 } },
    }
  }
  function save() {
    if (!active || !key) return
    try { localStorage.setItem(key, JSON.stringify({ at: Date.now(), visits: pending })) } catch { /* Storage is optional. */ }
  }
  const snapshot = safeFn('rum.snapshot', () => {
    if (!active || !getConfig()?.enabled || !selected || !view) return
    view.revision++
    pending = boundedVisits(pending, structuredClone(view))
  })
  let captureQueued = false
  const capture = () => {
    if (captureQueued || !active || !selected) return
    captureQueued = true
    defer(() => { captureQueued = false; snapshot() })
  }
  function begin(kind: Visit['navigationType']) {
    snapshot()
    selected = Math.random() < options.sampleRate
    view = selected ? create(kind) : undefined
    if (view && kind === 'document') {
      const n = performance.getEntriesByType('navigation')[0] as PerformanceNavigationTiming | undefined
      if (n) view.diagnostics.navigation = {
        dnsMs: Math.max(0, n.domainLookupEnd - n.domainLookupStart),
        connectMs: Math.max(0, n.connectEnd - n.connectStart),
        tlsMs: n.secureConnectionStart > 0 ? Math.max(0, n.connectEnd - n.secureConnectionStart) : null,
        requestMs: Math.max(0, n.responseStart - n.requestStart),
        responseMs: Math.max(0, n.responseEnd - n.responseStart),
      }
    }
    patchContext({ webVisit: view ? { id: view.id, route: view.route, device: view.deviceClass } : undefined })
    capture()
  }
  // Persistence is optional and scoped to this token as well as app/origin.
  // Hash off-thread; never put the credential in a storage key or block startup on storage.
  defer(() => {
    try {
      void crypto.subtle.digest('SHA-256', new TextEncoder().encode(config.token)).then(safeFn('rum.restoreQueue', digest => {
        key = '@sentori/rum/' + identifier + '/' + config.ingestUrl + '/' + Array.from(new Uint8Array(digest), b => b.toString(16).padStart(2, '0')).join('')
        try {
          if (!active) { localStorage.removeItem(key); return }
          const raw = localStorage.getItem(key)
          if (!raw || raw.length > 64_000) return
          const saved = JSON.parse(raw) as { at?: number; visits?: Visit[] }
          if (saved?.at && Date.now() - saved.at >= 0 && Date.now() - saved.at < 86_400_000 && Array.isArray(saved.visits)) {
            for (const v of saved.visits.slice(-20)) {
              if (v?.appIdentifier === identifier && typeof v.id === 'string' && v.id.length === 36 && Number.isInteger(v.revision)) pending = boundedVisits(pending, v)
            }
          }
        } catch { /* Broken or blocked storage is not an SDK fault. */ }
        finally { save() }
      })).catch(() => {})
    } catch { /* Unsupported crypto or blocked storage disables persistence only. */ }
  })
  begin('document')
  const metric = safeFn('rum.metric', (m: Metric) => {
    if (!active || !selected || !view || m.navigationType === 'soft-navigation' || !Number.isFinite(m.value) || m.value < 0) return
    view.metrics[m.name] = { id: m.id.slice(0, 100), value: m.value, unit: m.name === 'CLS' ? 'score' : 'ms' }
    delete view.missing[m.name]
    capture()
  })
  // Register bfcache handling first so each restored document has a fresh denominator.
  const restored = safeFn('rum.restore', (event: PageTransitionEvent) => { if (active && event.persisted) begin('bfcache') })
  window.addEventListener('pageshow', restored)
  for (const install of [onCLS, onFCP, onINP, onLCP, onTTFB]) {
    defer(() => { try { install(metric, { reportAllChanges: true }) } catch { /* Unsupported APIs are represented in missing. */ } })
  }
  const unsubscribe = registerEmitHook(event => {
    if (!active || !selected || !view || event.kind !== 'error') return
    view.errorCount = Math.min(1000, view.errorCount + 1)
    capture()
  })
  const resourceError = safeFn('rum.resourceError', (event: Event) => {
    if (!active || !options.resourceErrors || !getConfig()?.enabled) return
    const target = event.target
    if (!(target instanceof HTMLScriptElement || target instanceof HTMLLinkElement || target instanceof HTMLImageElement)) return
    const value = target instanceof HTMLLinkElement ? target.href : target.src
    if (!value || value.startsWith(config!.ingestUrl)) return
    if (view) view.resourceErrorCount = Math.min(1000, view.resourceErrorCount + 1)
    // Bounded per page; resource failures remain distinct from JavaScript exceptions.
    if (++reportedResources <= 10) error(new Error('Resource failed to load'), { source: 'resource', resourceType: target.tagName.toLowerCase(), route: normalizeRoute(value, location.origin) })
  })
  window.addEventListener('error', resourceError, true)
  let stopNetwork: (() => void) | undefined
  try { stopNetwork = options.networkErrors ? installNetworkErrors(config.ingestUrl, (kind, path, status) => {
    if (active && getConfig()?.enabled) error(new Error('Network request failed'), { source: 'network', kind, route: path, status })
  }) : undefined } catch { /* Optional instrumentation must not prevent monitoring. */ }
  defer(() => { try {
    resources = new PerformanceObserver(safeFn('rum.resources', list => {
      if (!active || !selected || !view) return
      for (const raw of list.getEntries()) {
        const entry = raw as PerformanceResourceTiming
        if (entry.name.startsWith(config!.ingestUrl) || !/^https?:/.test(entry.name)) continue
        const summary = resourceSummary(entry, location.origin)
        const rows = view.diagnostics.resources
        rows.push(summary); rows.sort((a, b) => b.durationMs - a.durationMs); rows.length = Math.min(rows.length, 10)
      }
    }))
    resources.observe({ type: 'resource', buffered: true })
  } catch { /* Optional resource timing. */ } })
  defer(() => { try {
    tasks = new PerformanceObserver(safeFn('rum.longTasks', list => {
      if (!active || !selected || !view) return
      for (const e of list.getEntries()) {
        const t = view.diagnostics.longTasks
        t.count = Math.min(1_000_000, t.count + 1); t.totalMs = Math.min(86_400_000, t.totalMs + e.duration); t.maxMs = Math.max(t.maxMs, e.duration)
      }
    }))
    tasks.observe({ type: 'longtask', buffered: true })
  } catch { /* Long tasks are not supported in every browser. */ } })
  async function flush(final = false) {
    try {
      if (!active || !getConfig()?.enabled || sending) return
      snapshot(); save()
      if (Date.now() < nextAttempt && (!final || failures > 0)) return
      if (!pending.length) return
      sending = true
      const batch = pending.slice(-20)
      request = new AbortController()
      const timeout = setTimeout(() => request?.abort(), 5000)
      const response = await fetch(config!.ingestUrl + '/v1/web/visits', {
        method: 'POST', headers: { Authorization: 'Bearer ' + config!.token, 'Content-Type': 'application/json', 'Sentori-Sdk': 'web/' + SDK_VERSION },
        body: JSON.stringify({ visits: batch }), keepalive: true, credentials: 'omit', redirect: 'error', signal: request.signal,
      }).finally(() => clearTimeout(timeout))
      if (!active) return
      if (response.ok || [400, 401, 403, 404, 413].includes(response.status)) {
        pending = pending.filter(v => !batch.some(sent => sent.id === v.id && sent.revision === v.revision))
        failures = 0
      } else failures++
      if (response.status === 403) { controller?.stop(); return }
      nextAttempt = Date.now() + Math.min(300_000, 30_000 * 2 ** Math.min(failures, 4))
      save()
    } catch { failures++; nextAttempt = Date.now() + Math.min(300_000, 30_000 * 2 ** Math.min(failures, 4)); save() }
    finally { sending = false }
  }
  // web-vitals finalizes on hidden. Defer our flush until those callbacks finish.
  const hidden = safeFn('rum.hidden', () => { if (document.visibilityState === 'hidden') queueMicrotask(() => { void flush(true) }) })
  const pagehide = safeFn('rum.pagehide', () => { queueMicrotask(() => { void flush(true) }) })
  document.addEventListener('visibilitychange', hidden)
  window.addEventListener('pagehide', pagehide)
  const interval = setInterval(() => { void flush() }, 30_000)
  controller = {
    flush,
    routeReady: safeFn('rum.routeReady', (path, startedAt) => {
      if (!active || !selected || !getConfig()?.enabled || !Number.isFinite(startedAt) || startedAt < 0 || startedAt > performance.now()) return
      const row = create('spa', normalizeRoute(path, location.origin))
      row.revision = 1
      row.metrics.routeReady = { id: row.id, value: performance.now() - startedAt, unit: 'ms' }
      pending = boundedVisits(pending, row)
      patchContext({ webVisit: { id: view?.id, route: row.route, device: row.deviceClass } })
    }),
    stop: safeFn('rum.stop', () => {
      active = false; selected = false; view = undefined; pending = []
      stopNetwork?.(); unsubscribe(); patchContext({ webVisit: undefined })
      clearInterval(interval); if (startupTimer !== undefined) clearTimeout(startupTimer); startup.length = 0
      request?.abort(); resources?.disconnect(); tasks?.disconnect()
      window.removeEventListener('pageshow', restored); window.removeEventListener('pagehide', pagehide)
      window.removeEventListener('error', resourceError, true); document.removeEventListener('visibilitychange', hidden)
      try { if (key) localStorage.removeItem(key) } catch { /* Optional storage. */ }
    }),
  }
  return controller
})

/** Always return a usable controller, including when optional setup is unavailable. */
export function startWebMonitoring(options: WebMonitoringOptions): WebMonitoring {
  return start(options) ?? inert
}
