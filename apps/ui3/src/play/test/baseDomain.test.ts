// DOMAIN: base-domain derivation (lib/baseDomain.ts) — which deployment the HUD keys its
// backend hosts to. The trust side (isTrustedBaseDomain + the launch gate) is covered in
// systemScene.test.tsx.

import { afterEach, describe, expect, it, vi } from 'vitest'
import { hostBaseDomain, normaliseBaseDomain } from '../lib/baseDomain'

describe('hostBaseDomain', () => {
  it('derives the apex from decentraland deployments, bare or subdomain', () => {
    expect(hostBaseDomain('decentraland.org')).toBe('decentraland.org')
    expect(hostBaseDomain('play.decentraland.org')).toBe('decentraland.org')
    expect(hostBaseDomain('decentraland.zone')).toBe('decentraland.zone')
    expect(hostBaseDomain('cdn.decentraland.zone')).toBe('decentraland.zone')
  })

  it('derives nothing from unrecognised hosts — the param or the org default decide instead', () => {
    expect(hostBaseDomain('localhost')).toBe(null)
    expect(hostBaseDomain('interconnected.online')).toBe(null)
    expect(hostBaseDomain('decentraland.org.evil.example')).toBe(null)
    expect(hostBaseDomain('evil-decentraland.org')).toBe(null)
  })
})

describe('normaliseBaseDomain', () => {
  it('lowercases a bare domain, as the engine does', () => {
    expect(normaliseBaseDomain('Decentraland.org')).toBe('decentraland.org')
    expect(normaliseBaseDomain(' interconnected.online ')).toBe('interconnected.online')
  })

  it('refuses what the engine refuses — a split-brain session otherwise', () => {
    for (const bad of [null, '', 'https://x.io', 'x.io/path', 'x.io:443', 'x io', 'localhost', '.x.io', 'x.io.', 'x..io', 'münchen.de']) {
      expect(normaliseBaseDomain(bad)).toBe(null)
    }
  })
})

describe('engineServiceOptions', () => {
  const load = async (onSites: boolean) => {
    vi.resetModules()
    if (onSites) (window as { __DCL_PUBLIC__?: unknown }).__DCL_PUBLIC__ = {}
    else delete (window as { __DCL_PUBLIC__?: unknown }).__DCL_PUBLIC__
    return import('../lib/baseDomain')
  }
  afterEach(() => {
    delete (window as { __DCL_PUBLIC__?: unknown }).__DCL_PUBLIC__
  })

  it('behind the gateway: every service by its gateway url, and no base domain the engine would refuse', async () => {
    const m = await load(true)
    const { SERVICES } = await import('../engine/generated')
    const options = m.engineServiceOptions()
    expect(window.location.host).toContain(':') // a dev host with a port, like 127.0.0.1:5222
    expect(options.baseDomain).toBeUndefined()
    for (const s of SERVICES) expect(options[s.name], s.name).toBe(m.serviceUrl(s.name))
    expect(options.realmProvider).toBe(`${window.location.origin}/realm-provider-ea`)
  })

  it('upstream: only the base domain, which the engine composes every service from', async () => {
    const m = await load(false)
    expect(m.engineServiceOptions()).toEqual({ baseDomain: 'decentraland.org' })
  })
})
