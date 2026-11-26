import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    include: ['*.e2e.ts'],
    environment: 'node',
    globalSetup: ['./global-setup.ts'],
    // The dev server compiles each page on its first visit, which the first spec to open it pays for.
    testTimeout: 90_000,
    hookTimeout: 120_000,
    pool: 'forks',
    // Each spec drives a whole Chromium; parallel files on one machine only add contention.
    fileParallelism: false
  }
})
