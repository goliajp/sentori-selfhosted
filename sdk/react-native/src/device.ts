// Synchronous device snapshot — what the JS layer can know without
// crossing the bridge. The native module enriches asynchronously via
// setNativeDeviceInfo at init; until then the JS-visible subset
// ships (an event with a thin device beats a delayed event).

import type { Device } from '@goliapkg/sentori-core';

let _native: Partial<Device> | null = null;

export const setNativeDeviceInfo = (info: Partial<Device>): void => {
  _native = info;
};

export const collectDevice = (): Device | undefined => {
  let base: Device | undefined;
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const RN = require('react-native') as {
      Platform?: { OS?: string; Version?: number | string };
      Dimensions?: { get(dim: string): { width: number; height: number; scale: number } };
    };
    const os = RN.Platform?.OS ?? 'unknown';
    base = { os, osVersion: RN.Platform?.Version?.toString() };
    const win = RN.Dimensions?.get('window');
    if (win) {
      base.screen = { width: win.width, height: win.height, scale: win.scale };
    }
  } catch {
    return _native ? ({ os: 'unknown', ..._native } as Device) : undefined;
  }
  return _native ? { ...base, ..._native } : base;
};

export const __resetForTests = (): void => {
  _native = null;
};
