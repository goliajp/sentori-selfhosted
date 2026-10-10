import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { build } from 'vite';
import { productionBoundary } from '../tooling/production-boundary.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
// These modules are canaries, not private preview fixtures. The check must also
// run from the public checkout, where vis/ is intentionally absent.
const canaries = new Set(['vis/src/model.ts', 'vis/src/store.tsx', 'devtools/mock-api.mjs'].map(path => `${root}${path}`));
async function compile(source, sharedSource) {
  return build({
    configFile: false,
    root,
    logLevel: 'silent',
    resolve: { alias: { '@demo': `${root}vis/src/model.ts` } },
    plugins: [productionBoundary(), {
      name: 'boundary-probe',
      resolveId(id) { if (id === 'entry') return '\0entry'; if (id === `${root}shared/probe.js` || canaries.has(id)) return id; },
      load(id) {
        if (id === '\0entry') return source;
        if (id === `${root}shared/probe.js`) return sharedSource;
        if (canaries.has(id)) return 'export const boundaryCanary = true;';
      },
    }],
    build: { write: false, minify: false, rollupOptions: { input: 'entry' } },
  });
}

await compile(`import { Spinner } from '${root}shared/design/Spinner.tsx'; console.log(Spinner)`);
for (const source of [
  `import '@demo'`,
  `export * from '${root}vis/src/model.ts'`,
  `import('${root}vis/src/store.tsx')`,
  `import '${root}devtools/mock-api.mjs'`,
]) {
  await assert.rejects(compile(source), /Production cannot import/);
}
await assert.rejects(
  compile(`import '${root}shared/probe.js'`, `import '${root}src/lib/theme.ts'`),
  /Shared design components must be independent/,
);
console.log('PASS: real build accepts shared UI and rejects fixture aliases, exports, lazy imports, dev tools and shared application-state dependencies');
