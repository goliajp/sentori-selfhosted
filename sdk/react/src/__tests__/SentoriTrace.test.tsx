import { clearSpans, drainSpans } from '@goliapkg/sentori-core'
import { cleanup, render } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test } from 'bun:test'

import { TraceRender } from '../SentoriTrace.js'

beforeEach(() => clearSpans())
afterEach(() => {
  cleanup()
  clearSpans()
})

describe('TraceRender', () => {
  test('renders children', () => {
    const { getByText } = render(
      <TraceRender name="page">
        <span>hello</span>
      </TraceRender>,
    )
    expect(getByText('hello')).toBeDefined()
  })

  test('opens a react.render span on mount, finishes on unmount', () => {
    const { unmount } = render(
      <TraceRender name="OrdersTable">
        <div>orders</div>
      </TraceRender>,
    )

    // Span is open but not yet pushed to buffer.
    expect(drainSpans()).toHaveLength(0)

    unmount()

    const spans = drainSpans()
    expect(spans).toHaveLength(1)
    expect(spans[0]?.op).toBe('react.render')
    expect(spans[0]?.name).toBe('OrdersTable')
    expect(spans[0]?.status).toBe('ok')
    expect(spans[0]?.durationMs).toBeGreaterThanOrEqual(0)
  })

  test('custom op + tags + data flow through to finished span', () => {
    const { unmount } = render(
      <TraceRender
        data={{ rowCount: 42 }}
        name="dashboard mount"
        op="react.mount"
        tags={{ route: 'dashboard' }}
      >
        <div />
      </TraceRender>,
    )
    unmount()

    const sp = drainSpans()[0]!
    expect(sp.op).toBe('react.mount')
    expect(sp.name).toBe('dashboard mount')
    expect(sp.tags).toMatchObject({ route: 'dashboard' })
    expect(sp.data).toEqual({ rowCount: 42 })
  })

  test('name defaults to op when omitted', () => {
    const { unmount } = render(
      <TraceRender op="react.mount">
        <div />
      </TraceRender>,
    )
    unmount()

    expect(drainSpans()[0]?.name).toBe('react.mount')
  })

  test('multiple sequential mounts emit independent spans', () => {
    const first = render(<TraceRender name="a"><div /></TraceRender>)
    first.unmount()
    const second = render(<TraceRender name="b"><div /></TraceRender>)
    second.unmount()

    const spans = drainSpans()
    expect(spans).toHaveLength(2)
    expect(spans.map((s) => s.name).sort()).toEqual(['a', 'b'])
  })

  test('re-render with new props does NOT restart the span', () => {
    const { rerender, unmount } = render(
      <TraceRender name="first">
        <div />
      </TraceRender>,
    )
    rerender(
      <TraceRender name="second-name-ignored">
        <div />
      </TraceRender>,
    )
    unmount()

    const spans = drainSpans()
    expect(spans).toHaveLength(1)
    // First-render name wins (lifespan is the component instance).
    expect(spans[0]?.name).toBe('first')
  })
})
