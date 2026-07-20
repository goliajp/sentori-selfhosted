// Phase 21 sub-C: env-driven config resolution.
//
// Next.js convention is `NEXT_PUBLIC_*` for browser-readable values
// and unprefixed for server-only. We honour both — clientInit() reads
// NEXT_PUBLIC_SENTORI_* (the bundler inlines these at build time);
// serverInit() reads SENTORI_* first, falling back to NEXT_PUBLIC_*
// so a single SaaS deploy can share a token between server and client
// when that's desired.

import type { CommonInitOptions } from '@goliapkg/sentori-core'

export type Side = 'client' | 'server'

export type SentoriNextConfig = Partial<CommonInitOptions> & {
  /** Override the env-resolution. Useful in tests. */
  envOverride?: Record<string, string | undefined>
}

const CLIENT_PREFIX = 'NEXT_PUBLIC_SENTORI_'
const SERVER_PREFIX = 'SENTORI_'

/** v2.0 W3 — CommonInitOptions now has a nested `capture` object that
 *  can't be resolved from a single env var, so KEY_MAP restricts to
 *  the four primitive-string fields the env layer actually drives.
 *  Nested options stay explicit-only via `cfg`. */
type EnvDrivenKey = 'environment' | 'ingestUrl' | 'release' | 'token'

const KEY_MAP: Record<EnvDrivenKey, string> = {
  environment: 'ENVIRONMENT',
  ingestUrl: 'INGEST_URL',
  release: 'RELEASE',
  token: 'TOKEN',
}

/**
 * Resolve a complete CommonInitOptions from env + explicit overrides.
 * `side` controls the env prefix; explicit values from `cfg` always
 * win.
 *
 * Throws when a required field is unresolved on either side — the
 * caller can catch + log at boot time and continue without Sentori
 * if the env isn't wired yet.
 */
export function resolveConfig(side: Side, cfg: SentoriNextConfig = {}): CommonInitOptions {
  const env = cfg.envOverride ?? processEnv()
  const out: Partial<CommonInitOptions> = {}

  for (const k of Object.keys(KEY_MAP) as EnvDrivenKey[]) {
    const explicit = cfg[k]
    if (explicit !== undefined) {
      out[k] = explicit
      continue
    }
    const suffix = KEY_MAP[k]
    const browser = env[`${CLIENT_PREFIX}${suffix}`]
    const server = env[`${SERVER_PREFIX}${suffix}`]
    const v = side === 'client' ? browser : (server ?? browser)
    if (v) out[k] = v
  }

  // v2.0 W3 — `capture` is nested, env can't drive it. Carry the
  // explicit value through so callers can still pass
  // `capture: { trackAutoBreadcrumb: true }` to resolveConfig().
  if (cfg.capture !== undefined) {
    out.capture = cfg.capture
  }

  // Defaults: ingestUrl points at the public SaaS if nothing was set.
  if (!out.ingestUrl) out.ingestUrl = 'https://ingest.sentori.golia.jp'

  for (const required of ['environment', 'release', 'token'] as const) {
    if (!out[required]) {
      throw new Error(
        `[sentori-next] missing config field "${required}" (set ` +
          `${side === 'client' ? CLIENT_PREFIX : SERVER_PREFIX}${KEY_MAP[required]} ` +
          `or pass it explicitly)`,
      )
    }
  }

  return out as CommonInitOptions
}

function processEnv(): Record<string, string | undefined> {
  // Both Node and browser bundlers expose `process.env` after Next's
  // build pipeline. The browser version only contains NEXT_PUBLIC_*.
  const p = (globalThis as { process?: { env?: Record<string, string | undefined> } }).process
  return p?.env ?? {}
}
