import { useAsyncData, formatApiError } from './useAsyncData';

/** Scope-tag every result, including failures. A previous project's data must
 * disappear in the render where scope changes, before effects start fetching. */
export function useScopedAsyncData<T>(scope: string, fetcher: (signal: AbortSignal) => Promise<T>) {
  const state = useAsyncData(async signal => {
    try { return { scope, value: await fetcher(signal), failure: null }; }
    catch (error) { return { scope, value: null, failure: formatApiError(error) }; }
  }, [scope]);
  const current = state.data?.scope === scope ? state.data : null;
  return { data: current?.value ?? null, error: current?.failure ?? null,
    loading: state.loading || !current, reload: state.reload };
}
