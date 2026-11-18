import { afterEach, describe, expect, it } from 'vitest'
import { Env } from '@dcl/ui-env'
import { createEnvConfig, resolveBasePath, resolveEnv } from './env'

const SYSTEM = { VITE_DCL_DEFAULT_ENV: 'stg' }

describe('resolveEnv', () => {
  afterEach(() => window.history.replaceState({}, '', '/'))

  it('ignores ?env= on a production hostname', () => {
    window.history.replaceState({}, '', '/collections?env=dev')
    expect(resolveEnv({ host: 'decentraland.org' }, SYSTEM)).toBe(Env.PRODUCTION)
    expect(resolveEnv({ host: 'play.decentraland.org:443' }, SYSTEM)).toBe(Env.PRODUCTION)
    // Other .org hosts are not ours: the override keeps working there (previews, mirrors).
    expect(resolveEnv({ host: 'example.org' }, SYSTEM)).toBe(Env.DEVELOPMENT)
    expect(resolveEnv({ host: 'notdecentraland.org' }, SYSTEM)).toBe(Env.DEVELOPMENT)
  })

  it('honours ?env= everywhere else and falls back to the default environment', () => {
    window.history.replaceState({}, '', '/collections?env=dev')
    expect(resolveEnv(window.location, SYSTEM)).toBe(Env.DEVELOPMENT)
    window.history.replaceState({}, '', '/collections')
    expect(resolveEnv(window.location, SYSTEM)).toBe(Env.STAGING)
    expect(resolveEnv(undefined, SYSTEM)).toBe(Env.STAGING)
  })
})

describe('resolveBasePath', () => {
  it('mounts the router at /create when served by-path on the Decentraland domains', () => {
    expect(resolveBasePath('/create')).toBe('/create')
    expect(resolveBasePath('/create/collections/editor')).toBe('/create')
  })

  it('mounts the router at the root everywhere else', () => {
    expect(resolveBasePath('')).toBeUndefined()
    expect(resolveBasePath('/')).toBeUndefined()
    expect(resolveBasePath('/collections')).toBeUndefined()
    expect(resolveBasePath('/creator')).toBeUndefined()
    expect(resolveBasePath('/@dcl/creators-site/0.0.1/')).toBeUndefined()
  })
})

describe('createEnvConfig', () => {
  it('reads the resolved environment only', () => {
    const config = createEnvConfig(
      { [Env.DEVELOPMENT]: { URL: 'dev' }, [Env.PRODUCTION]: { URL: 'prod' } },
      Env.PRODUCTION
    )
    expect(config.get('URL')).toBe('prod')
    expect(config.get('MISSING', 'fallback')).toBe('fallback')
    expect(config.is(Env.PRODUCTION)).toBe(true)
    expect(config.getEnv()).toBe(Env.PRODUCTION)
    expect(() => createEnvConfig({}, Env.STAGING).get('URL')).toThrow()
  })
})
