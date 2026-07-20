import { GlobalRegistrator } from '@happy-dom/global-registrator'

GlobalRegistrator.register()

// Phase 21 sub-B: tests run under happy-dom, but the JS SDK's
// transport tries to POST to ingestUrl. Stub fetch on every layer
// happy-dom exposes — global, window, and the constructor — so
// initialisation, hook installs, and capture paths all no-op.
const stubFetch = async () => new Response('{}', { status: 202 })
;(globalThis as { fetch: unknown }).fetch = stubFetch
;(window as unknown as { fetch: unknown }).fetch = stubFetch
// Some happy-dom versions snapshot the original fetch constructor
// onto window.Window.prototype; overwriting our reference is harmless.
;(window as unknown as { sendBeacon?: unknown }).sendBeacon = () => true
;(navigator as unknown as { sendBeacon?: unknown }).sendBeacon = () => true
