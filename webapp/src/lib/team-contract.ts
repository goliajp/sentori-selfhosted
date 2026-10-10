export const taskStages = ['triage', 'ready', 'inProgress', 'review', 'verify', 'done', 'cancelled'] as const;
export type TaskStage = typeof taskStages[number];
export type TaskFields = { title: string; description: string; kind: 'task' | 'bug'; priority: 'urgent' | 'high' | 'normal' | 'low'; assignee: string | null; due: string | null; reproduction: string; rootCause: string };
export type Task = TaskFields & { id: string; projectId: string; issueId: string | null; stage: TaskStage; version: number; verified: boolean; fixRelease: string; automation: Record<string, unknown>; createdAt: string; updatedAt: string };
export type TaskEvent = { id: string; actor: string | null; actorName: string | null; action: string; body: string; at: string };
export type TaskCommand = { commandId: string } & (
  { action: 'create'; fields: TaskFields; issueId: string | null } |
  { action: 'edit'; fields: TaskFields; taskId: string; expectedVersion: number } |
  { action: 'transition'; taskId: string; expectedVersion: number; stage: TaskStage } |
  { action: 'comment'; taskId: string; expectedVersion: number; body: string; mention: string | null }
);
export type InboxItem = { id: string; projectId: string; taskId: string | null; monitorId: string | null; category: 'assignment' | 'mention' | 'task' | 'monitor'; title: string; body: string; at: string; read: boolean; archived: boolean };
export type InboxPreferences = { assignments: boolean; mentions: boolean; taskUpdates: boolean; monitors: boolean };
export const nextStages: Record<TaskStage, TaskStage[]> = { triage: ['ready', 'cancelled'], ready: ['inProgress', 'triage', 'cancelled'], inProgress: ['review', 'ready', 'cancelled'], review: ['verify', 'inProgress', 'cancelled'], verify: ['done', 'inProgress', 'cancelled'], done: ['inProgress'], cancelled: ['triage'] };
/** getRandomValues works on plain HTTP self-hosted instances too. */
export function commandId() {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 15) | 64; bytes[8] = (bytes[8] & 63) | 128;
  const hex = Array.from(bytes, value => value.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
