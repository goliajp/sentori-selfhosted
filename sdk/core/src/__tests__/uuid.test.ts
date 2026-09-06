import { expect, test } from 'bun:test'

import { uuidV7 } from '../uuid.js'

test('uuidV7: shape', () => {
  const id = uuidV7()
  expect(id).toMatch(
    /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
  )
})

test('uuidV7: timestamp prefix is monotonic-ish', async () => {
  const a = uuidV7()
  await new Promise((r) => setTimeout(r, 5))
  const b = uuidV7()
  // First 12 hex chars are the unix-ms timestamp.
  const aMs = parseInt(a.slice(0, 8) + a.slice(9, 13), 16)
  const bMs = parseInt(b.slice(0, 8) + b.slice(9, 13), 16)
  expect(bMs).toBeGreaterThanOrEqual(aMs)
})

test('uuidV7: 1000 calls produce 1000 unique ids', () => {
  const set = new Set<string>()
  for (let i = 0; i < 1000; i++) set.add(uuidV7())
  expect(set.size).toBe(1000)
})
