import type { MessageKey } from '../i18n/en';
import { platformLabel } from './platform-label';

type Translate = (key: MessageKey, params?: Record<string, string>) => string;

// These labels are presentation only. API values, filenames, device model
// identifiers and custom names keep their original spelling.
const ARCHITECTURES: Record<string, string> = {
  arm: 'Arm', arm64: 'Arm64', aarch64: 'AArch64', arm64e: 'Arm64 (arm64e)',
  armv7: 'Armv7', armv7l: 'Armv7 (armv7l)', armv7s: 'Armv7 (armv7s)',
  'armeabi-v7a': 'Armv7 (armeabi-v7a)', 'arm64-v8a': 'Arm64 (arm64-v8a)',
  x86: 'x86', i386: 'i386', i686: 'i686',
  x86_64: 'x86-64', amd64: 'AMD64', 'x86-64': 'x86-64',
};
const PROVIDERS: Record<string, string> = {
  apns: 'APNs', fcm: 'FCM', webpush: 'Web Push', hcm: 'HUAWEI Push Kit', mipush: 'MiPush',
};
const ARTIFACTS: Record<string, MessageKey> = {
  sourcemap: 'artifact.sourceMap', dsym: 'artifact.dsym',
  proguard: 'artifact.proguard', srcbundle: 'artifact.sourceBundle',
};
const INTEGRATIONS: Record<string, string> = {github: 'GitHub', jira: 'Jira', webhook: 'Webhook'};
const INTEGRATION_EVENTS: Record<string, MessageKey> = {
  'issue.new': 'integration.event.issueNew', 'issue.regressed': 'integration.event.issueRegressed',
  'task.assigned': 'integration.event.taskAssigned', 'task.updated': 'integration.event.taskUpdated',
  'monitor.down': 'monitor.down', 'monitor.recovered': 'monitor.recovered',
};

export function signalKindLabel(value: string): string {
  return ({http: 'HTTP', ui: 'UI'} as Record<string, string>)[value] ?? value;
}

export function architectureLabel(value: string): string {
  return ARCHITECTURES[value.trim().toLowerCase()] ?? value;
}
export function providerLabel(value: string | null | undefined): string {
  return value ? PROVIDERS[value.trim().toLowerCase()] ?? value : '—';
}
export function artifactLabel(value: string, t: Translate): string {
  const key = ARTIFACTS[value];
  return key ? t(key) : value;
}
export function integrationLabel(value: string, t: Translate): string {
  return value === 'email' ? t('integration.email') : INTEGRATIONS[value] ?? value;
}
export function integrationEventLabel(value: string, t: Translate): string {
  const key = INTEGRATION_EVENTS[value];
  return key ? t(key) : value;
}

export function deviceSummary(device: Record<string, unknown> | null | undefined, t: Translate): string {
  if (!device) return '';
  const model = typeof device.model === 'string' ? architectureLabel(device.model) : '';
  const os = typeof device.os === 'string' && device.os.trim() ? platformLabel(device.os, t) : '';
  const version = typeof device.osVersion === 'string' || typeof device.osVersion === 'number' ? String(device.osVersion) : '';
  return [model, [os, version].filter(Boolean).join(' ')].filter(Boolean).join(' · ');
}
