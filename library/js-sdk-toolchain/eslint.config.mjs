import { FlatCompat } from '@eslint/eslintrc'
import path from 'path'
import { fileURLToPath } from 'url'

// @dcl/eslint-config still publishes an eslintrc-style config; wrap it for eslint 9.
const compat = new FlatCompat({ baseDirectory: path.dirname(fileURLToPath(import.meta.url)) })

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
      '@typescript-eslint/consistent-type-imports': [
        'error',
        { prefer: 'type-imports', fixStyle: 'separate-type-imports', disallowTypeAnnotations: false }
      ],
      // `callback && callback()` and a JSX namespace's empty interfaces are deliberate here.
      '@typescript-eslint/no-unused-expressions': ['error', { allowShortCircuit: true, allowTernary: true }],
      '@typescript-eslint/no-empty-object-type': ['error', { allowInterfaces: 'always' }],
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
    // Tests load modules with `require` after `jest.doMock`, and scene fixtures hold bare expressions.
    files: ['test/**/*.ts', 'test/**/*.tsx'],
    rules: {
      '@typescript-eslint/no-require-imports': 'off',
      '@typescript-eslint/no-unused-expressions': 'off'
    }
  },
  {
    ignores: [
      '**/*.gen.ts',
      '**/dist/**',
      '**/node_modules/**',
      '**/*.d.ts',
      '**/*.js',
      '**/*.cjs',
      '**/*.mjs',
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
