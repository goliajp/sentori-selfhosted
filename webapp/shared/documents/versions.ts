import snapshots from './doc-versions.json';

export type VersionedDocPage = { slug: string; title: string; description?: string | null; markdown: string };
export type DocSupport = 'active' | 'lts' | 'eol' | 'unconfirmed';
export type DocVersion = {
  id: string; version: string; releasedAt: string; commit: string;
  channel: 'latest' | 'archive' | 'current'; support: DocSupport; lts: boolean | null;
  endOfSupport: string | null; pages: VersionedDocPage[]; changes: string;
};
// These are frozen exports of the named Git tags, not today's docs under old labels.
// Support policy is independent of release age and archive status.
export const docVersions = snapshots as DocVersion[];
export const latestDocVersion = docVersions.find(version => version.channel === 'latest')!;
export function getDocVersion(id: string | null | undefined) {
  return id ? docVersions.find(version => version.id === id) : latestDocVersion;
}
export function getVersionPage(version: DocVersion, slug: string) {
  const page = version.pages.find(page => page.slug === slug);
  return page ? { ...page, description: page.description ?? undefined } : undefined;
}
export function docParams(version: string, topic = '', anchor?: string) {
  return { version, topic, ...(anchor ? { anchor } : {}) };
}
