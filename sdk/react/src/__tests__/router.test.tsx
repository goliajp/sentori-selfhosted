import {
  __resetTraceContextForTests,
  __useFallbackTraceContextForTests,
  clearSpans,
  drainSpans,
  setActiveSpan,
} from '@goliapkg/sentori-core'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterAll, afterEach, beforeEach, describe, expect, test } from 'bun:test'
import { Link, MemoryRouter, Route, Routes } from 'react-router'

import { clearBreadcrumbs, getBreadcrumbs } from '@goliapkg/sentori-javascript'

import { useSentoriRouter } from '../router.js'
import { SentoriProvider } from '../SentoriProvider.js'

const PROVIDER_PROPS = {
  config: {
    environment: 'test',
    ingestUrl: 'http://localhost:0',
    release: 'test@0.0.0',
    token: 'st_pk_testtesttesttesttesttesttest',
  },
}

function Shell() {
  useSentoriRouter()
  return (
    <>
      <Link to="/orders">orders</Link>
      <Link to="/billing">billing</Link>
      <Routes>
        <Route element={<div>home</div>} path="/" />
        <Route element={<div>orders-page</div>} path="/orders" />
        <Route element={<div>billing-page</div>} path="/billing" />
      </Routes>
    </>
  )
}

const navSpans = () => drainSpans().filter((s) => s.op === 'react.navigation')

describe('useSentoriRouter', () => {
  beforeEach(() => {
    __useFallbackTraceContextForTests() // see navigation.test.ts
    clearBreadcrumbs()
    clearSpans()
    setActiveSpan(null)
  })
  afterEach(() => {
    cleanup()
    clearBreadcrumbs()
    clearSpans()
    setActiveSpan(null)
  })
  afterAll(() => {
    __resetTraceContextForTests()
  })

  test('initial mount does NOT emit a nav breadcrumb', () => {
    render(
      <SentoriProvider {...PROVIDER_PROPS}>
        <MemoryRouter initialEntries={['/']}>
          <Shell />
        </MemoryRouter>
      </SentoriProvider>,
    )
    expect(screen.getByText('home')).toBeDefined()
    expect(getBreadcrumbs().filter((b) => b.type === 'nav')).toHaveLength(0)
  })

  test('navigation emits a nav breadcrumb with from/to', () => {
    render(
      <SentoriProvider {...PROVIDER_PROPS}>
        <MemoryRouter initialEntries={['/']}>
          <Shell />
        </MemoryRouter>
      </SentoriProvider>,
    )

    fireEvent.click(screen.getByText('orders'))
    expect(screen.getByText('orders-page')).toBeDefined()

    const navs = getBreadcrumbs().filter((b) => b.type === 'nav')
    expect(navs).toHaveLength(1)
    expect(navs[0]?.data).toEqual({ from: '/', to: '/orders' })

    fireEvent.click(screen.getByText('billing'))
    expect(screen.getByText('billing-page')).toBeDefined()

    const navsAfter = getBreadcrumbs().filter((b) => b.type === 'nav')
    expect(navsAfter).toHaveLength(2)
    expect(navsAfter[1]?.data).toEqual({ from: '/orders', to: '/billing' })
  })

  test('opens a react.navigation span per route (initial + each transition)', () => {
    render(
      <SentoriProvider {...PROVIDER_PROPS}>
        <MemoryRouter initialEntries={['/']}>
          <Shell />
        </MemoryRouter>
      </SentoriProvider>,
    )
    fireEvent.click(screen.getByText('orders'))
    fireEvent.click(screen.getByText('billing'))
    cleanup() // unmount → finishes the last open span

    const spans = navSpans()
    expect(spans.map((s) => s.name)).toEqual(['/', '/ → /orders', '/orders → /billing'])
    // each route is its own trace root
    expect(spans.every((s) => s.parentSpanId === null)).toBe(true)
    expect(new Set(spans.map((s) => s.traceId)).size).toBe(3)
    expect(spans[1]?.tags).toEqual({ 'nav.from': '/', 'nav.to': '/orders' })
  })
})
