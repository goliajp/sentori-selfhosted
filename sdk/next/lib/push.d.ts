import type { PushMessage, PushReceipt, PushTicket } from '@goliapkg/sentori-core';
export type SentoriPushConfig = {
    ingestUrl: string;
    token: string;
    fetch?: typeof fetch;
};
export type SentoriPushClient = {
    send(msg: PushMessage): Promise<PushTicket>;
    sendBatch(msgs: PushMessage[]): Promise<PushTicket[]>;
    getReceipt(sendId: string): Promise<PushReceipt>;
    isSentoriPushToken(value: unknown): value is string;
};
export declare function sentoriPush(cfg: SentoriPushConfig): SentoriPushClient;
//# sourceMappingURL=push.d.ts.map