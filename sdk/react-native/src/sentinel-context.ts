// Shared device + app collectors. Used by both capture.ts (errors)
// and pre-crash-sentinel.ts (nearCrash). Same shape so the dashboard
// can render both kinds of events through the same UI components.

import { getCachedNetworkType } from './netinfo';
import type { App, Device } from './types';

export const collectDeviceForSentinel = (): Device => {
  let os: Device['os'] = 'other';
  let osVersion = '0';
  let locale: string | undefined;
  const networkType = getCachedNetworkType();
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const RN = require('react-native') as {
      NativeModules: {
        I18nManager?: { localeIdentifier?: string };
        SettingsManager?: {
          settings?: { AppleLanguages?: string[]; AppleLocale?: string };
        };
      };
      Platform: { OS: string; Version: number | string };
    };
    const rnOS = RN.Platform.OS;
    os = rnOS === 'android' || rnOS === 'ios' || rnOS === 'web' ? rnOS : 'other';
    osVersion = String(RN.Platform.Version);
    if (rnOS === 'ios') {
      const s = RN.NativeModules.SettingsManager?.settings;
      locale = s?.AppleLocale ?? s?.AppleLanguages?.[0];
    } else if (rnOS === 'android') {
      locale = RN.NativeModules.I18nManager?.localeIdentifier;
    }
  } catch {
    // not in RN runtime
  }
  const device: Device = { os, osVersion };
  if (locale) device.locale = locale;
  if (networkType) device.networkType = networkType;
  return device;
};

export const getAppForSentinel = (release: string): App => {
  const m = /^(?:[^@]+@)?([^+]+)(?:\+(.+))?$/.exec(release);
  const version = m?.[1] ?? '0.0.0';
  const build = m?.[2];

  let rnVersion = 'unknown';
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    rnVersion = (require('react-native/package.json') as { version: string }).version;
  } catch {
    // not in RN runtime
  }

  return {
    build,
    framework: { name: 'react-native', version: rnVersion },
    version,
  };
};
