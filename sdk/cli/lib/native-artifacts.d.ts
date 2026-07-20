export type AdminUpload = {
    apiUrl: string;
    projectId: string;
    release?: string;
    token: string;
};
export type DsymSlice = {
    arch: string;
    debugId: string;
    file: string;
};
/**
 * Use `dwarfdump --uuid <path>` to enumerate `(arch, debug_id, file)`
 * for each Mach-O slice. Returns [] if dwarfdump isn't installed or the
 * output couldn't be parsed; callers should fall back to explicit
 * `--debug-id` / `--arch` flags in that case.
 */
export declare function dsymSlicesFromDwarfdump(path: string): DsymSlice[];
/** Walk a `.dSYM` bundle and return the DWARF binary files inside
 *  `Contents/Resources/DWARF/`. If `path` already points at a binary
 *  (not a bundle), returns `[path]`. */
export declare function dwarfBinariesIn(path: string): string[];
export type DsymUploadOptions = AdminUpload & {
    /** Explicit overrides when dwarfdump isn't available. */
    arch?: string;
    debugId?: string;
    /** A `Foo.dSYM` bundle or a raw DWARF binary. */
    path: string;
    objectName?: string;
};
export type DsymUploadResult = {
    slices: {
        arch: string;
        debugId: string;
    }[];
};
export declare function uploadDsym(opts: DsymUploadOptions): Promise<DsymUploadResult>;
export type MappingUploadOptions = AdminUpload & {
    debugId?: string;
    path: string;
};
export declare function uploadMapping(opts: MappingUploadOptions): Promise<void>;
//# sourceMappingURL=native-artifacts.d.ts.map