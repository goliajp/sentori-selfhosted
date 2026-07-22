/**
 * Phase 26 sub-B: AppState binding.
 *
 * Subscribes to AppState transitions:
 *   - active → start a fresh session (after a previous background end)
 *   - background / inactive → end the current session
 *
 * RN's AppState fires `inactive` on iOS during multitasking peek; we
 * end on it because that's effectively a background and the user may
 * not return. If they do, `active` starts a new one — the on-the-wire
 * session count goes up by one, which matches "the user opened the app
 * twice". Sentry historically did the opposite (treat inactive as
 * still alive), but that lets a swiped-away session never end.
 */
import { endSession, startSession } from '../session-tracker';

let _installed = false;
let _subscription: { remove: () => void } | null = null;

type AppStateLike = {
  addEventListener: (
    event: 'change',
    handler: (state: string) => void
  ) => { remove: () => void };
};

export const installLifecycleHandler = (): void => {
  if (_installed) return;
  _installed = true;
  let AppState: AppStateLike | undefined;
  try {
    // RN ships AppState; in test / non-RN host the require throws and
    // we silently no-op.
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    AppState = (require('react-native') as { AppState?: AppStateLike }).AppState;
  } catch {
    return;
  }
  if (!AppState || typeof AppState.addEventListener !== 'function') return;

  _subscription = AppState.addEventListener('change', (state) => {
    if (state === 'active') {
      startSession();
    } else if (state === 'background' || state === 'inactive') {
      endSession();
    }
  });
};

export const __uninstallLifecycleForTests = (): void => {
  _subscription?.remove();
  _subscription = null;
  _installed = false;
};
