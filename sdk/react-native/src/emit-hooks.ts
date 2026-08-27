// Post-emit hooks — how B-type replay (and future reactions) learn
// "an event just went out" without the verbs knowing about them.
//
// Hooks run inside their own try/catch: a broken subscriber must
// never take the emit path down with it (failure isolation).

import type { WireEvent } from '@goliapkg/sentori-core';
import { reportInternal } from '@goliapkg/sentori-core';

type EmitHook = (event: WireEvent) => void;

const hooks: EmitHook[] = [];

export const registerEmitHook = (hook: EmitHook): void => {
  hooks.push(hook);
};

export const onEventEmitted = (event: WireEvent): void => {
  for (const hook of hooks) {
    try {
      hook(event);
    } catch (e) {
      reportInternal('emit-hook', e);
    }
  }
};

export const __resetForTests = (): void => {
  hooks.length = 0;
};
