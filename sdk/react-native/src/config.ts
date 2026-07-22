import type { BeforeSendHook, LogLevel, ReadyInfo } from '@goliapkg/sentori-core';

/**
 * v2.3 — `ReadyInfo` is shared across SDKs via `@goliapkg/sentori-core`
 * so a host that switches from web to RN reads the same shape. The RN
 * SDK always populates `native` + `coldStartMs`; the core type marks
 * both optional for the web SDK's benefit (web has no native module).
 */
export type { ReadyInfo };

export type Config = {
  token: string;
  release: string;
  environment: string;
  ingestUrl: string;
  enabled: boolean;
  /** Phase 42 sub-D.07: opt-in screenshot capture on captureException. */
  screenshotsEnabled: boolean;
  /** Phase 44 sub-B: per-event-class sampling rates 0..1.
   *  `null` = keep everything (default). */
  errorSampleRate: null | number;
  traceSampleRate: null | number;
  /** v2.0 — sampling rate for `kind: 'message'` events emitted via
   *  `captureMessage`. `null` = keep all (default). */
  messageSampleRate: null | number;
  /** Phase 46: when true, every `captureException` seals the
   *  session-trail buffer and uploads it as a `sessionTrail`
   *  attachment. Defaults to false. */
  sessionTrailEnabled: boolean;
  /** v2.0 W3 — when true, every `track(name, props)` call also
   *  pushes a `{ type: 'track', data: { name, props } }` breadcrumb
   *  so a subsequent capture carries the customer journey. Defaults
   *  to false to preserve v1 customer breadcrumb shape on upgrade. */
  trackAutoBreadcrumb: boolean;
  /** v2.3 — Sentori console output gate.
   *
   *  Default `warn`: SDK is silent on host's console unless
   *  something is genuinely broken (transport sustained failure,
   *  native module not found, internal SDK exception). No
   *  per-tick / per-init / per-breadcrumb noise.
   *
   *  Set `'silent'` for absolute silence (e.g. CI smoke runs);
   *  set `'info'` or `'debug'` when debugging Sentori itself. */
  logLevel?: LogLevel;
  /** v2.3 — fires once after init completes. Use this to know the
   *  SDK is live instead of scanning the console. `info` carries
   *  the native-module bind status + cold-start timing. Host
   *  wraps any host-side logging here. */
  onReady?: (info: ReadyInfo) => void;
  /** v2.3 — host-side mutate-or-drop hook called once per event
   *  just before transport enqueue. Return the event to send it,
   *  `null` to drop. Sync only. If the hook throws or returns a
   *  non-event, SDK falls back to the un-mutated event and emits
   *  one one-shot warn. */
  beforeSend?: BeforeSendHook;
};

let _config: Config | null = null;

export const setConfig = (config: Config): void => {
  _config = config;
};

export const getConfig = (): Config | null => _config;

export const isInitialized = (): boolean => _config !== null;

export const __resetForTests = (): void => {
  _config = null;
};
