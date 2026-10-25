// `sentori-cli issue list / resolve` — CI triage over the AI
// surface (`/api/*`, api-scope token; the same loop an agent runs).
async function apiFetch(c, path, init) {
    const resp = await fetch(`${c.apiUrl.replace(/\/+$/, '')}${path}`, {
        ...init,
        headers: {
            Authorization: `Bearer ${c.token}`,
            'Content-Type': 'application/json',
            ...(init?.headers ?? {}),
        },
    });
    if (!resp.ok) {
        const detail = await resp.text().catch(() => '');
        throw new Error(`${resp.status} ${resp.statusText}${detail ? ` — ${detail.slice(0, 300)}` : ''}`);
    }
    return (await resp.json());
}
export async function listIssues(c, opts = {}) {
    return (await listIssuesPage(c, opts)).issues;
}
export async function listIssuesPage(c, opts = {}) {
    if (opts.limit !== undefined && (!Number.isSafeInteger(opts.limit) || opts.limit < 1 || opts.limit > 200))
        throw new Error('--limit must be an integer from 1 to 200');
    if (opts.offset !== undefined && (!Number.isSafeInteger(opts.offset) || opts.offset < 0))
        throw new Error('--offset must be a non-negative safe integer');
    const q = new URLSearchParams();
    if (opts.status)
        q.set('status', opts.status);
    if (opts.kind)
        q.set('kind', opts.kind);
    if (opts.app)
        q.set('app_id', opts.app);
    if (opts.platform)
        q.set('platform', opts.platform);
    if (opts.environment)
        q.set('environment', opts.environment);
    if (opts.release)
        q.set('release', opts.release);
    if (opts.limit !== undefined)
        q.set('limit', String(opts.limit));
    if (opts.offset !== undefined)
        q.set('offset', String(opts.offset));
    const qs = q.toString();
    const body = await apiFetch(c, `/api/issues${qs ? `?${qs}` : ''}`);
    return body;
}
export async function resolveIssue(c, issueId, release) {
    await apiFetch(c, `/api/issues/${encodeURIComponent(issueId)}/resolve`, {
        method: 'POST',
        body: JSON.stringify(release ? { release } : {}),
    });
}
export async function noteIssue(c, issueId, body) {
    await apiFetch(c, `/api/issues/${encodeURIComponent(issueId)}/notes`, {
        method: 'POST',
        body: JSON.stringify({ body }),
    });
}
export async function fetchBundle(c, issueId) {
    const resp = await fetch(`${c.apiUrl.replace(/\/+$/, '')}/api/issues/${encodeURIComponent(issueId)}/bundle`, { headers: { Authorization: `Bearer ${c.token}` } });
    if (!resp.ok)
        throw new Error(`${resp.status} ${resp.statusText}`);
    return resp.text();
}
export function formatIssueLine(i) {
    const flag = i.regressed ? ' REGRESSED' : '';
    return `${i.id}  [${i.kind}] ${i.title}  ${i.usersCount}u×${i.maxPerUser}  ${i.eventCount}ev${flag}`;
}
//# sourceMappingURL=issue.js.map