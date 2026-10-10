import type { PushSend } from './api';
export type PushAction =
  | {
      type: 'createCredential';
      provider: string;
      config: Record<string, unknown>;
      secret: string;
      label: string;
    }
  | {
      type: 'activateCredential';
      id: string;
      expectedVersion: number;
      force: boolean;
    }
  | {
      type: 'deleteCredential' | 'revokeDevice';
      id: string;
      expectedVersion: number;
    }
  | { type: 'testSend'; deviceId: string; title: string; body: string }
  | {
      type: 'retrySend';
      id: string;
      expectedVersion: number;
      acknowledgeDuplicate: boolean;
    }
  | { type: 'retryFailed'; ids: string[] };
export type PushCommand = { commandId: string; action: PushAction };
export type PushCommandResult = {
  id?: string;
  sendId?: string;
  requeued?: number;
};
export type PushDetail = {
  send: PushSend;
  attempts: {
    id: string;
    attempt: number;
    outcome: string;
    httpStatus: number | null;
    durationMs: number | null;
    at: string;
  }[];
};
