// `sentori-cli issue list / resolve` — CI triage over the AI
// surface (`/api/*`, api-scope token; the same loop an agent runs).

type Issue = {
  id: string
  kind: string
  title: string
  messageSample: string
  status: string
  eventCount: number
  usersCount: number
  maxPerUser: number
  lastSeen: string
  regressed: boolean
}

export type ApiConfig = {
  apiUrl: string
  token: string
}

async function apiFetch<T>(c: ApiConfig, path: string, init?: RequestInit): Promise<T> {
  const resp = await fetch(`${c.apiUrl.replace(/\/+$/, '')}${path}`, {
    ...init,
    headers: {
      Authorization: `Bearer ${c.token}`,
      'Content-Type': 'application/json',
      ...(init?.headers ?? {}),
    },
  })
  if (!resp.ok) {
    const detail = await resp.text().catch(() => '')
    throw new Error(
      `${resp.status} ${resp.statusText}${detail ? ` — ${detail.slice(0, 300)}` : ''}`,
    )
  }
  return (await resp.json()) as T
}

export async function listIssues(
  c: ApiConfig,
  opts: { status?: string; kind?: string } = {},
): Promise<Issue[]> {
  const q = new URLSearchParams()
  if (opts.status) q.set('status', opts.status)
  if (opts.kind) q.set('kind', opts.kind)
  const qs = q.toString()
  const body = await apiFetch<{ issues: Issue[] }>(c, `/api/issues${qs ? `?${qs}` : ''}`)
  return body.issues
}

export async function resolveIssue(
  c: ApiConfig,
  issueId: string,
  release?: string,
): Promise<void> {
  await apiFetch(c, `/api/issues/${encodeURIComponent(issueId)}/resolve`, {
    method: 'POST',
    body: JSON.stringify(release ? { release } : {}),
  })
}

export async function noteIssue(c: ApiConfig, issueId: string, body: string): Promise<void> {
  await apiFetch(c, `/api/issues/${encodeURIComponent(issueId)}/notes`, {
    method: 'POST',
    body: JSON.stringify({ body }),
  })
}

export async function fetchBundle(c: ApiConfig, issueId: string): Promise<string> {
  const resp = await fetch(
    `${c.apiUrl.replace(/\/+$/, '')}/api/issues/${encodeURIComponent(issueId)}/bundle`,
    { headers: { Authorization: `Bearer ${c.token}` } },
  )
  if (!resp.ok) throw new Error(`${resp.status} ${resp.statusText}`)
  return resp.text()
}

export function formatIssueLine(i: Issue): string {
  const flag = i.regressed ? ' REGRESSED' : ''
  return `${i.id}  [${i.kind}] ${i.title}  ${i.usersCount}u×${i.maxPerUser}  ${i.eventCount}ev${flag}`
}
