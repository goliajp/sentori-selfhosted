export type PushClientConfig = {
    apiUrl: string;
    projectId: string;
    token: string;
};
type AdminCredentialRow = {
    provider: string;
    config: Record<string, unknown>;
    updatedAt: string;
};
type Ticket = {
    id: string;
    status: 'queued' | 'sent' | 'failed';
    providerOutcome?: string | null;
    error?: string | null;
    retryCount: number;
    createdAt: string;
    sentAt?: string | null;
};
/** Parse a CLI flag value that may be `@file.json` (read from disk
 *  and parse) or a literal JSON string. */
export declare function parseJsonArg(raw: string, kind: string): unknown;
export declare function pushCredsList(cfg: PushClientConfig): Promise<AdminCredentialRow[]>;
export declare function pushCredsSet(cfg: PushClientConfig, provider: string, config: unknown, secret: unknown): Promise<{
    ok: boolean;
}>;
export declare function pushCredsDelete(cfg: PushClientConfig, provider: string): Promise<void>;
export type SendOpts = {
    to: string;
    title?: string;
    body?: string;
    data?: unknown;
    priority?: 'high' | 'normal';
    ttl?: number;
    idempotencyKey?: string;
};
export declare function pushSend(cfg: PushClientConfig, opts: SendOpts): Promise<Ticket>;
export declare function pushReceipt(cfg: PushClientConfig, sendId: string): Promise<{
    ticket: Ticket;
}>;
export {};
//# sourceMappingURL=push.d.ts.map