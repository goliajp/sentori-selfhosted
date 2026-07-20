import type { Frame } from './types.js';
/**
 * JS-flavoured wrapper. The browser dashboard wants short filenames so
 * "https://cdn.example.com/static/App.tsx" displays as "static/App.tsx".
 * RN keeps the long path because Hermes paths are already short and
 * symbolication needs the absolute form.
 */
export declare function parseStack(stack: string | undefined): Frame[];
//# sourceMappingURL=stack.d.ts.map