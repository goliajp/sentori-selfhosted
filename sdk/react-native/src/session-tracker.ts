/**
 * Phase 26 sub-B: RN session tracker glue.
 *
 * Mirrors the JS SDK's session-tracker but sends through the RN
 * transport. AppState binding lives in `handlers/lifecycle.ts`; this
 * file is just the singleton + the start/end/markErrored/markCrashed
 * surface.
 */
import { SessionTracker } from '@goliapkg/sentori-core';

import { getConfig } from './config';
import { getUser } from './capture';
import { sendSessionPing } from './transport';

let _tracker: null | SessionTracker = null;

const tracker = (): SessionTracker => {
  if (_tracker) return _tracker;
  _tracker = new SessionTracker((ping) => {
    const cfg = getConfig();
    if (!cfg) return;
    void sendSessionPing(cfg.ingestUrl, cfg.token, ping);
  });
  return _tracker;
};

export const startSession = (): void => {
  const cfg = getConfig();
  if (!cfg) return;
  const user = getUser();
  tracker().start({
    environment: cfg.environment,
    release: cfg.release,
    userId: user?.id ?? null,
  });
};

export const endSession = (status?: 'exited'): void => {
  if (!_tracker) return;
  _tracker.end(status);
};

export const markSessionErrored = (): void => {
  _tracker?.markErrored();
};

export const markSessionCrashed = (): void => {
  _tracker?.markCrashed();
};

export const __resetSessionForTests = (): void => {
  _tracker = null;
};
