import { describe, expect, test } from 'bun:test'

import { resolveConfig } from '../config.js'

const FULL_CLIENT = {
  NEXT_PUBLIC_SENTORI_ENVIRONMENT: 'prod',
  NEXT_PUBLIC_SENTORI_RELEASE: 'app@1.2.3',
  NEXT_PUBLIC_SENTORI_TOKEN: 'st_pk_testtesttesttesttesttesttest',
}

const FULL_SERVER = {
  SENTORI_ENVIRONMENT: 'prod',
  SENTORI_INGEST_URL: 'http://localhost:8080',
  SENTORI_RELEASE: 'app@1.2.3',
  SENTORI_TOKEN: 'st_pk_serverservertestservertest',
}

describe('resolveConfig', () => {
  test('client: pulls NEXT_PUBLIC_* and falls back to public ingest', () => {
    const cfg = resolveConfig('client', { envOverride: FULL_CLIENT })
    expect(cfg.environment).toBe('prod')
    expect(cfg.release).toBe('app@1.2.3')
    expect(cfg.token).toBe('st_pk_testtesttesttesttesttesttest')
    expect(cfg.ingestUrl).toBe('https://ingest.sentori.golia.jp')
  })

  test('server: prefers SENTORI_* over NEXT_PUBLIC_*', () => {
    const env = {
      ...FULL_SERVER,
      NEXT_PUBLIC_SENTORI_TOKEN: 'public-loses',
    }
    const cfg = resolveConfig('server', { envOverride: env })
    expect(cfg.token).toBe('st_pk_serverservertestservertest')
    expect(cfg.ingestUrl).toBe('http://localhost:8080')
  })

  test('server: falls back to NEXT_PUBLIC_* when SENTORI_* missing', () => {
    const cfg = resolveConfig('server', { envOverride: FULL_CLIENT })
    expect(cfg.token).toBe('st_pk_testtesttesttesttesttesttest')
  })

  test('explicit overrides win over env', () => {
    const cfg = resolveConfig('client', {
      envOverride: FULL_CLIENT,
      release: 'overridden@9.9.9',
    })
    expect(cfg.release).toBe('overridden@9.9.9')
  })

  test('throws on missing required field', () => {
    expect(() => resolveConfig('client', { envOverride: {} })).toThrow(
      /missing config field "environment"/,
    )
  })

  test('client side does not see SENTORI_* (only NEXT_PUBLIC_*)', () => {
    expect(() => resolveConfig('client', { envOverride: FULL_SERVER })).toThrow(
      /missing config field/,
    )
  })
})
