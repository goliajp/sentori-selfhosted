import { GlobalRegistrator } from '@happy-dom/global-registrator';
GlobalRegistrator.register();
// Phase 21 sub-B: tests run under happy-dom, but the JS SDK's
// transport tries to POST to ingestUrl. Stub fetch on every layer
// happy-dom exposes — global, window, and the constructor — so
// initialisation, hook installs, and capture paths all no-op.
const stubFetch = async () => new Response('{}', { status: 202 });
globalThis.fetch = stubFetch;
window.fetch = stubFetch;
window.sendBeacon = () => true;
navigator.sendBeacon = () => true;
//# sourceMappingURL=setup.js.map