import { expect, test } from 'bun:test'
import { listIssues } from '../issue.js'

test('CLI forwards pagination and filters and preserves page metadata', async () => {
  const page = { issues: [{ id: 'second' }], total: 3, limit: 1, offset: 1, nextOffset: 2 }
  const server = Bun.serve({ port: 0, fetch(request) {
    const url = new URL(request.url)
    expect(url.pathname).toBe('/api/issues')
    expect(Object.fromEntries(url.searchParams)).toEqual({ status: 'open', app_id: 'app', environment: 'development', limit: '1', offset: '1' })
    return Response.json(page)
  } })
  try {
    const child = Bun.spawn([process.execPath, new URL('../index.ts', import.meta.url).pathname,
      'issue', 'list', '--json', '--limit', '1', '--offset', '1', '--app', 'app', '--environment', 'development', '--api-url', server.url.toString()], {
      env: { ...process.env, SENTORI_API_TOKEN: 'test-token' }, stdout: 'pipe', stderr: 'pipe',
    })
    const output = await new Response(child.stdout).text()
    expect(await child.exited).toBe(0)
    expect(JSON.parse(output)).toEqual(page)
  } finally { await server.stop(true) }
})

test('invalid pagination is rejected before making a request', async () => {
  const config = { apiUrl: 'http://127.0.0.1:1', token: 'test' }
  for (const limit of [0, -1, 201, 1.5, NaN]) await expect(listIssues(config, { limit })).rejects.toThrow('--limit')
  for (const offset of [-1, 1.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1]) await expect(listIssues(config, { offset })).rejects.toThrow('--offset')
})
