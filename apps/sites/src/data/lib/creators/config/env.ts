export enum Env {
  DEVELOPMENT = 'dev',
  STAGING = 'stg',
  PRODUCTION = 'prod'
}

const DOMAIN = 'decentraland'

const ENV_BY_NAME: Record<string, Env> = {
  dev: Env.DEVELOPMENT,
  development: Env.DEVELOPMENT,
  zone: Env.DEVELOPMENT,
  stg: Env.STAGING,
  staging: Env.STAGING,
  today: Env.STAGING,
  prod: Env.PRODUCTION,
  production: Env.PRODUCTION,
  org: Env.PRODUCTION
}

/** The environment a Decentraland host serves: `.org` production, `.today` staging, `.zone` development. */
function envOfHost(host: string): Env | undefined {
  const labels = host.split(':')[0].toLowerCase().split('.')
  const tld = labels[labels.length - 1]
  return labels[labels.length - 2] === DOMAIN ? ENV_BY_NAME[tld] : undefined
}

export function isProductionHost(host: string): boolean {
  return envOfHost(host) === Env.PRODUCTION
}

/**
 * The environment for this page load: the host's on a Decentraland domain, `fallback` anywhere else. `?env=`
 * may switch any visitor but a production one, so a link cannot make decentraland.org sign requests against
 * dev or staging services.
 */
export function resolveEnv(location: Pick<Location, 'host' | 'search'> | undefined, fallback: Env): Env {
  if (!location) return fallback
  const ofHost = envOfHost(location.host)
  if (ofHost === Env.PRODUCTION) return ofHost
  const requested = new URLSearchParams(location.search).get('env')?.toLowerCase()
  return (requested && ENV_BY_NAME[requested]) || ofHost || fallback
}

export type EnvConfig = {
  get: (name: string, defaultValue?: string) => string
  is: (env: Env) => boolean
  getEnv: () => Env
}

/** The values of one environment, read by name. */
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
