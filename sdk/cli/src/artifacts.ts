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

export type ArtifactRow = {
  kind: string
  name: string
  /** The 32-hex debug id, for dSYM slices. `null` for map files. */
  debugId: null | string
  contentHash: string
  sizeBytes: number
  createdAt: string
  /** Did the server parse it? `null` on artifacts uploaded before
   *  the check existed — "never looked at", not "looked at and
   *  fine". `false` means it stored and will symbolicate nothing. */
  usable?: boolean | null
}

export type ArtifactsResponse = {
  release: string
  /** False when the server has never heard this release name at all
   *  — a typo in `--release` and an un-uploaded release look the
   *  same otherwise, and only one of them is fixed by uploading. */
  known: boolean
  kinds: Record<string, number>
  missing: string[]
  artifacts: ArtifactRow[]
}

export async function fetchArtifacts(opts: {
  apiUrl: string
  release: string
  token: string
}): Promise<ArtifactsResponse> {
  const url =
    `${opts.apiUrl.replace(/\/+$/, '')}/v1/releases/` +
    `${encodeURIComponent(opts.release)}/artifacts`
  const resp = await fetch(url, {
    headers: { Authorization: `Bearer ${opts.token}` },
  })
  if (!resp.ok) {
    const detail = await resp.text().catch(() => '')
    throw new Error(
      `${resp.status} ${resp.statusText}${detail ? ` — ${detail.slice(0, 200)}` : ''}`,
    )
  }
  return (await resp.json()) as ArtifactsResponse
}
