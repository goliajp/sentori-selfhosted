import { describe, expect, test } from 'bun:test'

import { normalizeUrl } from '../url.js'

describe('normalizeUrl', () => {
  test('replaces numeric id segments', () => {
    expect(normalizeUrl('https://api.example.com/users/123')).toBe(
      'https://api.example.com/users/{id}',
    )
    expect(normalizeUrl('https://api.example.com/users/123/orders/456')).toBe(
      'https://api.example.com/users/{id}/orders/{id}',
    )
  })

  test('replaces UUID segments', () => {
    expect(normalizeUrl('https://api.example.com/devices/69ef2dc5-c11e-a382-0b7c-fd1d12345678')).toBe(
      'https://api.example.com/devices/{id}',
    )
  })

  test('replaces long-hex / ObjectId segments', () => {
    // 24-hex (mongo ObjectId style), like the screenshot's device id
    expect(normalizeUrl('https://api.staging.focusai.com/api/devices/69ef2dc5c11ea3820b7cfd1d')).toBe(
      'https://api.staging.focusai.com/api/devices/{id}',
    )
    expect(normalizeUrl('https://x.com/sites/657785a99744216b7d213686')).toBe(
      'https://x.com/sites/{id}',
    )
  })

  test('replaces long opaque alphanumeric tokens (>=20 chars with a digit)', () => {
    expect(normalizeUrl('https://x.com/blobs/aZ09aZ09aZ09aZ09aZ09x')).toBe(
      'https://x.com/blobs/{id}',
    )
  })

  test('leaves slugs / words / short codes alone', () => {
    expect(normalizeUrl('https://api.example.com/v1/users/me')).toBe(
      'https://api.example.com/v1/users/me',
    )
    expect(normalizeUrl('https://shop.example.com/products/winter-jacket-2024')).toBe(
      'https://shop.example.com/products/winter-jacket-2024',
    )
    expect(normalizeUrl('https://api.example.com/v1/orders')).toBe(
      'https://api.example.com/v1/orders',
    )
    // short alphanumeric code (e.g. VC64VOVEX0VT, 12 chars) is left as-is —
    // we'd rather under-normalize than collapse a real path segment
    expect(normalizeUrl('https://device.example.com/VC64VOVEX0VT/api/health')).toBe(
      'https://device.example.com/VC64VOVEX0VT/api/health',
    )
  })

  test('drops query string and fragment', () => {
    expect(
      normalizeUrl('https://api.example.com/thumbnails?interval=-86400s&limit=20&order_by=starttime_desc'),
    ).toBe('https://api.example.com/thumbnails')
    expect(normalizeUrl('https://api.example.com/x/123?token=abc#frag')).toBe(
      'https://api.example.com/x/{id}',
    )
  })

  test('keeps scheme + host (does not touch the host even if it looks id-ish)', () => {
    expect(normalizeUrl('http://192.168.1.100:9999/perf-pulse')).toBe(
      'http://192.168.1.100:9999/perf-pulse',
    )
  })

  test('handles relative URLs', () => {
    expect(normalizeUrl('/devices/abc?organization_id=657785a99744216b7d213686')).toBe('/devices/abc')
    expect(normalizeUrl('/users/123/profile')).toBe('/users/{id}/profile')
    expect(normalizeUrl('/health')).toBe('/health')
  })

  test('handles empty / weird input without throwing', () => {
    expect(normalizeUrl('')).toBe('')
    expect(normalizeUrl('not a url at all')).toBe('not a url at all')
  })
})
