// The `?gateway=` launch option: the parse and service mapping of the engine's
// `common::gateway::Gateway` (crates/common/src/gateway.rs), over its generated service table.

import type { ServiceDef } from '../engine/generated/index'

export interface Gateway {
  /** The base url, without a trailing slash. */
  prefix: string
  /** A `gateway.` host: services under their host label, signatures over the forwarded path. */
  virtualHost: boolean
}

/**
 * An http(s) base url with a host and no credentials, query or fragment (`Gateway::parse`);
 * anything else is null.
 */
export function parseGateway(raw: string | null): Gateway | null {
  if (raw == null) return null
  const trimmed = raw.trim()
  // a bare trailing `?` or `#` counts as a query or fragment
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

/** The base domains of Decentraland's own deployments. */
export const DECENTRALAND_DOMAINS: readonly string[] = ['decentraland.org', 'decentraland.zone']

/**
 * Whether `prefix` is trusted for a session on `baseDomain` (`Gateway::parse_link`): `ownOrigin`
 * when its host is outside Decentraland's domains, or `https://gateway.<baseDomain>` with no port
 * or path when `baseDomain` is one of Decentraland's own deployments. The apex and any path are
 * never trusted.
 */
export function isTrustedGatewayPrefix(prefix: string, baseDomain: string, ownOrigin: string | null = null): boolean {
  const url = new URL(prefix)
  if (prefix === ownOrigin && !isDecentralandHost(url.hostname)) return true
  return (
    url.protocol === 'https:' &&
    url.port === '' &&
    url.pathname === '/' &&
    DECENTRALAND_DOMAINS.includes(baseDomain) &&
    url.hostname === `gateway.${baseDomain}`
  )
}

/** The untrusted-launch warning for `?gateway=`: one text for a parsed gateway, another for a refused value. */
export function gatewayLaunchWarning(gateway: Gateway | null): string {
  return gateway != null
    ? 'Sends sign-in, profiles, comms admission and every other Decentraland API request through this server. Whoever runs it would see your signed requests and choose the answers.'
    : 'Is not an http(s) base url without credentials, query or fragment, so the Explorer ignores it and reaches every service directly. Check the link if you meant to use a gateway.'
}

/**
 * The `?gateway=` launch value: the parsed prefix, or the trimmed raw value `parseGateway`
 * refused; null when absent or empty.
 */
export function gatewayLaunchValue(raw: string | null, gateway: Gateway | null): string | null {
  if (gateway != null) return gateway.prefix
  const trimmed = raw?.trim() ?? ''
  return trimmed === '' ? null : trimmed
}

/** Whether that value is a parsed gateway that `isTrustedGatewayPrefix` accepts. */
export function isTrustedGatewayLaunchValue(
  value: string,
  gateway: Gateway | null,
  baseDomain: string,
  ownOrigin: string | null = null
): boolean {
  return gateway != null && value === gateway.prefix && isTrustedGatewayPrefix(value, baseDomain, ownOrigin)
}

/**
 * The service's base url through the gateway (`Gateway::service`), or null when the service
 * table gives it no segment (`Service::gateway_segment`).
 */
export function gatewayServiceUrl(gateway: Gateway, s: ServiceDef): string | null {
  const segment = gateway.virtualHost ? s.gatewayVirtualHost : s.gatewayDirect
  if (segment == null) return null
  return segment === '' ? gateway.prefix : `${gateway.prefix}/${segment}`
}

/**
 * A backend-supplied profile image under `base` (`Gateway::profile_image`): only an
 * `https://profile-images.<domain>` url with that literal authority and no dot segment once its
 * escaped separators are decoded; anything else is null.
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
 * The path after the service alias of `raw` under a `gateway.` virtual host
 * (`Gateway::original_path`); null for an apex gateway, another host, or a path under no
 * `serviceBases` url. `serviceBases` are the resolved service urls. Throws on a url without `//`, a
 * backslash, a path URL parsing changes, or a dot segment once escaped separators are decoded.
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
  // compare the raw path with the parsed one: URL resolves dot segments
  const sep = raw.indexOf('://')
  if (sep < 0) throw new Error('ambiguous gateway signing path')
  const authorityAndPath = raw.slice(sep + 3)
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
  // strip the first segment only, and only when a slash follows it
  const cut = rest.indexOf('/')
  return `/${cut < 0 ? rest : rest.slice(cut + 1)}`
}

function isDecentralandHost(host: string): boolean {
  // a trailing DNS dot names the same host
  const h = host.replace(/\.$/, '')
  return DECENTRALAND_DOMAINS.some((d) => h === d || h.endsWith(`.${d}`))
}

// A dot segment, after decoding escaped separators and dots.
function hasProxyDotSegment(path: string): boolean {
  const decoded = path.toLowerCase().replace(/%2f|%5c/g, '/').replace(/%2e/g, '.')
  return decoded.split('/').some((part) => part === '.' || part === '..')
}
