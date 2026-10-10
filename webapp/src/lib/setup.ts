import { api, type EventDetail } from './api';

export const SETUP_PLATFORMS = ['react-native', 'web', 'weapp', 'ios', 'android'] as const;
export type SetupPlatform = (typeof SETUP_PLATFORMS)[number];

export function setupPlatform(value: string | null): SetupPlatform {
  return SETUP_PLATFORMS.find((p) => p === value) ?? 'react-native';
}

export const SETUP_DOCS: Record<SetupPlatform, string> = {
  'react-native': '/docs/getting-started/react-native',
  web: '/docs/getting-started/web',
  weapp: '/docs/getting-started/weapp',
  ios: '/docs/sdk-swift',
  android: '/docs/sdk-kotlin',
};

export function setupCheckId(projectId: string, reset = false): string {
  const key = `sentori_setup_check:${projectId}`;
  try {
    const saved = sessionStorage.getItem(key);
    if (saved && !reset) return saved;
    const id = crypto.randomUUID();
    sessionStorage.setItem(key, id);
    return id;
  } catch {
    return crypto.randomUUID();
  }
}

/** A receipt must belong to the example this user ran. Other project
 * traffic, or an uploaded artifact alone, does not verify this setup. */
export async function findSetupEvent(projectId: string, checkId: string): Promise<{ issueId: string; event: EventDetail } | null> {
  const lists = await Promise.all(['open', 'resolved', 'ignored'].map((status) => api.listIssues({
    projectId, status, kind: 'error', contextKey: 'sentoriSetup', contextValue: checkId, limit: 5,
  })));
  const issues = lists.flatMap((list) => list.issues).filter((issue) => issue.projectId === projectId);
  for (const issue of issues) {
    const { events } = await api.listOccurrences(issue.id);
    for (const occurrence of events.slice(0, 10)) {
      const event = await api.getEvent(occurrence.id);
      const context = event.payload?.context as Record<string, unknown> | undefined;
      if (event.projectId === projectId && context?.sentoriSetup === checkId) return { issueId: issue.id, event };
    }
  }
  return null;
}

/** The test checks delivery and source coordinates separately. Native
 * frames may arrive readable without the JS `symbolicated` flag. */
export function hasReadableSource(event: EventDetail): boolean {
  const error = event.payload?.error as { stack?: { file?: string; line?: number; symbolicated?: boolean; contextLine?: string }[] } | undefined;
  if (!Array.isArray(error?.stack)) return false;
  return error.stack.some((frame) => Boolean(
    frame && typeof frame.file === 'string' && frame.file && frame.file !== '?' && frame.file !== '<unknown>' &&
    typeof frame.line === 'number' && frame.line > 0 &&
    (frame.symbolicated === true || typeof frame.contextLine === 'string'),
  ));
}
