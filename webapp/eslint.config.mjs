import js from '@eslint/js'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import globals from 'globals'
// typescript-eslint refuses to load against TypeScript 7.0: it
// throws at import time, so `bun run check` dies before it lints
// anything. Support is tracked upstream for TS >= 7.1
// (typescript-eslint#10940). Until then `typescript` here stays
// on ~6.0.x — the four SDK packages are already on ^7, because
// none of them lint through this plugin.
import tseslint from 'typescript-eslint'

export default tseslint.config(
  { ignores: ['dist'] },
  {
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      ecmaVersion: 2024,
      globals: globals.browser,
    },
    plugins: {
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      'react-refresh/only-export-components': ['warn', { allowConstantExport: true }],
    },
  },
)
