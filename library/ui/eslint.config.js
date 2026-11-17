import js from '@eslint/js'
import reactHooks from 'eslint-plugin-react-hooks'
import tseslint from 'typescript-eslint'

export default tseslint.config(
  {
    ignores: [
      'node_modules',
      '.agent-work',
      'figma/.figma-cache',
      'figma/figma-snapshot',
      'figma/test/fixtures',
      'figma/capture',
      'figma/review/review-dist',
      'figma/review/src/generated',
      'templates',
      // The Explorer HUD UI keeps the conventions of the HUD it moved from (react-web, which has no linter);
      // its hosts type-check it.
      'apps/hud'
    ]
  },
  js.configs.recommended,
  {
    files: ['**/*.mjs'],
    languageOptions: { globals: { console: 'readonly', process: 'readonly', URL: 'readonly' } }
  },
  ...tseslint.configs.recommended,
  { files: ['figma/review/src/**/*.{ts,tsx}'], ...reactHooks.configs.flat.recommended }
)
