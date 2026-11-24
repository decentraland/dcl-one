// The `?gateway=` launch option as both HUD hosts resolve it for their own fetches: the engine's
// `common::gateway::Gateway` (crates/common/src/gateway.rs), the same parse and the same service
// mapping, so the page and its engine reach the same endpoints. The engine takes the gateway as
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

// Kept out of the gateway: site links and storage delegations use their own public destinations;
// abgen's CDN is refused by the gateway's `/ab-cdn` route.
const OWN_DESTINATION: ReadonlySet<string> = new Set(['authPage', 'reels', 'storage', 'abgenCdn'])

/** The service's base url through the gateway (`Gateway::service`), or null when it keeps its own destination. */
export function gatewayServiceUrl(gateway: Gateway, s: ServiceDef): string | null {
  if (s.value !== 'http' || OWN_DESTINATION.has(s.name)) return null
  // a direct origin serves the peer at its root, preview on the main gatekeeper and community
  // thumbnails through social
  const direct: Record<string, string> = { catalyst: '', previewGatekeeper: 'comms-gatekeeper', assetsCdn: 'social-api' }
  const segment = !gateway.virtualHost && s.name in direct ? direct[s.name] : s.sub
  return segment === '' ? gateway.prefix : `${gateway.prefix}/${segment}`
}

/**
 * A backend-supplied profile image through the gateway (`Gateway::profile_image`): only an
 * `https://profile-images.<domain>` url with that literal authority and no dot segment after the
 * proxy decodes separators; anything else is null and loads as given.
 */
export function gatewayProfileImage(gateway: Gateway, raw: string, domain: string): string | null {
  const authority = /^https:\/\/([^/?#]*)/i.exec(raw)?.[1]
  if (authority?.toLowerCase() !== `profile-images.${domain}`) return null
  try {
    const image = new URL(raw)
    const decoded = image.pathname.toLowerCase().replace(/%2f|%5c/g, '/').replace(/%2e/g, '.')
    if (decoded.split('/').some((part) => part === '.' || part === '..')) return null
    return `${gateway.prefix}/profile-images${image.href.slice(image.origin.length)}`
  } catch {
    return null
  }
}
