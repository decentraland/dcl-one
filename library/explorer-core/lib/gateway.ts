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
    const decoded = image.pathname.toLowerCase().replace(/%2f|%5c/g, '/').replace(/%2e/g, '.')
    if (decoded.split('/').some((part) => part === '.' || part === '..')) return null
    return base + image.href.slice(image.origin.length)
  } catch {
    return null
  }
}

/** A backend-supplied profile image through the gateway's `/profile-images`, or null (`profileImageUnder`). */
export function gatewayProfileImage(gateway: Gateway, raw: string, domain: string): string | null {
  return profileImageUnder(`${gateway.prefix}/profile-images`, raw, domain)
}
