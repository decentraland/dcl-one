import { existsSync, readdirSync } from 'node:fs'

export const PORT = Number(process.env.E2E_PORT ?? 5283)
export const BASE = `http://localhost:${PORT}`
/** Where the site serves the creators pages. */
export const APP_PATH = '/create'
/** A host nothing answers: the page's telemetry is read off the requests it makes there. */
export const SEGMENT_HOST = 'segment.e2e.invalid'

/**
 * The env the e2e dev server runs with. Vite loads `.env*` files in every mode and they win over
 * `process.env` unless the key is present, so the developer-only overrides are pinned.
 */
export function hermeticEnv(): NodeJS.ProcessEnv {
  return {
    ...process.env,
    VITE_CREATORS_ENV: 'dev',
    VITE_FEATURE_FLAG_OVERRIDES: '',
    VITE_SEGMENT_WRITE_KEY: 'e2e-write-key',
    VITE_SEGMENT_API_HOST: SEGMENT_HOST
  }
}

/**
 * The browser: `CHROMIUM_BIN`, else the newest Chromium in the Nix store (the repository's development shell),
 * else playwright-core's own download (`pnpm exec playwright-core install chromium`).
 */
export function chromiumPath(): string | undefined {
  if (process.env.CHROMIUM_BIN) return process.env.CHROMIUM_BIN
  if (!existsSync('/nix/store')) return undefined
  const newest = readdirSync('/nix/store')
    .filter(name => /-chromium-[\d.]+$/.test(name))
    .sort((a, b) => a.slice(33).localeCompare(b.slice(33), undefined, { numeric: true }))
    .at(-1)
  return newest && existsSync(`/nix/store/${newest}/bin/chromium`) ? `/nix/store/${newest}/bin/chromium` : undefined
}
