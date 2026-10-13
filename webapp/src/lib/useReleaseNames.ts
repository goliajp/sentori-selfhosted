import { api, type ReleaseRow } from './api';
import { useT } from '../i18n';
import { useScopedAsyncData } from './useScopedAsyncData';
import { releaseDisplayName, type ReleaseScope } from './release-name';

/** Resolve only visible release keys, including historical rows outside page 1. */
export function useReleaseNames(projectId: string, names: (string | null | undefined)[], known: ReleaseRow[] = []) {
  const t = useT();
  const wanted = [...new Set(names.filter((name): name is string => !!name))].sort();
  const missing = wanted.filter(name => !known.some(row => row.name === name));
  const state = useScopedAsyncData(JSON.stringify([projectId, missing]), async signal => {
    const rows: ReleaseRow[] = [];
    if (projectId) {
      let batch: string[] = [];
      const fetchBatch = async () => {
        rows.push(...(await api.listReleases(projectId, {names: batch, limit: 100}, signal)).releases);
        batch = [];
      };
      for (const name of missing) {
        // Bound the encoded URL as well as the number of keys; old versions
        // can contain long Unicode names.
        if (batch.length && (batch.length === 100 || encodeURIComponent(JSON.stringify([...batch, name])).length > 6000)) await fetchBatch();
        batch.push(name);
      }
      if (batch.length) await fetchBatch();
    }
    return rows;
  });
  const rows = new Map([...state.data ?? [], ...known].map(row => [row.name, row]));
  return {
    ...state,
    get: (raw: string) => rows.get(raw),
    label: (raw: string | null | undefined, scope?: ReleaseScope) => raw ? releaseDisplayName(rows.get(raw) ?? {name: raw}, t, scope) : '—',
  };
}
