import { availableParallelism } from 'node:os'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'
import { hudAliases } from './gallery/vite.config'

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
            'apps/sites/**/*.test.{ts,tsx}',
            'apps/shop/*.test.{ts,tsx}',
            'apps/shop/preview/*.test.{ts,tsx}',
            'foundations/**/*.test.ts',
            'test/**/*.test.{ts,tsx}',
            'figma/review/test/**/*.dom.test.tsx',
            'gallery/test/**/*.test.{ts,tsx}'
          ],
          // Build contracts run only in the consumer project, avoiding duplicate concurrent builds. The gallery's HUD
          // tests run in the hud project.
          exclude: ['test/**/*.build.test.ts', 'test/consumer/**', 'gallery/test/hud/**'],
          setupFiles: ['test/setup.ts'],
          css: { modules: { classNameStrategy: 'non-scoped' } }
        }
      },
      {
        // The Explorer HUD UI (apps/hud), with the gallery's canvas of its Showcase, under the harness its hosts run
        // it with: no CSS processing, and the host behaviours (`@hud-host`) of a web host.
        extends: true,
        resolve: { alias: hudAliases },
        test: {
          name: 'hud',
          environment: 'jsdom',
          include: ['apps/hud/**/*.test.{ts,tsx}', 'gallery/test/hud/**/*.test.{ts,tsx}'],
          // Tests that need packages only the HUD hosts install; react-web's Vitest runs them.
          exclude: ['apps/hud/test/hosted/**'],
          setupFiles: ['apps/hud/test/setup.ts'],
          // jsdom's localStorage, not Node's own (on by default from Node 25).
          execArgv: ['--no-experimental-webstorage'],
          css: false
        }
      }
    ]
  }
})
