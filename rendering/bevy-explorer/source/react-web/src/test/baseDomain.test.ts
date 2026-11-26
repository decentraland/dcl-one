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

describe('?gateway=', () => {
  afterEach(() => {
    window.history.replaceState(null, '', '/')
  })

  it("routes the APIs the engine's gateway routes and gates a gateway outside Decentraland's deployments", async () => {
    window.history.replaceState(null, '', '/?gateway=https://gateway.example.test&places=https://places.decentraland.org')
    vi.resetModules()
    const m = await import('../lib/baseDomain')
    expect(m.serviceUrl('events')).toBe('https://gateway.example.test/events')
    expect(m.serviceUrl('places')).toBe('https://places.decentraland.org')
    expect(m.serviceUrl('storage')).toBe('https://storage.decentraland.org')
    const { untrustedLaunchParams } = await import('../lib/launchGate')
    expect(untrustedLaunchParams({ native: false }).map((p) => p.name)).toEqual(['gateway'])
    expect(m.nativeProfileImage('https://profile-images.decentraland.org/a%2Fb?q=1')).toBe(
      'https://gateway.example.test/profile-images/a%2Fb?q=1'
    )
    expect(m.nativeProfileImage('https://cdn.decentraland.org/a')).toBe('https://cdn.decentraland.org/a')
    const { engineLaunch } = await import('../lib/engineLaunch')
    expect(engineLaunch(new URLSearchParams(window.location.search)).options.gateway).toBe('https://gateway.example.test')
  })

  it("signs the path a gateway. host forwards, with the engine's service urls on native", async () => {
    // a native url carries the engine's resolved services and its gateway
    const places = 'https://gateway.decentraland.org/places'
    window.history.replaceState(null, '', `/?native=1&gateway=https://gateway.decentraland.org&places=${encodeURIComponent(places)}`)
    vi.resetModules()
    const m = await import('../lib/baseDomain')
    expect(m.serviceUrl('places')).toBe(places)
    expect(m.gatewaySignedPath(`${places}/api/places?limit=1`)).toBe('/api/places')
    expect(m.gatewaySignedPath('https://places.decentraland.org/api/places')).toBe(null)
    const { untrustedLaunchParams } = await import('../lib/launchGate')
    expect(untrustedLaunchParams({ native: true })).toEqual([])
  })

  it("trusts an https origin under Decentraland's deployments, never a path under one", async () => {
    for (const [gateway, untrusted] of [
      ['https://gateway.decentraland.org', []],
      ['https://gateway.decentraland.org/_scene/x', ['gateway']],
      ['http://gateway.decentraland.org', ['gateway']],
      ['https://gateway.decentraland.zone', ['gateway']],
      // the apex is the website, not a gateway
      ['https://decentraland.org', ['gateway']],
      // a value the parse refuses is still weighed
      ['ftp://gateway.decentraland.org', ['gateway']],
      ['https://gateway.decentraland.org/?', ['gateway']]
    ] as const) {
      window.history.replaceState(null, '', `/?gateway=${encodeURIComponent(gateway)}`)
      vi.resetModules()
      const { untrustedLaunchParams } = await import('../lib/launchGate')
      expect(untrustedLaunchParams({ native: false }).map((p) => p.name), gateway).toEqual(untrusted)
    }
  })
})
