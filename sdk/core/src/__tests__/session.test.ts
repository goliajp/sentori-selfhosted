import { describe, expect, it, vi } from 'vitest'

import { type SessionPing, SessionTracker } from '../session.js'

const ctx = { environment: 'prod', release: 'myapp@1.0.0', userId: 'u_abc' }

describe('SessionTracker', () => {
  it('start → end produces a single ping with positive duration', () => {
    const sent: SessionPing[] = []
    let now = 1_000
    const t = new SessionTracker((p) => sent.push(p), () => now)
    t.start(ctx)
    now = 5_500
    t.end()
    expect(sent.length).toBe(1)
    const p = sent[0]!
    expect(p.status).toBe('ok')
    expect(p.durationMs).toBe(4_500)
    expect(p.release).toBe('myapp@1.0.0')
    expect(p.userId).toBe('u_abc')
    expect(p.startedAt).toMatch(/^\d{4}-\d{2}-\d{2}T/)
  })

  it('markErrored upgrades ok → errored', () => {
    const sent: SessionPing[] = []
    const t = new SessionTracker((p) => sent.push(p), () => 0)
    t.start(ctx)
    t.markErrored()
    t.end()
    expect(sent[0]!.status).toBe('errored')
  })

  it('markCrashed wins over errored', () => {
    const sent: SessionPing[] = []
    const t = new SessionTracker((p) => sent.push(p), () => 0)
    t.start(ctx)
    t.markErrored()
    t.markCrashed()
    t.markErrored() // doesn't downgrade
    t.end()
    expect(sent[0]!.status).toBe('crashed')
  })

  it('end() does nothing when no session is active', () => {
    const send = vi.fn()
    const t = new SessionTracker(send, () => 0)
    t.end()
    expect(send).not.toHaveBeenCalled()
  })

  it('end() is idempotent — second call sends nothing', () => {
    const sent: SessionPing[] = []
    const t = new SessionTracker((p) => sent.push(p), () => 0)
    t.start(ctx)
    t.end()
    t.end()
    expect(sent.length).toBe(1)
  })

  it('explicit `exited` overrides ok but not crashed', () => {
    const sent: SessionPing[] = []
    let n = 0
    const t = new SessionTracker((p) => sent.push(p), () => n++)
    t.start(ctx)
    t.end('exited')
    expect(sent[0]!.status).toBe('exited')

    t.start(ctx)
    t.markCrashed()
    t.end('exited')
    expect(sent[1]!.status).toBe('crashed')
  })

  it('start while active drops the previous session without sending', () => {
    const sent: SessionPing[] = []
    const t = new SessionTracker((p) => sent.push(p), () => 0)
    t.start(ctx)
    const firstId = t.peek()!.id
    t.start(ctx)
    expect(sent.length).toBe(0)
    expect(t.peek()!.id).not.toBe(firstId)
  })
})
