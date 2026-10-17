// Lightweight diagnostic signals. Standard Web Vitals are available from the optional /performance entry point.
// Diagnostic observations are bounded by the signal ring. layoutShiftTotal and
// worstInteraction are approximations, not standard CLS or INP measurements.

import { pushSignal, reportInternal } from '@goliapkg/sentori-core'

const observers: PerformanceObserver[] = []

type LayoutShift = PerformanceEntry & { value: number; hadRecentInput: boolean }
type EventTiming = PerformanceEntry & { interactionId?: number; duration: number }

const observe = (init: PerformanceObserverInit, fn: (list: PerformanceObserverEntryList) => void): void => {
  try {
    const o = new PerformanceObserver(fn)
    o.observe(init)
    observers.push(o)
  } catch (err) {
    // An engine without this entry type. Not a fault worth reporting
    // — it is a browser being older, not the SDK breaking.
    void err
  }
}

export const installWebVitals = (): void => {
  if (typeof PerformanceObserver === 'undefined') return

  // LCP: the last one wins. The browser keeps revising its candidate
  // as the page loads, so pushing every candidate would fill the ring
  // with the same metric.
  let lastLcp = 0
  observe({ type: 'largest-contentful-paint', buffered: true }, (list) => {
    try {
      const entries = list.getEntries()
      const last = entries[entries.length - 1]
      if (!last) return
      const ms = Math.round(last.startTime)
      if (ms === lastLcp) return
      lastLcp = ms
      pushSignal('vital', { name: 'LCP', ms })
    } catch (err) {
      reportInternal('vital-lcp', err)
    }
  })

  // CLS: a running sum, pushed only when it crosses the thresholds a
  // reader acts on. Every shift would be hundreds of entries on a
  // busy page, and the ring is sixty seconds of context, not a
  // timeline.
  let cls = 0
  let reported = 0
  observe({ type: 'layout-shift', buffered: true }, (list) => {
    try {
      for (const raw of list.getEntries()) {
        const e = raw as LayoutShift
        // A shift right after a real interaction is the page
        // responding, not the page misbehaving.
        if (e.hadRecentInput) continue
        cls += e.value
      }
      // 0.1 and 0.25 are the "needs improvement" and "poor" lines
      // every Web Vitals report is drawn against.
      for (const line of [0.1, 0.25]) {
        if (cls >= line && reported < line) {
          reported = line
          pushSignal('vital', { name: 'layoutShiftTotal', value: Math.round(cls * 1000) / 1000 })
        }
      }
    } catch (err) {
      reportInternal('vital-cls', err)
    }
  })

  // INP, approximated by the worst interaction seen. The official
  // metric is a high percentile over the whole visit, which needs the
  // page to end; the slowest one is what a person actually noticed,
  // and it is available while the page is still open.
  let worst = 0
  observe({ type: 'event', buffered: true, durationThreshold: 40 } as PerformanceObserverInit, (list) => {
    try {
      for (const raw of list.getEntries()) {
        const e = raw as EventTiming
        if (!e.interactionId) continue
        const ms = Math.round(e.duration)
        if (ms <= worst) continue
        worst = ms
        pushSignal('vital', { name: 'worstInteraction', ms, event: e.name })
      }
    } catch (err) {
      reportInternal('vital-inp', err)
    }
  })
}

export const uninstallWebVitals = (): void => {
  for (const o of observers) {
    try {
      o.disconnect()
    } catch {
      // Already gone with the page.
    }
  }
  observers.length = 0
}
