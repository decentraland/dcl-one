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
    const { SERVICES } = await import('@explorer-core/engine/generated')
    const options = m.engineServiceOptions()
    expect(window.location.host).toContain(':') // a dev host with a port, like 127.0.0.1:5222
    expect(options.baseDomain).toBeUndefined()
    for (const s of SERVICES) expect(options[s.name], s.name).toBe(m.serviceUrl(s.name))
    expect(options.realmProvider).toBe(`${window.location.origin}/realm-provider-ea`)
    expect(options.reels).toBe(`${window.location.origin}/reels`)
    // not through the gateway yet (#631): Decentraland's public host, never one built from the
    // page's own host and port
    expect(options.abgenCdn).toBe('https://abgen-cdn.decentraland.org')
  })

  it('upstream: only the base domain, which the engine composes every service from', async () => {
    const m = await load(false)
    expect(m.engineServiceOptions()).toEqual({ baseDomain: 'decentraland.org' })
  })
})

describe('the Pulse host behind the gateway', () => {
  const load = async (pulseServer: string | undefined, search = '/') => {
    vi.resetModules()
    window.__DCL_PUBLIC__ = pulseServer == null ? {} : { pulseServer }
    window.history.replaceState(null, '', search)
    return import('../lib/baseDomain')
  }
  afterEach(() => {
    delete window.__DCL_PUBLIC__
    window.history.replaceState(null, '', '/')
    vi.restoreAllMocks()
  })

  it('is the host the deployment names', async () => {
    const m = await load('Pulse.example.test:7743')
    expect(m.engineServiceOptions().pulseServer).toBe('pulse.example.test:7743')
  })

  it.each([undefined, ''])('is the page host, silently, when the deployment names none (%j)', async (pulseServer) => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    const m = await load(pulseServer)
    expect(m.engineServiceOptions().pulseServer).toBe(window.location.host)
    expect(warn).not.toHaveBeenCalled()
  })

  it('is the page host, with a warning, when the deployment names an invalid one', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    const m = await load('https://pulse.example.test')
    expect(m.engineServiceOptions().pulseServer).toBe(window.location.host)
    expect(warn).toHaveBeenCalledOnce()
  })

  it('yields to an entry-url override', async () => {
    const m = await load('pulse.example.test', '/?pulseServer=other.example.test')
    expect(m.engineServiceOptions().pulseServer).toBe('other.example.test')
  })

  it('ignores a page-host value the engine wrote back into the address bar', async () => {
    const m = await load('pulse.example.test', `/?pulseServer=${window.location.host}`)
    expect(m.engineServiceOptions().pulseServer).toBe('pulse.example.test')
  })

  it('plays no part under ?baseDomain=, where Pulse composes from the base domain', async () => {
    const m = await load('pulse.example.test', '/?baseDomain=decentraland.org')
    expect(m.serviceUrl('pulseServer')).toBe('pulse-server.decentraland.org')
  })

  it('keeps a page-host override under ?baseDomain=', async () => {
    const m = await load('pulse.example.test', `/?baseDomain=decentraland.org&pulseServer=${window.location.host}`)
    expect(m.serviceUrl('pulseServer')).toBe(window.location.host)
  })
})
