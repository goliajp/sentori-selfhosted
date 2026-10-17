import type { ProjectApp } from './api';
import type { I18nCtx } from '../i18n';
import { platformLabel } from './platform-label';

type AppName = Pick<ProjectApp, 'name' | 'identifier' | 'platform'>;

export function isIdentifiedApp(app: AppName) {
  return Boolean(app.name || app.identifier);
}

export function appDisplayName(app: AppName, t: I18nCtx['t']) {
  return app.name || app.identifier || t('apps.unidentified', {platform: platformLabel(app.platform, t)});
}

export function appDisplayLabel(app: AppName, t: I18nCtx['t']) {
  const name = appDisplayName(app, t);
  const platform = platformLabel(app.platform, t);
  return name.trim().toLowerCase() === platform.trim().toLowerCase()
    ? name : `${name} · ${platform}`;
}
