export const PORT = Number(process.env.E2E_PORT ?? 5283)
export const BASE = `http://localhost:${PORT}`

/**
 * The env the e2e dev server runs with. Vite loads `.env*` files in every mode and they win over
 * `process.env` unless the key is present, so the developer-only overrides are pinned to empty strings.
 */
export function hermeticViteEnv(): NodeJS.ProcessEnv {
  return {
    ...process.env,
    VITE_DCL_DEFAULT_ENV: 'dev',
    VITE_FEATURE_FLAG_OVERRIDES: ''
  }
}
