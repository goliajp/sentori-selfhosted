import { describe, expect, mock, test } from 'bun:test'

// Mock the JS SDK's captureError so we observe what onRequestError
// would have sent without spinning up a real transport.
const captured: { error: Error; tags: Record<string, string> }[] = []

mock.module('@goliapkg/sentori-javascript', () => ({
  captureError: (error: Error, extras: { tags: Record<string, string> }) => {
    captured.push({ error, tags: extras.tags })
  },
  // serverInit only uses initSentori; mock it to a noop.
  initSentori: () => {},
}))

const { onRequestError } = await import('../server.js')

describe('onRequestError', () => {
  test('captures Error subclasses', async () => {
    captured.length = 0
    await onRequestError(
      new TypeError('boom'),
      { method: 'GET', path: '/api/widgets' },
      { routePath: '/api/widgets', routeType: 'route', runtime: 'nodejs' },
    )
    expect(captured).toHaveLength(1)
    expect(captured[0]!.error.message).toBe('boom')
    expect(captured[0]!.tags).toMatchObject({
      'next.method': 'GET',
      'next.route': '/api/widgets',
      'next.runtime': 'nodejs',
      source: 'next.requestError',
    })
  })

  test('wraps non-Error throws into Error', async () => {
    captured.length = 0
    await onRequestError('string error', { method: 'POST' })
    expect(captured).toHaveLength(1)
    expect(captured[0]!.error.message).toBe('string error')
  })

  test('falls back to request.path / request.url when context.routePath is absent', async () => {
    captured.length = 0
    await onRequestError(new Error('x'), { method: 'GET', url: '/from-url' })
    expect(captured[0]!.tags['next.route']).toBe('/from-url')
  })
})
