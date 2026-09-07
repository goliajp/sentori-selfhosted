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
    const q = new URLSearchParams();
    if (opts.status)
        q.set('status', opts.status);
    if (opts.kind)
        q.set('kind', opts.kind);
    const qs = q.toString();
    const body = await apiFetch(c, `/api/issues${qs ? `?${qs}` : ''}`);
    return body.issues;
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