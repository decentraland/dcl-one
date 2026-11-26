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

describe('when the page is on a deployed host', () => {
  const MAINNET = 1
  const SEPOLIA = 11155111

  describe.each(['dcl.dev', 'dcl.live', 'create.dcl.dev', 'dcl.live:443', 'dcl.dev.'])('and the host is %s', host => {
    it("should be production for a node on mainnet", () => {
      expect(resolveEnv({ host, search: '' }, Env.DEVELOPMENT, MAINNET)).toBe(Env.PRODUCTION)
    })

    it("should be development for a node on sepolia", () => {
      expect(resolveEnv({ host, search: '' }, Env.PRODUCTION, SEPOLIA)).toBe(Env.DEVELOPMENT)
    })

    it("should keep the node's environment if ?env= names another", () => {
      expect(resolveEnv({ host, search: '?env=dev' }, Env.PRODUCTION, MAINNET)).toBe(Env.PRODUCTION)
      expect(resolveEnv({ host, search: '?env=prod' }, Env.PRODUCTION, SEPOLIA)).toBe(Env.DEVELOPMENT)
      expect(resolveEnv({ host, search: '?env=stg' }, Env.PRODUCTION, SEPOLIA)).toBe(Env.DEVELOPMENT)
    })
  })

  describe("and the node's network is unknown", () => {
    it.each([undefined, 0, 5])('should resolve as on any other host for network %s', networkId => {
      expect(resolveEnv({ host: 'dcl.dev', search: '' }, Env.STAGING, networkId)).toBe(Env.STAGING)
      expect(resolveEnv({ host: 'dcl.dev', search: '?env=dev' }, Env.STAGING, networkId)).toBe(Env.DEVELOPMENT)
    })
  })
})

describe("when the page is on another host and the node's network is known", () => {
  const SEPOLIA = 11155111

  it.each(['localhost:5212', 'example.org', 'notdcl.dev', 'dcl.dev.example.org'])(
    'should not follow the node on %s',
    host => {
      expect(resolveEnv({ host, search: '' }, Env.PRODUCTION, SEPOLIA)).toBe(Env.PRODUCTION)
      expect(resolveEnv({ host, search: '?env=stg' }, Env.PRODUCTION, SEPOLIA)).toBe(Env.STAGING)
    }
  )

  it("should not follow the node on Decentraland's own hosts", () => {
    expect(resolveEnv({ host: 'decentraland.org', search: '' }, Env.STAGING, SEPOLIA)).toBe(Env.PRODUCTION)
    expect(resolveEnv({ host: 'decentraland.today', search: '' }, Env.PRODUCTION, SEPOLIA)).toBe(Env.STAGING)
    expect(resolveEnv({ host: 'decentraland.zone', search: '?env=prod' }, Env.STAGING, SEPOLIA)).toBe(Env.PRODUCTION)
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
