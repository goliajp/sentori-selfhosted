/** Bounded, non-identifying observations shared by the optional RUM collector. */
export type MetricName = 'LCP' | 'INP' | 'CLS' | 'FCP' | 'TTFB' | 'routeReady'
export type Measurement = { id: string; value: number; unit: 'ms' | 'score' }
export type Visit = {
  id: string; revision: number; startedAt: string; appIdentifier: string
  release: string; environment: string; route: string; deviceClass: 'mobile' | 'desktop'
  navigationType: 'document' | 'bfcache' | 'spa'; sampleRate: number
  metrics: Partial<Record<MetricName, Measurement>>; missing: Record<string, string>
  errorCount: number; resourceErrorCount: number; browser: string
  diagnostics: { navigation?: Record<string, number | null>; resources: Resource[]; longTasks: { count: number; totalMs: number; maxMs: number } }
}
export type Resource = { url: string; type: string; durationMs: number; transferBytes: number | null; timingAvailable: boolean }

/** Drop credentials, query and fragment; mask common IDs. Hosts can supply a route template. */
export function normalizeRoute(value: string, base = 'https://route.invalid'): string {
  try {
    const u = new URL(value, base)
    if (!['http:', 'https:'].includes(u.protocol)) return '/'
    return u.pathname.split('/').map(part => {
      if (/^\d+$/.test(part) || /^[a-f0-9-]{20,}$/i.test(part) || part.length > 64 || /@|%40/i.test(part)) return ':id'
      return part
    }).join('/').slice(0, 240) || '/'
  } catch { return '/' }
}

export function resourceSummary(entry: PerformanceResourceTiming, origin: string): Resource {
  const url = new URL(entry.name, origin)
  const available = url.origin === origin || entry.responseStart > 0
  return {
    url: url.origin + normalizeRoute(url.href), type: entry.initiatorType.slice(0, 30),
    durationMs: Math.max(0, Math.round(entry.duration)),
    transferBytes: available ? entry.transferSize : null, timingAvailable: available,
  }
}

/** Resends replace the same view; never accumulate metric revisions as extra visits. */
export function boundedVisits(visits: readonly Visit[], next: Visit): Visit[] {
  if (visits.some(v => v.id === next.id && v.revision > next.revision)) return visits.slice()
  const rows = visits.filter(v => v.id !== next.id).concat(next).slice(-20)
  while (rows.length && new TextEncoder().encode(JSON.stringify(rows)).byteLength > 48_000) rows.shift()
  return rows
}
