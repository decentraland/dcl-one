// The HUD's gateway resolver over the engine's generated service table (crates/common/src/gateway.rs
// rules). The endpoint equality with the engine's own resolver is the cross-language contract test
// (infra/gateway/tests/explorer_routes.py); these cases pin the parse and profile-image edges.
import { describe, expect, it } from 'vitest'
import { SERVICES } from '../engine/generated/serviceTable'
import { gatewayProfileImage, gatewayServiceUrl, isTrustedGatewayPrefix, parseGateway } from '../lib/gateway'

const service = (name: string) => SERVICES.find((s) => s.name === name)!

describe('parseGateway', () => {
  it('keeps an http(s) base url without its trailing slash; a gateway. host is the virtual host', () => {
    expect(parseGateway(' https://Gateway.Example.test/mount/ ')).toEqual({
      prefix: 'https://gateway.example.test/mount',
      virtualHost: true
    })
    expect(parseGateway('http://localhost:5222')).toEqual({ prefix: 'http://localhost:5222', virtualHost: false })
  })

  it('refuses what the engine refuses', () => {
    for (const raw of [null, '', 'ftp://host', 'https://user@host', 'https://u:p@host', 'https://host/?', 'https://host/#', 'host', '/mount']) {
      expect(parseGateway(raw), String(raw)).toBe(null)
    }
  })
})

describe('gatewayServiceUrl', () => {
  const vhost = parseGateway('https://gateway.example.test/mount')!
  const direct = parseGateway('http://localhost:5222')!

  it('routes HTTP APIs by host label, with the direct origin aliases', () => {
    expect(gatewayServiceUrl(vhost, service('catalyst'))).toBe('https://gateway.example.test/mount/peer')
    expect(gatewayServiceUrl(direct, service('catalyst'))).toBe('http://localhost:5222')
    expect(gatewayServiceUrl(vhost, service('previewGatekeeper'))).toBe('https://gateway.example.test/mount/comms-gatekeeper-local')
    expect(gatewayServiceUrl(direct, service('previewGatekeeper'))).toBe('http://localhost:5222/comms-gatekeeper')
    expect(gatewayServiceUrl(direct, service('assetsCdn'))).toBe('http://localhost:5222/social-api')
    expect(gatewayServiceUrl(vhost, service('featureFlags'))).toBe('https://gateway.example.test/mount/feature-flags')
  })

  it('leaves sockets, Pulse, storage, abgen and the site pages on their own destinations', () => {
    for (const name of ['ethereumRpc', 'socialRpc', 'pulseServer', 'storage', 'abgenCdn', 'reels']) {
      expect(gatewayServiceUrl(vhost, service(name)), name).toBe(null)
      expect(gatewayServiceUrl(direct, service(name)), name).toBe(null)
    }
  })
})

describe('gatewayProfileImage', () => {
  const gateway = parseGateway('https://gateway.example.test/prefix')!

  it('rewrites only the literal profile-images host of the base domain', () => {
    expect(gatewayProfileImage(gateway, 'https://PROFILE-images.example.test/avatar%2Fface?q=x', 'example.test')).toBe(
      'https://gateway.example.test/prefix/profile-images/avatar%2Fface?q=x'
    )
    for (const raw of [
      'https://profile-images.example.test/bad%2f..%2f..%2fauth',
      'https://profile-images.example.test:443/i',
      'http://profile-images.example.test/i',
      'https://user@profile-images.example.test/i',
      'https://profile-images.example.test.evil/i',
      'https://cdn.example.test/i',
      'https://gateway.example.test/prefix/profile-images/i'
    ]) {
      expect(gatewayProfileImage(gateway, raw, 'example.test'), raw).toBe(null)
    }
  })
})

describe('isTrustedGatewayPrefix', () => {
  it("trusts the page's origin and an https origin of the session's Decentraland deployment", () => {
    expect(isTrustedGatewayPrefix('https://gateway.decentraland.org', 'decentraland.org')).toBe(true)
    expect(isTrustedGatewayPrefix('https://decentraland.zone', 'decentraland.zone')).toBe(true)
    expect(isTrustedGatewayPrefix('http://node.example.test', 'node.example.test', 'http://node.example.test')).toBe(true)
    for (const [prefix, domain] of [
      ['https://gateway.decentraland.org/_scene/x', 'decentraland.org'],
      ['https://gateway.decentraland.org:8443', 'decentraland.org'],
      ['http://gateway.decentraland.org', 'decentraland.org'],
      ['https://gateway.decentraland.zone', 'decentraland.org'],
      ['https://evil-decentraland.org', 'decentraland.org'],
      ['https://gateway.example.test', 'example.test']
    ]) {
      expect(isTrustedGatewayPrefix(prefix, domain), prefix).toBe(false)
    }
    expect(isTrustedGatewayPrefix('http://node.example.test/_scene/x', 'node.example.test', 'http://node.example.test')).toBe(false)
  })
})
