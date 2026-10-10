import { useRef, useState } from 'react';
import { api, ApiError } from './api';
import type {
  PushAction,
  PushCommand,
  PushCommandResult,
} from './push-contract';
import { commandId } from './team-contract';
import { formatApiError } from './useAsyncData';
export function usePushCommand(
  project: string,
  saved: (result: PushCommandResult) => void,
) {
  const [busy, setBusy] = useState(false),
    [error, setError] = useState<string | null>(null),
    [uncertain, setUncertain] = useState(false);
  const lock = useRef(false),
    pending = useRef<PushCommand | null>(null);
  async function run(action?: PushAction) {
    if (lock.current || (!pending.current && !action)) return;
    pending.current ??= {
      commandId: commandId(),
      action: structuredClone(action!),
    };
    lock.current = true;
    setBusy(true);
    setError(null);
    try {
      const result = await api.pushCommand(project, pending.current);
      pending.current = null;
      setUncertain(false);
      saved(result);
    } catch (failure) {
      if (
        failure instanceof ApiError &&
        failure.status >= 400 &&
        failure.status < 500
      ) {
        pending.current = null;
        setUncertain(false);
      } else setUncertain(true);
      setError(formatApiError(failure));
    } finally {
      lock.current = false;
      setBusy(false);
    }
  }
  return { busy, error, uncertain, locked: busy || uncertain, run };
}
