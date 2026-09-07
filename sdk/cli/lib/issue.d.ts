type Issue = {
    id: string;
    kind: string;
    title: string;
    messageSample: string;
    status: string;
    eventCount: number;
    usersCount: number;
    maxPerUser: number;
    lastSeen: string;
    regressed: boolean;
};
export type ApiConfig = {
    apiUrl: string;
    token: string;
};
export declare function listIssues(c: ApiConfig, opts?: {
    status?: string;
    kind?: string;
}): Promise<Issue[]>;
export declare function resolveIssue(c: ApiConfig, issueId: string, release?: string): Promise<void>;
export declare function noteIssue(c: ApiConfig, issueId: string, body: string): Promise<void>;
export declare function fetchBundle(c: ApiConfig, issueId: string): Promise<string>;
export declare function formatIssueLine(i: Issue): string;
export {};
//# sourceMappingURL=issue.d.ts.map