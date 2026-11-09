import js from '@eslint/js'
import reactHooks from 'eslint-plugin-react-hooks'
import tseslint from 'typescript-eslint'

export default tseslint.config(
  {
    ignores: [
      'node_modules',
      'figma/.figma-cache',
      'figma/figma-snapshot',
      'figma/test/fixtures',
      'figma/capture',
      'figma/review/review-dist',
      'figma/review/src/generated',
      'templates'
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
