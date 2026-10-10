import { useRef, useState } from 'react';
import { api, ApiError } from './api';
import { formatApiError } from './useAsyncData';
import type { IntegrationKind, IntegrationConfigure, IntegrationSend, IntegrationRetry } from './integration-contract';

type Operation = { type: 'configure'; kind: IntegrationKind; body: IntegrationConfigure } | { type: 'send'; id: string; body: IntegrationSend } | { type: 'retry'; id: string; body: IntegrationRetry };
export function useIntegrationCommand(project: string, saved: () => void) {
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null), [uncertain, setUncertain] = useState(false);
  const lock = useRef(false), pending = useRef<Operation | null>(null);
  async function run(operation: Operation) {
    if (lock.current) return;
    lock.current = true; pending.current ??= operation; setBusy(true); setError(null);
    try {
      const p = pending.current;
      if (p.type === 'configure') await api.configureIntegration(project, p.kind, p.body);
      else if (p.type === 'send') await api.sendIntegration(project, p.id, p.body);
      else await api.retryDelivery(project, p.id, p.body);
      pending.current = null; setUncertain(false); saved();
    } catch (failure) {
      if (failure instanceof ApiError && failure.status >= 400 && failure.status < 500) { pending.current = null; setUncertain(false); } else setUncertain(true);
      setError(formatApiError(failure));
    } finally { lock.current = false; setBusy(false); }
  }
  return { busy, error, uncertain, run, retry: () => { if (pending.current) void run(pending.current); } };
}
