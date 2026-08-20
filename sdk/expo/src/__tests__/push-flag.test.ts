// `push: false` has to actually change the merged manifest.
//
// This runs the real mod rather than asserting the plugin composed
// something: config-plugins defer manifest edits until prebuild, so a
// test that only checks "the plugin was added" proves nothing about
// what lands in AndroidManifest.xml. We compose the plugin, then
// invoke the mod it registered against a manifest shaped like the one
// prebuild passes in.

import { describe, expect, test } from 'bun:test'

// eslint-disable-next-line @typescript-eslint/no-require-imports
const withSentori = require('../../app.plugin.js') as (
  config: Record<string, unknown>,
  props?: Record<string, unknown>,
) => AnyConfig

type ManifestApplication = {
  $: Record<string, string>
  service?: { $: Record<string, string> }[]
}
type Manifest = {
  manifest: { $: Record<string, string>; application: ManifestApplication[] }
}
type AnyConfig = {
  mods?: { android?: { manifest?: (c: unknown) => unknown | Promise<unknown> } }
  [k: string]: unknown
}

const SERVICE = 'com.sentori.SentoriFirebaseMessagingService'

function baseConfig() {
  return {
    name: 'app',
    slug: 'app',
    android: { package: 'com.example.app' },
    // `withPlugins` asserts on this; prebuild always supplies it.
    _internal: { projectRoot: '/tmp' },
  }
}

/** A manifest with our service present, the way manifest merging
 *  would hand it over once the SDK's own manifest is merged in. */
function baseManifest(): Manifest {
  return {
    manifest: {
      $: { 'xmlns:android': 'http://schemas.android.com/apk/res/android' },
      application: [{ $: { 'android:name': '.MainApplication' }, service: [] }],
    },
  }
}

async function runManifestMod(props: Record<string, unknown>): Promise<Manifest> {
  const config = withSentori(baseConfig(), props)
  const mod = config.mods?.android?.manifest
  if (!mod) throw new Error('plugin registered no android manifest mod')
  const out = (await mod({
    modResults: baseManifest(),
    modRequest: { platformProjectRoot: '/tmp', projectRoot: '/tmp' },
  })) as { modResults: Manifest }
  return out.modResults
}

describe('withSentori({ push })', () => {
  test('by default our FCM service is left alone', async () => {
    const m = await runManifestMod({})
    const services = m.manifest.application[0]?.service ?? []
    expect(services.some((s) => s.$['tools:node'] === 'remove')).toBe(false)
  })

  test('push: false removes our service from the merged manifest', async () => {
    const m = await runManifestMod({ push: false })
    const services = m.manifest.application[0]?.service ?? []
    const ours = services.find((s) => s.$['android:name'] === SERVICE)
    expect(ours).toBeDefined()
    expect(ours?.$['tools:node']).toBe('remove')
  })

  test('push: false declares the tools namespace, without which the attribute is inert', async () => {
    const m = await runManifestMod({ push: false })
    expect(m.manifest.$['xmlns:tools']).toBe('http://schemas.android.com/tools')
  })

  test('push: false does not add the POST_NOTIFICATIONS permission', async () => {
    const m = (await runManifestMod({ push: false })) as unknown as {
      manifest: { 'uses-permission'?: { $: Record<string, string> }[] }
    }
    const perms = m.manifest['uses-permission'] ?? []
    expect(perms.some((p) => p.$['android:name']?.endsWith('POST_NOTIFICATIONS'))).toBe(false)
  })

  test('push: false keeps the rest of the SDK — this is a delivery switch, not an off switch', async () => {
    const config = withSentori(baseConfig(), { push: false, sdkVersion: '6.1.0' }) as AnyConfig & {
      mods?: { ios?: { infoPlist?: (c: unknown) => unknown | Promise<unknown> } }
    }
    // The version stamp lands in Info.plist and is what the native
    // side reports; turning push off must not take it along.
    const mod = config.mods?.ios?.infoPlist
    expect(mod).toBeDefined()
    const out = (await mod!({ modResults: {}, modRequest: { projectRoot: '/tmp' } })) as {
      modResults: Record<string, string>
    }
    expect(Object.values(out.modResults)).toContain('6.1.0')
  })
})
