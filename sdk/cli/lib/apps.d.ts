import type { ApiConfig } from './issue.js';
export type AppSummary = {
    id: string;
    projectId: string;
    name: string | null;
    identifier: string;
    platform: string;
    kind: string;
    events24h: number;
    errors24h: number;
    releaseCount: number;
};
export declare function listApps(config: ApiConfig): Promise<{
    projectId: string;
    apps: AppSummary[];
}>;
export declare function showProject(config: ApiConfig): Promise<{
    project: {
        id: string;
        name: string;
        createdAt: string;
    };
}>;
//# sourceMappingURL=apps.d.ts.map