import { createRequire } from 'module'
import path from 'path'
import { fileURLToPath } from 'url'

// eslint and @dcl/eslint-config's typescript-eslint plugins embed the TypeScript compiler API, which
// typescript 7 no longer ships: they are installed in tools/compiler-api with their own typescript 6,
// so the shareable config and its plugins resolve from there.
const toolsDirectory = path.join(path.dirname(fileURLToPath(import.meta.url)), 'tools', 'compiler-api')
const { FlatCompat } = createRequire(path.join(toolsDirectory, 'package.json'))('@eslint/eslintrc')

// @dcl/eslint-config still publishes an eslintrc-style config; wrap it for eslint 9.
const compat = new FlatCompat({ baseDirectory: toolsDirectory })

export default [
  ...compat.config({
    extends: '@dcl/eslint-config/sdk',
    parserOptions: {
      project: [
        'packages/@dcl/ecs/tsconfig.json',
        'packages/@dcl/sdk-commands/tsconfig.json',
        'packages/@dcl/sdk/tsconfig.json',
        'packages/@dcl/playground-assets/tsconfig.json',
        'packages/@dcl/react-ecs/tsconfig.json',
        'scripts/tsconfig.json',
        'test/tsconfig.json',
        'test/ecs/snippets/tsconfig.json'
      ]
    }
  }),
  {
    rules: {
      'no-param-reassign': 'warn',
      'no-console': 'error',
      '@typescript-eslint/no-unused-vars': [
        'warn',
        {
          ignoreRestSiblings: true,
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_|ReactEcs',
          caughtErrorsIgnorePattern: '^_'
        }
      ]
    }
  },
  {
    files: ['packages/@dcl/ecs/**/*.ts', 'packages/@dcl/sdk/**/*.ts', 'scripts/**/*.ts', 'test/**/*.ts'],
    rules: { 'no-console': 'off' }
  },
  {
    ignores: [
      '**/*.gen.ts',
      '**/dist/**',
      '**/node_modules/**',
      '**/*.d.ts',
      '**/*.js',
      'packages/@dcl/ecs/dist/**',
      'packages/@dcl/ecs/src/components/generated',
      'packages/@dcl/js-runtime',
      'packages/@dcl/playground-assets/dist',
      'packages/@dcl/sdk/dist/**',
      'packages/@dcl/sdk/internal/**',
      'packages/@dcl/sdk/testing/**',
      'test/sdk/simple-scene',
      'tmp'
    ]
  }
]
