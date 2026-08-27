// SDK-internal runtime config, set once by init(). Everything else
// reads through getConfig() and treats null as "not initialized →
// every verb is a no-op" (the failure-isolation iron rule).

import type { LogLevel, WireEvent } from '@goliapkg/sentori-core';

export type Config = {
  token: string;
  ingestUrl: string;
  release: string;
  environment: string;
  enabled: boolean;
  /** Warn-scenario auto-detection switches (conservative defaults). */
  detect: {
    rageTap: boolean;
    longFreeze: boolean;
    slowColdStart: boolean;
    slowApi: boolean;
  };
  /** B-type replay rolling buffer, seconds. 0 disables. */
  replaySeconds: number;
  replayScreens: boolean;
  /** The integrator's backend health URL; carried on batches, probed
   *  server-side. The app itself never pings it. */
  backendHealthUrl?: string;
  /** Sentori console output gate. Default `warn`: silent on the
   *  host's console unless something is genuinely broken. */
  logLevel?: LogLevel;
  /** Host-side mutate-or-drop hook, sync only. Throw / bad return →
   *  the un-mutated event ships and one warn is emitted. */
  beforeSend?: (event: WireEvent) => WireEvent | null;
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
