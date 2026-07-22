// Shared data-loading hook for the list pages.
//
// Replaces the hand-rolled `useState` trio + `useEffect(() => { load() })`
// that every page grew independently. Beyond the boilerplate it fixes two
// bugs that shape had: a superseded response could overwrite a newer one,
// and a response arriving after unmount still called setState.

import { useCallback, useEffect, useRef, useState } from 'react';

import { ApiError } from './api';

export function formatApiError(e: unknown): string {
  return e instanceof ApiError ? `${e.status}: ${e.body}` : String(e);
}

export interface AsyncData<T> {
  data: T | null;
  error: string | null;
  loading: boolean;
  reload: () => void;
  setData: React.Dispatch<React.SetStateAction<T | null>>;
  setError: React.Dispatch<React.SetStateAction<string | null>>;
}

export function useAsyncData<T>(
  fetcher: () => Promise<T>,
  deps: unknown[],
  formatError: (e: unknown) => string = formatApiError,
): AsyncData<T> {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  // Read through refs so `reload` is stable yet always runs the newest
  // closure — pages build the fetcher from filter state that changes
  // without being in `deps`. Synced in an effect declared ahead of the
  // one that runs it, so a run always sees the current render's closure.
  const fetcherRef = useRef(fetcher);
  const formatRef = useRef(formatError);
  useEffect(() => {
    fetcherRef.current = fetcher;
    formatRef.current = formatError;
  });

  // Every run takes a ticket and only the newest ticket may write state.
  // Without it, switching projects twice in quick succession lets the
  // slower first response land last and clobber the newer data.
  const ticket = useRef(0);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const run = useCallback(async () => {
    const mine = ++ticket.current;
    // Settle to a value rather than letting it reject, so the rejection is
    // handled the moment it happens and never surfaces as unhandled.
    const settled = fetcherRef.current().then(
      value => ({ ok: true as const, value }),
      failure => ({ ok: false as const, failure }),
    );
    // Nothing above touches state. The flag flips only past this boundary,
    // so calling run() from an effect never sets state synchronously.
    await Promise.resolve();
    if (!alive.current || mine !== ticket.current) return;
    setLoading(true);
    const result = await settled;
    if (!alive.current || mine !== ticket.current) return;
    if (result.ok) {
      setData(result.value);
      setError(null);
    } else {
      setError(formatRef.current(result.failure));
    }
    setLoading(false);
  }, []);

  const reload = useCallback(() => {
    void run();
  }, [run]);

  useEffect(() => {
    void run();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);

  return { data, error, loading, reload, setData, setError };
}
