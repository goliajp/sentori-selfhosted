export type MomentProperties = Record<string, number | string>;
export type MomentStatus = 'abandoned' | 'failed' | 'ok' | 'open';
export declare class MomentHandle {
    private readonly span;
    private status;
    private readonly checkpoints;
    private readonly startedAtMs;
    constructor(name: string, props: MomentProperties);
    get name(): string;
    /** Record a named checkpoint within the moment. Cheap, in-memory;
     *  serialised onto the span data field at finish time. */
    checkpoint(label: string): void;
    /** Successful completion. */
    end(): void;
    /** Failed completion — moment ran but didn't reach success. */
    fail(reason?: string): void;
    /** User abandoned (foregrounded → backgrounded for > 30s, or app
     *  closed without `.end()`). Dashboard counts this in abandonment
     *  rate. */
    abandon(): void;
    /** Internal — finalize the span with the right status + ship
     *  checkpoint timestamps as data. */
    private finishWith;
    /** Test-only. */
    __getStatus(): MomentStatus;
}
export declare function startMoment(name: string, opts?: {
    properties?: MomentProperties;
}): MomentHandle;
//# sourceMappingURL=moments.d.ts.map