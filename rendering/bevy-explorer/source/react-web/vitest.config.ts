import { searchForWorkspaceRoot } from 'vite'
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import { SHARED_ALIASES, SHARED_DEDUPE, SHARED_TREES } from './vite.shared'

// Deterministic integration tests (tier 1): render the real session/components with a
// fake bridge driver, assert every domain's wire API calls + response handling. No engine.
// The real-engine e2e (tier 2) lives under e2e/ and runs via Playwright, not Vitest.
export default defineConfig({
  plugins: [react(), {
    name: 'scene-system-modules',
    // The scene runtime supplies the `~system/*` virtual modules. Tests provide their methods with vi.mock.
    resolveId(id) { return id.startsWith('~system/') ? id : null }
  }],
  resolve: { alias: SHARED_ALIASES, dedupe: SHARED_DEDUPE },
  // The shared sources live outside this app (vite.shared.ts), like in vite.config.ts.
  server: { fs: { allow: [searchForWorkspaceRoot(process.cwd()), ...SHARED_TREES] } },
  test: {
    environment: 'jsdom',
    globals: true,
    // jsdom's localStorage, not Node's own (on by default from Node 25): .nvmrc pins 24, the repo's Nix shell has a newer one.
    execArgv: ['--no-experimental-webstorage'],
    setupFiles: ['./src/test/setup.ts'],
    css: false,
    sequence: { hooks: 'list' },
    // The shared HUD UI's and the Explorer page core's tests too, including those that need this app's packages.
    include: ['src/**/*.test.{ts,tsx}', '../../../../library/ui/apps/hud/**/*.test.{ts,tsx}', '../../../../library/explorer-core/**/*.test.{ts,tsx}']
  }
})
