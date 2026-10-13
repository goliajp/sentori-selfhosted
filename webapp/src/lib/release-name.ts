import type { ReleaseRow } from './api';
import type { I18nCtx } from '../i18n';
import { appDisplayName } from './app-name';

export type ReleaseScope = { appId?: string; platform?: string | null };
type NamedRelease = Pick<ReleaseRow, 'name' | 'apps'>;

/** The transport key remains exact; only the reading label is formatted. */
export function releaseVersion(raw: string) {
  const separator = raw.lastIndexOf('@');
  const version = separator >= 0 ? raw.slice(separator + 1) : raw;
  return version.replace(/\+(\d+)$/, ' ($1)');
}

export function releaseDisplayName(release: NamedRelease, t: I18nCtx['t'], scope: ReleaseScope = {}) {
  const apps = (release.apps ?? []).filter(app => (!scope.appId || app.id === scope.appId) && (!scope.platform || app.platform === scope.platform));
  const names = [...new Set(apps.map(app => {
    const name = appDisplayName(app, t);
    // Configured client names often include their platform. Keep it as a
    // separate dimension instead of repeating it in a shared release title.
    const suffix = ' ' + app.platform;
    return app.name && name.toLowerCase().endsWith(suffix.toLowerCase()) && name.length > suffix.length
      ? name.slice(0, -suffix.length) : name;
  }))].sort((a,b) => a.localeCompare(b));
  return [names.join(' / '), releaseVersion(release.name)].filter(Boolean).join(' ');
}
