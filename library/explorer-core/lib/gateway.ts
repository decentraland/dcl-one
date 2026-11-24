// The `?gateway=` launch option as both HUD hosts resolve it for their own fetches: the engine's
// `common::gateway::Gateway` (crates/common/src/gateway.rs), the same parse and the service
// mapping its generated service table carries, so the page and its engine reach the same endpoints. The engine takes the gateway as
// its `gateway` launch option and signs the paths the gateway forwards.

import type { ServiceDef } from '../engine/generated/index'

export interface Gateway {
  /** The base url, without a trailing slash. */
  prefix: string
  /** A `gateway.` host: services under their host label, signatures over the forwarded path. */
  virtualHost: boolean
}

/**
 * The gateway the engine will accept (`Gateway::parse`): an http(s) base url with a host and no
 * credentials, query or fragment. Anything else is null, so the page and the engine never split.
 */
export function parseGateway(raw: string | null): Gateway | null {
  if (raw == null) return null
  const trimmed = raw.trim()
  // a bare trailing `?` or `#` is a query or fragment to the engine's parser
  if (/[?#]/.test(trimmed)) return null
  let url: URL
  try {
    url = new URL(trimmed)
  } catch {
    return null
  }
  if (!/^https?:$/.test(url.protocol) || url.hostname === '' || url.username !== '' || url.password !== '') return null
  return { prefix: url.href.replace(/\/+$/, ''), virtualHost: url.hostname.startsWith('gateway.') }
}

/** Decentraland's own deployments: the only base domains a link may select a gateway for. */
export const DECENTRALAND_DOMAINS: readonly string[] = ['decentraland.org', 'decentraland.zone']

/**
 * Whether a link may select this gateway with no prompt, for a session on `baseDomain`
 * (`Gateway::parse_link`): the page's own origin, or `https://gateway.<baseDomain>` or
 * `https://<baseDomain>` with no port or path when that is one of Decentraland's own deployments.
 * A path is never trusted: a node proxies other hosts under one (`/_scene/<host>`) with the
 * request's signature headers.
 */
export function isTrustedGatewayPrefix(prefix: string, baseDomain: string, ownOrigin: string | null = null): boolean {
  if (prefix === ownOrigin) return true
  const url = new URL(prefix)
  return (
    url.protocol === 'https:' &&
    url.port === '' &&
    url.pathname === '/' &&
    DECENTRALAND_DOMAINS.includes(baseDomain) &&
    (url.hostname === baseDomain || url.hostname === `gateway.${baseDomain}`)
  )
}

/**
 * The service's base url through the gateway (`Gateway::service`), or null when it keeps its own
 * destination. The segments come from the engine's service table (`Service::gateway_segment`).
 */
export function gatewayServiceUrl(gateway: Gateway, s: ServiceDef): string | null {
  const segment = gateway.virtualHost ? s.gatewayVirtualHost : s.gatewayDirect
  if (segment == null) return null
  return segment === '' ? gateway.prefix : `${gateway.prefix}/${segment}`
}

/**
 * A backend-supplied profile image under `base` (`Gateway::profile_image`): only an
 * `https://profile-images.<domain>` url with that literal authority and no dot segment after the
 * proxy decodes separators; anything else is null and loads as given.
 */
export function profileImageUnder(base: string, raw: string, domain: string): string | null {
  const authority = /^https:\/\/([^/?#]*)/i.exec(raw)?.[1]
  if (authority?.toLowerCase() !== `profile-images.${domain}`) return null
  try {
    const image = new URL(raw)
    if (hasProxyDotSegment(image.pathname)) return null
    return base + image.href.slice(image.origin.length)
  } catch {
    return null
  }
}

/** A backend-supplied profile image through the gateway's `/profile-images`, or null (`profileImageUnder`). */
export function gatewayProfileImage(gateway: Gateway, raw: string, domain: string): string | null {
  return profileImageUnder(`${gateway.prefix}/profile-images`, raw, domain)
}

/**
 * The path a signed request to `raw` signs through a `gateway.` virtual host, which forwards the
 * path after the service alias (`Gateway::original_path`); null signs the url's own path (an apex
 * gateway, another host, or no service base under the gateway). `serviceBases` are the resolved
 * service urls. Throws on a path the proxy could read differently, never signing a normalised one.
 */
export function gatewaySignedPath(gateway: Gateway, raw: string, serviceBases: Iterable<string>): string | null {
  if (!gateway.virtualHost) return null
  let target: URL
  try {
    target = new URL(raw)
  } catch {
    return null
  }
  if (!gateway.prefix.startsWith(target.origin)) return null
  const mount = gateway.prefix.slice(target.origin.length)
  if (mount !== '' && !mount.startsWith('/')) return null
  // the browser resolves dot segments before sending, and the proxy decodes separators first
  const authorityAndPath = raw.slice(raw.indexOf('://') + 3)
  const publicTarget = authorityAndPath.split(/[?#]/)[0]
  const slash = publicTarget.indexOf('/')
  const rawPath = slash < 0 ? '/' : publicTarget.slice(slash)
  if (publicTarget.includes('\\') || rawPath !== target.pathname || hasProxyDotSegment(rawPath)) {
    throw new Error('ambiguous gateway signing path')
  }
  if (!rawPath.startsWith(`${mount}/`)) return null
  const rest = rawPath.slice(mount.length + 1)
  // a base at the mount itself has no service alias to strip
  const underService = [...serviceBases].some((base) => {
    if (!base.startsWith(`${gateway.prefix}/`)) return false
    const route = base.slice(gateway.prefix.length + 1)
    return route !== '' && rest.startsWith(route) && (rest.length === route.length || rest[route.length] === '/')
  })
  if (!underService) return null
  // the proxy strips only the first segment, and only when a slash follows it
  const cut = rest.indexOf('/')
  return `/${cut < 0 ? rest : rest.slice(cut + 1)}`
}

// The proxy decodes escaped separators before normalizing its path.
function hasProxyDotSegment(path: string): boolean {
  const decoded = path.toLowerCase().replace(/%2f|%5c/g, '/').replace(/%2e/g, '.')
  return decoded.split('/').some((part) => part === '.' || part === '..')
}
