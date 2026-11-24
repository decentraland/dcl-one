import { Env, createEnvConfig, resolveEnv } from './env'
import dev from './env/dev.json'
import prod from './env/prd.json'
import stg from './env/stg.json'

export { Env } from './env'

// A build is production unless its host or `VITE_CREATORS_ENV` says otherwise; the dev server uses the development services.
const fallback =
  resolveEnv({ host: '', search: `?env=${import.meta.env.VITE_CREATORS_ENV ?? ''}` }, import.meta.env.DEV ? Env.DEVELOPMENT : Env.PRODUCTION)

const BY_ENV = {
  [Env.DEVELOPMENT]: dev,
  [Env.STAGING]: stg,
  [Env.PRODUCTION]: prod
}

/** The page's environment: in the browser, the one its address names, or its node's network on a deployed host. */
export const config = createEnvConfig(
  BY_ENV,
  typeof window === 'undefined'
    ? fallback
    : resolveEnv(window.location, fallback, window.__DCL_PUBLIC__?.nodeNetworkId)
)

const search = typeof window === 'undefined' ? new URLSearchParams() : new URLSearchParams(window.location.search)

/** `?unity=false` forces the Babylon preview renderer, a debugging escape hatch; any other value is ignored. */
export const previewRendererOverride: 'babylon' | null = search.get('unity') === 'false' ? 'babylon' : null

/** The query string at call time: campaign params and analytics page context must follow the current URL. */
export const currentSearch = (): string => (typeof window === 'undefined' ? '' : window.location.search)
