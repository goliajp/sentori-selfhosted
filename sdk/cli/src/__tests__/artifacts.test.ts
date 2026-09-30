// The read half of the artifact API — the one the release gate asks.
//
// These pin the wire contract rather than the printing: what the gate
// keys on is `kinds` (a count per kind, zeros present) and `known`
// (has the server ever heard this release name). A response missing
// either turns the gate into a coin flip that reads as green.

import { describe, expect, test } from 'bun:test'

import { fetchArtifacts, type ArtifactsResponse } from '../artifacts.js'

function serve(status: number, body: unknown): () => void {
  const real = globalThis.fetch
  globalThis.fetch = (async (url: RequestInfo | URL) => {
    seen.push(String(url))
    return new Response(JSON.stringify(body), {
      status,
      headers: { 'content-type': 'application/json' },
    })
  }) as typeof fetch
  return () => {
    globalThis.fetch = real
  }
}
const seen: string[] = []

const full: ArtifactsResponse = {
  release: 'app@1.0.0+7',
  known: true,
  kinds: { sourcemap: 1, dsym: 2, proguard: 0, srcbundle: 0 },
  missing: ['proguard', 'srcbundle'],
  artifacts: [
    {
      kind: 'dsym',
      name: 'Insight.app-arm64-E63A748C-3F0E-302D-95EC-8DA5B55C97D9',
      debugId: 'E63A748C3F0E302D95EC8DA5B55C97D9',
      contentHash: 'aa',
      sizeBytes: 4096,
      createdAt: '2026-08-09T00:00:00Z',
    },
  ],
}

describe('artifacts read', () => {
  test('a kind with nothing uploaded is a zero, not an absent key', async () => {
    const restore = serve(200, full)
    try {
      const res = await fetchArtifacts({
        apiUrl: 'https://x.test',
        release: 'app@1.0.0+7',
        token: 't',
      })
      // `?? 0` in a gate would turn a broken response into "fine".
      expect(res.kinds.proguard).toBe(0)
      expect(Object.keys(res.kinds).sort()).toEqual([
        'dsym',
        'proguard',
        'sourcemap',
        'srcbundle',
      ])
    } finally {
      restore()
    }
  })

  test('the release name is escaped — @ and + are ordinary in one', async () => {
    seen.length = 0
    const restore = serve(200, full)
    try {
      await fetchArtifacts({
        apiUrl: 'https://x.test/',
        release: 'focus-ai-app@5.4.26080601+382',
        token: 't',
      })
      // A bare + in a path is not a space, but a bare one in a query
      // would be; encode regardless so the round trip is the same
      // string the SDK reports.
      expect(seen[0]).toBe(
        'https://x.test/v1/releases/focus-ai-app%405.4.26080601%2B382/artifacts',
      )
    } finally {
      restore()
    }
  })

  test('a non-2xx carries the server detail, not just the status', async () => {
    const restore = serve(403, { error: 'admin_token_required' })
    try {
      await expect(
        fetchArtifacts({ apiUrl: 'https://x.test', release: 'r', token: 't' }),
      ).rejects.toThrow(/admin_token_required/)
    } finally {
      restore()
    }
  })
})
