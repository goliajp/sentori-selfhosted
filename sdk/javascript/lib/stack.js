import { parseStack as parseStackCore } from '@goliapkg/sentori-core';
/**
 * JS-flavoured wrapper. The browser dashboard wants short filenames so
 * "https://cdn.example.com/static/App.tsx" displays as "static/App.tsx".
 * RN keeps the long path because Hermes paths are already short and
 * symbolication needs the absolute form.
 */
export function parseStack(stack) {
    return parseStackCore(stack, { shortFilenames: true });
}
//# sourceMappingURL=stack.js.map