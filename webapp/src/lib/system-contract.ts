/** Server-owned capability discovery. Availability never authorizes a request;
 * the server repeats the permission check for every object and operation. */
export type Capability = {
  supported: boolean;
  configured: boolean;
  allowed: boolean;
  reason: 'not_implemented' | 'not_configured' | 'owner_required' | 'managed_by_cloud' | null;
};
export type SystemInfo = {
  apiVersion: 1;
  serverVersion: string;
  deployment: 'self-hosted' | 'cloud';
  operatorName?: string | null;
  capabilities: Record<string, Capability | undefined>;
};
export type InstanceCheck = {
  version: number;
  origin: string;
  database: boolean;
  blobStorage: boolean;
  durableStorage: boolean;
  originValid: boolean;
};
export type InstanceHealth = {
  schemaVersion: 1;
  scope: 'read-only';
  checkedAt: string;
  status: 'pass' | 'warning' | 'fail' | 'unknown';
  backupMaxAgeHours: number;
  checks: { id: string; status: InstanceHealth['status']; code: string }[];
};
export type InstanceInfo = {
  operatingMode: 'normal' | 'restore_validation';
  restoredFrom: string | null;
  restoredSourceId: string | null;
  restoredAt: string | null;
  id: string;
  name: string;
  generation: number;
  version: number;
  initializedAt: string | null;
  lastCheck: InstanceCheck | null;
  checkedAt: string | null;
  origin: string;
  configurationSource: { name: 'database'; origin: 'environment'; owner: 'environment' | 'cloud' };
  serverVersion: string;
};
/** Unknown capability names are unavailable across rolling upgrades. */
export function canUse(system: SystemInfo | null, feature: string): boolean {
  const capability = system?.capabilities[feature];
  return !!capability?.supported && capability.configured && capability.allowed;
}

export type InstanceOperation = {
  id: string; instanceId: string; generation: number; kind: 'backup' | 'verify';
  state: 'queued' | 'running' | 'ready' | 'failed' | 'cancelled' | 'unavailable';
  attempt: number; cancelRequested: boolean;
  progress: { phase?: 'capturing' | 'verifying'; rows?: number; blobBytes?: number };
  bytes: number | null; sha256: string | null; errorCode: string | null;
  manifest: { archiveId: string; sourceInstanceId: string; generation: number; createdAt: string; tables: number; rows: number; blobs: number; blobBytes: number } | null;
  createdAt: string; startedAt: string | null; finishedAt: string | null;
};
export type BackupCommand = { commandId: string; instanceId: string; generation: number; recoveryKey: string };
export type ResetPermit = { operationId: string; permit: string; expiresAt: string };
export type ResetPreparation = { instanceId: string; instanceName: string; generation: number; currentPassword: string };
