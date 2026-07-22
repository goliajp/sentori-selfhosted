import { beforeEach, expect, test } from 'bun:test'

import {
  BreadcrumbBuffer,
  addBreadcrumb,
  clearBreadcrumbs,
  getBreadcrumbs,
} from '../breadcrumbs.js'

beforeEach(() => clearBreadcrumbs())

test('global ring: keeps last N', () => {
  for (let i = 0; i < 150; i++) addBreadcrumb('log', { i })
  const got = getBreadcrumbs()
  expect(got.length).toBe(100)
  expect((got[0]!.data as { i: number }).i).toBe(50)
  expect((got[99]!.data as { i: number }).i).toBe(149)
})

test('typed ring: respects custom cap', () => {
  const ring = new BreadcrumbBuffer(3)
  ring.push('log', { x: 1 })
  ring.push('log', { x: 2 })
  ring.push('log', { x: 3 })
  ring.push('log', { x: 4 })
  expect(ring.snapshot().map((b) => (b.data as { x: number }).x)).toEqual([2, 3, 4])
})

test('clear: empties the ring', () => {
  addBreadcrumb('log', {})
  clearBreadcrumbs()
  expect(getBreadcrumbs()).toEqual([])
})
