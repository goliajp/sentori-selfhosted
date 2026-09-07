// Reading back what the uploads actually landed.
//
// Every `upload` command in this CLI exits 0 on failure by contract:
// Sentori must never be the reason a release does not ship. That
// contract has a cost, and insight paid it — their iOS dSYM step
// stopped being called during a pipeline refactor, and because
// nothing was allowed to go red, nobody noticed for a year. The three
// artifact lights in the dashboard showed it; no one was looking at
// the dashboard during a release.
//
// So the check moves into the pipeline, as a separate deliberate
// step: uploads stay lenient, and `artifacts check` is allowed to
// fail. Asking the server is the point — a local ledger of "we ran
// the upload" cannot tell you the upload landed, and it is exactly
// the "we ran it" assumption that was false.
//
// A gap found here is recoverable: from server 2.12.0 an upload
// re-reads the crashes already stored for that release, so the
// stacks that arrived while the artifact was missing become
// readable. It does not re-group them.
export async function fetchArtifacts(opts) {
    const url = `${opts.apiUrl.replace(/\/+$/, '')}/v1/releases/` +
        `${encodeURIComponent(opts.release)}/artifacts`;
    const resp = await fetch(url, {
        headers: { Authorization: `Bearer ${opts.token}` },
    });
    if (!resp.ok) {
        const detail = await resp.text().catch(() => '');
        throw new Error(`${resp.status} ${resp.statusText}${detail ? ` — ${detail.slice(0, 200)}` : ''}`);
    }
    return (await resp.json());
}
//# sourceMappingURL=artifacts.js.map