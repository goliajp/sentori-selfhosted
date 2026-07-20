// Top-level entry point. Most callers should pull from the more
// specific subpaths instead — see exports map in package.json:
//
//   @goliapkg/sentori-next/client          — clientInit + React surface
//   @goliapkg/sentori-next/server          — serverInit + onRequestError
//   @goliapkg/sentori-next/instrumentation — drop-in register/onRequestError
//
// Re-exports below are kept thin so a default `import { ... } from
// '@goliapkg/sentori-next'` still works for the common cases.
export { clientInit } from './client.js';
export { serverInit, onRequestError } from './server.js';
export { resolveConfig } from './config.js';
export { SentoriErrorBoundary, SentoriProvider, useCaptureError, useSentori, } from '@goliapkg/sentori-react';
// v2.8 — server-side Push helper. Re-export from the dedicated
// `/push` subpath so server-only code can `import { sentoriPush }
// from '@goliapkg/sentori-next/push'` and avoid pulling the rest of
// the surface. The top-level re-export here keeps `import { ... }
// from '@goliapkg/sentori-next'` working for the common case.
export { sentoriPush } from './push.js';
//# sourceMappingURL=index.js.map