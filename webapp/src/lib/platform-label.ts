// What a platform value is called on screen.
//
// Three of the five are proper names and stay as they are in every
// language — translating `ios` would make it harder to read, not
// easier. Two are not:
//
//   `unknown` is not a runtime. It is what the server writes when an
//   SDK names a platform this build predates, and a bare English word
//   sitting among Chinese and Japanese labels reads as a bug rather
//   than as the fact it is.
//
//   `weapp` has a name in each language — WeChat publishes them. It
//   was a Chinese literal here, so an English or Japanese dashboard
//   showed 微信小程序 among its own labels.

import type { MessageKey } from '../i18n/en';

// `projects.platform` is free-form and `events.platform` is a fixed
// vocabulary, and both arrive here. `react-native` is a project label
// only — it was the column default until it stopped being one — and
// without it a project created before that shows the raw identifier.
const NAMES: Record<string, string> = {
  android: 'Android',
  ios: 'iOS',
  javascript: 'JavaScript',
  'react-native': 'React Native',
  web: 'Web',
};

type Translate = (key: MessageKey, params?: Record<string, string>) => string;

export function platformLabel(platform: string | null | undefined, t: Translate): string {
  if (!platform) return '—';
  if (platform === 'unknown') return t('platform.unknown');
  if (platform === 'weapp') return t('platform.weapp');
  return NAMES[platform] ?? platform;
}
