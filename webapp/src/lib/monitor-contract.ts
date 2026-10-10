export type MonitorConfiguration = { name: string; url: string; intervalSeconds: number; timeoutSeconds: number; expectedStatus: number; failureThreshold: number; enabled: boolean };
export type MonitorSample = { at: string; ok: boolean; code: number | null; latency: number; error: string | null };
export type Monitor = MonitorConfiguration & { id: string; projectId: string; version: number; status: 'unknown' | 'operational' | 'degraded' | 'down'; incidentOpen: boolean; nextCheckAt: string; samples: MonitorSample[]; checks30d: number; availability: number | null };
export type MonitorCommand = { commandId: string; id: string; expectedVersion: number; action: 'save' | 'delete' | 'check'; configuration: MonitorConfiguration | null };
export type MonitorEvent = { id: string; incidentId: string; kind: 'monitor.down' | 'monitor.recovered'; at: string };
