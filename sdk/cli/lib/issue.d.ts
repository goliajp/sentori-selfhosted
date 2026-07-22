type Issue = {
    errorType: string;
    eventCount: number;
    id: string;
    lastSeen: string;
    messageSample: string;
    status: 'active' | 'closed' | 'resolved' | 'silenced';
};
type AdminConfig = {
    apiUrl: string;
    projectId: string;
    token: string;
};
export type IssueListOptions = {
    config: AdminConfig;
    errorType?: string;
    limit?: number;
    status?: 'active' | 'closed' | 'resolved' | 'silenced';
};
export declare function issueList(opts: IssueListOptions): Promise<Issue[]>;
export declare function issuePatch(config: AdminConfig, issueId: string, body: {
    resolvedInRelease?: string;
    status: 'active' | 'closed' | 'resolved' | 'silenced';
}): Promise<Issue>;
/** Format one issue for terminal output — short, one line, scannable. */
export declare function formatIssueLine(i: Issue): string;
export {};
//# sourceMappingURL=issue.d.ts.map