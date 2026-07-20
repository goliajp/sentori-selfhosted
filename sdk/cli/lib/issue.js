// `sentori-cli issue list / resolve / silence` — CI triage helpers.
function url(c, path) {
    return `${c.apiUrl.replace(/\/+$/, '')}/admin/api/projects/${c.projectId}${path}`;
}
async function adminFetch(c, path, init) {
    const resp = await fetch(url(c, path), {
        ...init,
        headers: {
            Authorization: `Bearer ${c.token}`,
            'Content-Type': 'application/json',
            ...(init?.headers ?? {}),
        },
    });
    if (!resp.ok) {
        let detail = '';
        try {
            detail = await resp.text();
        }
        catch {
            // ignore
        }
        throw new Error(`${resp.status} ${resp.statusText}${detail ? ` — ${detail.slice(0, 300)}` : ''}`);
    }
    // PATCH /issues/<id> returns the row; some endpoints might return no
    // content — handle both.
    const txt = await resp.text();
    return (txt ? JSON.parse(txt) : null);
}
export async function issueList(opts) {
    const q = new URLSearchParams();
    if (opts.status)
        q.set('status', opts.status);
    if (opts.limit)
        q.set('limit', String(opts.limit));
    if (opts.errorType)
        q.set('errorType', opts.errorType);
    const qs = q.toString();
    return adminFetch(opts.config, `/issues${qs ? '?' + qs : ''}`);
}
export async function issuePatch(config, issueId, body) {
    return adminFetch(config, `/issues/${encodeURIComponent(issueId)}`, {
        body: JSON.stringify(body),
        method: 'PATCH',
    });
}
/** Format one issue for terminal output — short, one line, scannable. */
export function formatIssueLine(i) {
    const status = i.status.padEnd(9);
    const title = `${i.errorType}${i.messageSample ? `: ${i.messageSample}` : ''}`;
    const events = `${i.eventCount}×`;
    return `${i.id}  ${status}  ${title.slice(0, 80).padEnd(80)}  ${events}`;
}
//# sourceMappingURL=issue.js.map