import { type UploadOpts } from './upload.js';
export type UploadConfig = {
    version: 1;
    apiUrl: string;
    tokenEnv: string;
    release?: string;
    artifacts: {
        sourcemap?: string[];
        dsym?: string[];
        mapping?: string[];
    };
    sources?: {
        root?: string;
        include: string[];
        exclude?: string[];
        gitTag?: string;
    };
    sourceArchive?: string;
};
export type UploadPlan = {
    release: string;
    apiUrl: string;
    tokenEnv: string;
    files: {
        kind: UploadOpts['kind'];
        name: string;
        path: string;
        hash: string;
        bytes: number;
    }[];
    source?: {
        files: string[];
        tag?: string;
        commit?: string;
    };
};
export declare const WORKFLOW_HELP = "Release uploads:\n  sentori init --api-url <url> [--release <r>] [--sourcemap <path>] [--dsym <path>]\n               [--mapping <path>] [--sources <dir>] [--git-tag <tag>] [--config <file>]\n  sentori config validate [--config sentori.config.json] [--release <r>]\n  sentori doctor [--config sentori.config.json] [--api-url <url>]\n  sentori upload all [--config sentori.config.json] [--release <r>] [--git-tag <tag>]\n                    [--dry-run] [--strict]\n  sentori verify [--config sentori.config.json] [--release <r>] [--git-tag <tag>]\n  sentori sources pack --release <r> --output sources.zip [--root .] [--git-tag <tag>]\n                       [--exclude <path>] <source-dir-or-file...>\n  sentori upload srcbundle --release <r> [--root .] [--git-tag <tag>] <sources.zip|source-dir...>\n\nPaths in config are relative to that config. Sources keep paths relative to sources.root.\nSet the API-scope token in SENTORI_TOKEN (or the configured tokenEnv); init never stores it.\nRelease must match the SDK exactly, including its build number. SENTORI_RELEASE is supported.\nGit mode reads the specified tag's commit, never the working tree or a guessed latest tag.\nVerification checks every planned file's bytes and server validation; it does not claim event matching.\n";
export declare function loadUploadConfig(path: string): UploadConfig;
export declare function prepareUploadPlan(configPath: string, overrides?: {
    release?: string;
    apiUrl?: string;
    gitTag?: string;
}): UploadPlan;
export declare function verifyUploadPlan(plan: UploadPlan, token: string): Promise<string[]>;
export declare function runWorkflow(command: 'init' | 'validate' | 'upload' | 'verify' | 'doctor', argv: string[]): Promise<number>;
export declare function runSourcesPack(argv: string[]): Promise<number>;
//# sourceMappingURL=workflow.d.ts.map