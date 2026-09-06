import type { Frame } from './types.js';
export type ParseStackOptions = {
    /** Strip protocol + parent path so dashboard shows short filenames. */
    shortFilenames?: boolean;
};
export declare function parseStack(stack: string | undefined, opts?: ParseStackOptions): Frame[];
//# sourceMappingURL=stack.d.ts.map