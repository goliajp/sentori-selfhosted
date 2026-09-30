export declare function scanProbes(root: string): string[];
export declare function syncProbes(opts: {
    apiUrl: string;
    token: string;
    release: string;
    refs: string[];
}): Promise<{
    registered: number;
}>;
//# sourceMappingURL=probes.d.ts.map