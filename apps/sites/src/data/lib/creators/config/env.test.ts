import { describe, expect, it } from 'vitest'
import { Env, createEnvConfig, resolveEnv } from './env'

describe('resolveEnv', () => {
  it('ignores ?env= on a production hostname', () => {
    expect(resolveEnv({ host: 'decentraland.org', search: '?env=dev' }, Env.STAGING)).toBe(Env.PRODUCTION)
    expect(resolveEnv({ host: 'play.decentraland.org:443', search: '?env=dev' }, Env.STAGING)).toBe(Env.PRODUCTION)
    expect(resolveEnv({ host: 'decentraland.org.', search: '?env=dev' }, Env.STAGING)).toBe(Env.PRODUCTION)
  })

  it('honours ?env= on hosts that are not ours', () => {
    expect(resolveEnv({ host: 'example.org', search: '?env=dev' }, Env.STAGING)).toBe(Env.DEVELOPMENT)
    expect(resolveEnv({ host: 'notdecentraland.org', search: '?env=dev' }, Env.STAGING)).toBe(Env.DEVELOPMENT)
  })

  it('follows the Decentraland host, which ?env= may switch outside production', () => {
    expect(resolveEnv({ host: 'decentraland.zone', search: '' }, Env.PRODUCTION)).toBe(Env.DEVELOPMENT)
    expect(resolveEnv({ host: 'decentraland.today', search: '' }, Env.PRODUCTION)).toBe(Env.STAGING)
    expect(resolveEnv({ host: 'decentraland.zone', search: '?env=prod' }, Env.STAGING)).toBe(Env.PRODUCTION)
  })

  it('falls back to the default environment anywhere else', () => {
    expect(resolveEnv({ host: 'localhost:5212', search: '' }, Env.STAGING)).toBe(Env.STAGING)
    expect(resolveEnv({ host: 'localhost:5212', search: '?env=nonsense' }, Env.STAGING)).toBe(Env.STAGING)
    expect(resolveEnv(undefined, Env.STAGING)).toBe(Env.STAGING)
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
