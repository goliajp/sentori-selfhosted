import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

const root = fileURLToPath(new URL('../', import.meta.url));
const forbidden = ['vis', 'devtools'].map(dir => `${resolve(root, dir).replaceAll('\\', '/')}/`);

/** Runs on the resolved graph, including aliases, re-exports and lazy chunks. */
export function productionBoundary() {
  return {
    name: 'sentori-production-boundary',
    enforce: 'pre',
    load(id) {
      const path = id.split('?')[0].replaceAll('\\', '/');
      if (forbidden.some(prefix => path.startsWith(prefix))) {
        this.error(`Production cannot import design fixtures or development tools: ${path}`);
      }
      return null;
    },
    moduleParsed(module) {
      const shared = `${resolve(root, 'shared').replaceAll('\\', '/')}/`;
      if (!module.id.replaceAll('\\', '/').startsWith(shared)) return;
      for (const id of [...module.importedIds, ...module.dynamicallyImportedIds]) {
        const path = id.replaceAll('\\', '/');
        if (path.startsWith(root.replaceAll('\\', '/')) && !path.startsWith(shared) && !path.includes('/node_modules/')) {
          this.error(`Shared design components must be independent of application state: ${module.id} imports ${id}`);
        }
      }
    },
  };
}
