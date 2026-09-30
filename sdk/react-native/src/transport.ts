// The React Native binding for the core transport.
//
// Everything about queueing, backoff, refusal handling, the offline
// queue and attachment upload lives in `@goliapkg/sentori-core` now, so
// the web and mini-program SDKs get the same behaviour rather than a
// third copy of a retry loop. What is left here is the part that is
// genuinely React Native: where the config comes from, that storage is
// an optional peer dependency behind a native module, and what this
// build calls itself on the wire.
//
// The exported names are unchanged on purpose — every caller in this
// package, and every test, still sees the module it always saw.

import type { AssertStat, AttachmentKind, SessionPing, WireEvent } from '@goliapkg/sentori-core';
import { createTransport, type TransportStorage } from '@goliapkg/sentori-core';

import { getConfig } from './config';
import { isAnyNativeModuleLinked } from './native-loader';

// Pinned to package.json by a test — bump both together.
const SDK_VERSION = '7.1.1';

const getAsyncStorage = async (): Promise<null | TransportStorage> => {
  // Host may have the JS package without pod install / prebuild →
  // getItem would crash from a microtask outside our reach.
  if (!isAnyNativeModuleLinked(['RNCAsyncStorage', 'AsyncStorageModule'])) {
    return null;
  }
  try {
    // Resolve via the host's runtime `require` rather than `import()`
    // — the peer dep is optional and absent in monorepo CI.
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const mod = require('@react-native-async-storage/async-storage') as {
      default?: TransportStorage;
    } & TransportStorage;
    return mod.default ?? mod;
  } catch {
    return null;
  }
};

const transport = createTransport({
  config: () => {
    const c = getConfig();
    if (!c) return null;
    return { ingestUrl: c.ingestUrl, token: c.token, backendHealthUrl: c.backendHealthUrl };
  },
  storage: getAsyncStorage,
  sdkLabel: `react-native/${SDK_VERSION}`,
  // Read at call time, not captured: the iron-rule tests replace the
  // global to inject network failures, and a reference taken at module
  // load would keep pointing at the real one.
  fetch: ((input: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) =>
    globalThis.fetch(input, init)) as typeof fetch,
});

export const enqueue = (event: WireEvent): void => transport.enqueue(event);
export const countAssert = (name: string, ok: boolean, release: string): void =>
  transport.countAssert(name, ok, release);
export const queueSession = (ping: SessionPing): void => transport.queueSession(ping);
export const startTransport = (): void => transport.start();
export const flush = (): Promise<void> => transport.flush();
export const queueAttachment = (
  eventId: string,
  kind: AttachmentKind,
  blob: { base64?: string; text?: string; mediaType: string },
  opts: { source?: 'android' | 'ios' | 'js' } = {},
): void => transport.queueAttachment(eventId, kind, blob, opts);
export const uploadAttachment = (
  eventId: string,
  kind: AttachmentKind,
  blob: { base64?: string; text?: string; mediaType: string },
  opts: { source?: 'android' | 'ios' | 'js' } = {},
): Promise<null | { ref: string }> => transport.uploadAttachment(eventId, kind, blob, opts);
export const drainOfflineQueue = (): Promise<void> => transport.drainOfflineQueue();

export const __resetForTests = (): void => transport.reset();
export const __peekQueue = (): readonly WireEvent[] => transport.peekQueue();
export const __peekSessions = (): readonly SessionPing[] => transport.peekSessions();
export const __peekDropped = (): number => transport.peekDropped();
export const __sdkVersion = (): string => SDK_VERSION;
export const __peekAssertStats = (): readonly AssertStat[] => transport.peekAssertStats();
