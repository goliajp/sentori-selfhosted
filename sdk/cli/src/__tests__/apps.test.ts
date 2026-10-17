import { expect, test } from 'bun:test'
import { listApps, showProject } from '../apps.js'

test('project and app discovery use the token scope and surface API failures', async () => {
  const requests: string[] = []
  const server = Bun.serve({
    port: 0,
    fetch(request) {
      expect(request.headers.get('Authorization')).toBe('Bearer test-api-token')
      const path = new URL(request.url).pathname
      requests.push(path)
      if (path === '/api/project') return Response.json({ project: { id: 'project-a', name: 'insight-dev-server' } })
      return Response.json({ error: 'unavailable' }, { status: 503 })
    },
  })
  try {
    const config = { apiUrl: server.url.toString(), token: 'test-api-token' }
    expect((await showProject(config)).project.name).toBe('insight-dev-server')
    await expect(listApps(config)).rejects.toThrow('503')
    expect(requests).toEqual(['/api/project', '/api/apps'])
  } finally { await server.stop(true) }
})

test('discovery does not follow redirects to another service', async () => {
  let targetRequests = 0
  const target = Bun.serve({ port: 0, fetch() { targetRequests++; return Response.json({ apps: [] }) } })
  const server = Bun.serve({ port: 0, fetch() { return Response.redirect(target.url.toString()) } })
  try {
    await expect(listApps({ apiUrl: server.url.toString(), token: 'private-token' })).rejects.toThrow()
    expect(targetRequests).toBe(0)
  } finally { await server.stop(true); await target.stop(true) }
})

test('CLI app list uses SENTORI_API_TOKEN and preserves JSON app identity', async () => {
  const body = { projectId: 'project-a', apps: [{ id: 'api-app', kind: 'webapi', platform: 'javascript', identifier: 'api' }] }
  const server = Bun.serve({ port: 0, fetch(request) {
    if (request.headers.get('Authorization') !== 'Bearer scoped-token') return new Response('', { status: 401 })
    return Response.json(body)
  } })
  try {
    const child = Bun.spawn([process.execPath, new URL('../index.ts', import.meta.url).pathname, 'app', 'list', '--json', '--api-url', server.url.toString()], {
      env: { ...process.env, SENTORI_API_TOKEN: 'scoped-token' }, stdout: 'pipe', stderr: 'pipe',
    })
    const output = await new Response(child.stdout).text()
    expect(await child.exited).toBe(0)
    expect(JSON.parse(output)).toEqual(body)
  } finally { await server.stop(true) }
})
