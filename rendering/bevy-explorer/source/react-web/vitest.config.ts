import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import { SHARED_ALIASES, SHARED_DEDUPE } from './vite.shared'

// Deterministic integration tests (tier 1): render the real session/components with a
// fake bridge driver, assert every domain's wire API calls + response handling. No engine.
// The real-engine e2e (tier 2) lives under e2e/ and runs via Playwright, not Vitest.
export default defineConfig({
  plugins: [react(), {
    name: 'scene-restricted-actions',
    // The scene runtime supplies this virtual module. Tests provide its methods with vi.mock.
    resolveId(id) { return id === '~system/RestrictedActions' ? id : null }
  }],
  resolve: { alias: SHARED_ALIASES, dedupe: SHARED_DEDUPE },
  test: {
    environment: 'jsdom',
    globals: true,
    // jsdom's localStorage, not Node's own (on by default from Node 25): .nvmrc pins 24, the repo's Nix shell has a newer one.
    execArgv: ['--no-experimental-webstorage'],
    setupFiles: ['./src/test/setup.ts'],
    css: false,
    sequence: { hooks: 'list' },
    include: ['src/**/*.test.{ts,tsx}']
  }
})
