// `sentori-cli issue list / resolve / silence` — CI triage helpers.

type Issue = {
  errorType: string
  eventCount: number
  id: string
  lastSeen: string
  messageSample: string
  status: 'active' | 'closed' | 'resolved' | 'silenced'
}

type AdminConfig = {
  apiUrl: string
  projectId: string
  token: string
}

function url(c: AdminConfig, path: string): string {
  return `${c.apiUrl.replace(/\/+$/, '')}/admin/api/projects/${c.projectId}${path}`
}

async function adminFetch<T>(c: AdminConfig, path: string, init?: RequestInit): Promise<T> {
  const resp = await fetch(url(c, path), {
    ...init,
    headers: {
      Authorization: `Bearer ${c.token}`,
      'Content-Type': 'application/json',
      ...(init?.headers ?? {}),
    },
  })
  if (!resp.ok) {
    let detail = ''
    try {
      detail = await resp.text()
    } catch {
      // ignore
    }
    throw new Error(
      `${resp.status} ${resp.statusText}${detail ? ` — ${detail.slice(0, 300)}` : ''}`,
    )
  }
  // PATCH /issues/<id> returns the row; some endpoints might return no
  // content — handle both.
  const txt = await resp.text()
  return (txt ? JSON.parse(txt) : null) as T
}

export type IssueListOptions = {
  config: AdminConfig
  errorType?: string
  limit?: number
  status?: 'active' | 'closed' | 'resolved' | 'silenced'
}

export async function issueList(opts: IssueListOptions): Promise<Issue[]> {
  const q = new URLSearchParams()
  if (opts.status) q.set('status', opts.status)
  if (opts.limit) q.set('limit', String(opts.limit))
  if (opts.errorType) q.set('errorType', opts.errorType)
  const qs = q.toString()
  return adminFetch<Issue[]>(opts.config, `/issues${qs ? '?' + qs : ''}`)
}

export async function issuePatch(
  config: AdminConfig,
  issueId: string,
  body: { resolvedInRelease?: string; status: 'active' | 'closed' | 'resolved' | 'silenced' },
): Promise<Issue> {
  return adminFetch<Issue>(config, `/issues/${encodeURIComponent(issueId)}`, {
    body: JSON.stringify(body),
    method: 'PATCH',
  })
}

/** Format one issue for terminal output — short, one line, scannable. */
export function formatIssueLine(i: Issue): string {
  const status = i.status.padEnd(9)
  const title = `${i.errorType}${i.messageSample ? `: ${i.messageSample}` : ''}`
  const events = `${i.eventCount}×`
  return `${i.id}  ${status}  ${title.slice(0, 80).padEnd(80)}  ${events}`
}
