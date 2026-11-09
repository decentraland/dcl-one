import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

// Deterministic integration tests (tier 1): render the real session/components with a
// fake bridge driver, assert every domain's wire API calls + response handling. No engine.
// The real-engine e2e (tier 2) lives under e2e/ and runs via Playwright, not Vitest.
export default defineConfig({
  plugins: [react(), {
    name: 'scene-system-modules',
    // The scene runtime supplies the `~system/*` virtual modules. Tests provide their methods with vi.mock.
    resolveId(id) { return id.startsWith('~system/') ? id : null }
  }],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    css: false,
    include: ['src/**/*.test.{ts,tsx}']
  }
})
