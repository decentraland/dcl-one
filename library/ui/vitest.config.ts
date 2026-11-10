import { availableParallelism } from 'node:os'
import { fileURLToPath } from 'node:url'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [react()],
  test: {
    // The figma suites are bound by Git processes and disk I/O, not CPU: more than four workers only
    // stretch each case under contention (figma-design-system 72302bd).
    maxWorkers: Math.max(1, Math.min(4, availableParallelism() - 1)),
    projects: [
      {
        test: {
          name: 'figma',
          environment: 'node',
          include: [
            'figma/src/**/*.test.ts',
            'figma/test/**/*.test.ts',
            'figma/agent-kit/**/*.test.ts',
            'figma/key-guard.test.ts',
            'figma/lfs-prefetch.test.ts',
            'figma/lock-digests.test.ts',
            'figma/port-verification.test.ts'
          ],
          // Fixtures that workers build once and copy per test live in one folder per run.
          globalSetup: ['figma/test/global-setup.ts']
        }
      },
      {
        test: {
          name: 'consumer',
          environment: 'node',
          include: ['test/**/*.build.test.ts']
        }
      },
      {
        extends: true,
        test: {
          name: 'components',
          environment: 'jsdom',
          include: [
            'components/**/*.test.tsx',
            'foundations/**/*.test.ts',
            'test/**/*.test.{ts,tsx}',
            'figma/review/test/**/*.dom.test.tsx'
          ],
          // Build contracts run only in the consumer project, avoiding duplicate concurrent builds.
          exclude: ['test/**/*.build.test.ts', 'test/consumer/**'],
          setupFiles: ['test/setup.ts'],
          css: { modules: { classNameStrategy: 'non-scoped' } }
        }
      },
      {
        // The Explorer HUD UI (apps/hud), under the harness its hosts run it with: no CSS processing, and
        // the host behaviours (`@hud-host`) of a web host.
        extends: true,
        resolve: {
          alias: { '@hud-host': fileURLToPath(new URL('apps/hud/test/host.ts', import.meta.url)) }
        },
        test: {
          name: 'hud',
          environment: 'jsdom',
          include: ['apps/hud/test/**/*.test.{ts,tsx}'],
          setupFiles: ['apps/hud/test/setup.ts'],
          // jsdom's localStorage, not Node's own (on by default from Node 25).
          execArgv: ['--no-experimental-webstorage'],
          css: false
        }
      }
    ]
  }
})
