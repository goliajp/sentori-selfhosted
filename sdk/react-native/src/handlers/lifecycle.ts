/** Shared foreground state for signals, JS freeze detection and replay.
 * Both inactive and background suspend sampling; every transition advances
 * a generation so even a full suspend/resume between two ticks is visible.
 */
import { pushSignal } from '@goliapkg/sentori-core';

let _installed = false;
let _subscription: { remove: () => void } | null = null;
let _appState: AppStateLike | undefined;
let _state = 'active';
let _epoch = 0;

function updateState(state: string): void {
  if (state === _state) return;
  _state = state;
  _epoch += 1;
}

/** A generation changes even if background and resume happen between ticks. */
export function appLifecycle(): { active: boolean; epoch: number } {
  // currentState can change before the queued change callback reaches JS.
  if (_appState?.currentState != null) updateState(_appState.currentState);
  return { active: _state === 'active', epoch: _epoch };
}

type AppStateLike = {
  currentState?: string | null;
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

  bindLifecycle(AppState);
};

function bindLifecycle(AppState: AppStateLike): void {
  _appState = AppState;
  updateState(AppState.currentState ?? 'unknown');
  try {
    _subscription = AppState.addEventListener('change', (state) => {
      updateState(state);
      if (state === 'active') {
        pushSignal('lifecycle', { state: 'active' });
      } else if (state === 'background' || state === 'inactive') {
        pushSignal('lifecycle', { state: 'background' });
      }
    });
  } catch {
    // A missing/broken native subscription must never break host initialization.
  }
}

export const __uninstallLifecycleForTests = (): void => {
  _subscription?.remove();
  _subscription = null;
  _installed = false;
  _appState = undefined;
  _state = 'active';
  _epoch += 1;
};

export function __setLifecycleForTests(state: string): void {
  updateState(state);
}

export const __bindLifecycleForTests = bindLifecycle;
