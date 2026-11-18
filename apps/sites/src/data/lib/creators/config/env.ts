import { Env, getEnv } from '@dcl/ui-env'

type SystemEnvVariables = NonNullable<Parameters<typeof getEnv>[0]>

// The production domain and its subdomains; narrower than @dcl/ui-env's "any .org / .co" rule.
const PRODUCTION_DOMAIN = 'decentraland.org'

export function isProductionHost(host: string): boolean {
  const hostname = host.split(':')[0].toLowerCase()
  return hostname === PRODUCTION_DOMAIN || hostname.endsWith(`.${PRODUCTION_DOMAIN}`)
}

/**
 * The environment for this page load. `?env=` may switch a local, .zone or .today visitor to another
 * back end; a production visitor is never switched, so a link cannot make decentraland.org sign
 * requests against dev or staging services.
 */
export function resolveEnv(location: Pick<Location, 'host'> | undefined, systemEnvVariables: SystemEnvVariables): Env {
  if (location && isProductionHost(location.host)) return Env.PRODUCTION
  return getEnv(systemEnvVariables)
}

/** Where the app is mounted on the Decentraland domains: decentraland.<tld>/create. */
export const SITE_PATH = '/create'

/**
 * The router basename for this page load: `/create` when served by-path on the Decentraland domains,
 * the root everywhere else (localhost, Vercel previews, tests). Decided from the pathname: a hostname
 * allowlist blanks the page on any host it doesn't know, and the CDN base is where the assets live, not
 * where the app is mounted.
 */
export function resolveBasePath(pathname: string): string | undefined {
  return pathname === SITE_PATH || pathname.startsWith(`${SITE_PATH}/`) ? SITE_PATH : undefined
}

export type EnvConfig = {
  get: (name: string, defaultValue?: string) => string
  is: (env: Env) => boolean
  getEnv: () => Env
}

/** `@dcl/ui-env`'s config surface over a pre-resolved environment. */
export function createEnvConfig(configByEnv: Partial<Record<Env, Record<string, string>>>, env: Env): EnvConfig {
  const values = configByEnv[env]
  return {
    get: (name, defaultValue = '') => {
      if (!values) throw new Error(`Could not find a config for env=${env}`)
      return name in values ? values[name] : defaultValue
    },
    is: candidate => env === candidate,
    getEnv: () => env
  }
}
