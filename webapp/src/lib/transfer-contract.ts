import type { BackupCommand } from './system-contract';
export interface TransferDescriptor {
  id: string; sourceInstanceId: string; sourceGeneration: number;
  targetInstanceId: string; targetGeneration: number; projectIds: string[];
}
export interface TransferCreate extends TransferDescriptor {
  commandId: string; direction: 'source' | 'target'; instanceId: string; generation: number;
}
export interface TransferCommand {
  commandId: string; instanceId: string; generation: number; version: number;
  action: 'export_copy' | 'seal' | 'export_final' | 'activate' | 'verify' | 'complete' | 'cancel';
  recoveryKey?: string; rollbackPlan?: string; sdkUpdated?: boolean; events?: Record<string, string>;
}
export interface TransferTail {
  commandId: string; instanceId: string; generation: number; version: number;
  projectId: string; targetOrigin: string; targetToken: string; expiresInDays: number;
}
export interface TransferJob {
  id: string; kind: 'export' | 'import'; phase: 'copy' | 'final';
  state: 'queued' | 'running' | 'ready' | 'failed' | 'cancelled' | 'unavailable';
  cancelRequested: boolean; bytes: number | null; sha256: string | null; errorCode: string | null;
  progress: { rows?: number; phase?: string }; createdAt: string;
}
export interface ProjectTransfer extends TransferDescriptor {
  direction: 'source' | 'target'; instanceId: string; generation: number; version: number;
  state: 'copying' | 'copied' | 'sealed' | 'ready' | 'active' | 'observing' | 'completed' | 'cancelled' | 'attention';
  projectMapping: Record<string, string>; observedEvents: Record<string, string>; verifiedEvents: Record<string, string>;
  rollbackPlan: string | null; createdAt: string; jobs: TransferJob[];
  routes: { projectId: string; targetProjectId: string; origin: string; enabled: boolean; expiresAt: string; forwarded: number; failed: number; lastSeenAt: string | null; lastError: string | null }[];
}
export interface TransferUpload { command: BackupCommand; phase: 'copy' | 'final'; version: number; file: File }
export function transferDescriptor(row: TransferDescriptor): TransferDescriptor {
  return { id: row.id, sourceInstanceId: row.sourceInstanceId, sourceGeneration: row.sourceGeneration, targetInstanceId: row.targetInstanceId, targetGeneration: row.targetGeneration, projectIds: row.projectIds };
}
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export function validIdentity(value: unknown): value is { id: string; generation: number } {
  if (!value || typeof value !== 'object') return false;
  const v = value as Record<string, unknown>;
  return typeof v.id === 'string' && uuid.test(v.id) && Number.isSafeInteger(v.generation) && Number(v.generation) >= 0;
}
export function parseTransferDescriptor(text: string): TransferDescriptor | null {
  try {
    const v = JSON.parse(text) as TransferDescriptor;
    if (!v || !validIdentity({ id: v.id, generation: 0 }) || !validIdentity({ id: v.sourceInstanceId, generation: v.sourceGeneration }) || !validIdentity({ id: v.targetInstanceId, generation: v.targetGeneration }) || v.sourceInstanceId === v.targetInstanceId || !Array.isArray(v.projectIds) || !v.projectIds.length || v.projectIds.length > 1000 || !v.projectIds.every(id => typeof id === 'string' && uuid.test(id)) || new Set(v.projectIds).size !== v.projectIds.length) return null;
    return transferDescriptor(v);
  } catch { return null; }
}
