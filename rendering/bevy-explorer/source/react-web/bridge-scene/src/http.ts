// Shared data helpers: the service bases + two fetch primitives the domains build on.
//
// IMPORTANT: a system scene's `getRealm()` reports the scene's OWN realm (the local preview
// it was served from), NOT the world realm the engine is connected to — so it can't be used
// to locate the catalyst. Every service base comes from the ENGINE via
// `BevyApi.getServiceUrls()`: its override, else its composition from the base domain.
import { BevyApi } from './bevy-api'

/** The services the bridge calls, by their web param (crates/system_api_types/src/services.rs). */
export type ServiceName =
  | 'catalyst'
  | 'socialApi'
  | 'cameraReel'
  | 'events'
  | 'badges'
  | 'notifications'
  | 'assetsCdn'
  | 'marketplaceApi'

let services: Promise<Record<string, string>> | null = null

/** The engine's base url for a service (no trailing slash). */
export async function serviceBase(name: ServiceName): Promise<string> {
  services ??= BevyApi.getServiceUrls().catch((e: unknown) => {
    services = null
    throw e
  })
  const base = (await services)[name]
  if (base == null) throw new Error(`the engine reports no '${name}' service`)
  return base
}

/** Catalyst base for the engine's realm (lambdas/content live under here). */
export async function catalystBase(): Promise<string> {
  return serviceBase('catalyst')
}

const META = JSON.stringify({})

// Catalyst/public GET. MUST go through the engine's kernelFetch, not the browser's fetch():
// the scene runs in the engine's COEP (cross-origin-isolated) context, where a raw fetch() to
// the catalyst is blocked (no CORP). kernelFetch performs the request natively in the engine.
export async function getJson<T>(url: string): Promise<T | undefined> {
  const r = await BevyApi.kernelFetch({
    url,
    init: { headers: { 'Content-Type': 'application/json' }, method: 'GET' },
    meta: META
  })
  if (!r.ok) throw new Error(`HTTP ${r.status}: ${r.statusText ?? r.body}`)
  if (!r.body) return undefined
  return JSON.parse(r.body) as T
}

export async function signed<T>(
  url: string,
  method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' = 'GET',
  body?: object
): Promise<T | undefined> {
  const result = await BevyApi.kernelFetch({
    url,
    init: {
      headers: { 'Content-Type': 'application/json' },
      method,
      ...(body != null ? { body: JSON.stringify(body) } : {})
    },
    meta: META
  })
  if (!result.ok) throw new Error(`HTTP ${result.status}: ${result.statusText ?? result.body}`)
  if (!result.body) return undefined
  const parsed = JSON.parse(result.body) as { data?: T } & T
  return parsed.data ?? parsed
}

// Signed multipart/form-data POST/PUT with TEXT fields only (kernelFetch bodies are strings,
// so binary file parts like thumbnails can't go through here). The social-api community
// create/edit endpoints expect multipart, which is valid with text-only sections.
export async function signedForm<T>(
  url: string,
  method: 'POST' | 'PUT',
  fields: Record<string, string>
): Promise<T | undefined> {
  const boundary = '----DclBridgeFormBoundaryqZ1x9b7Kt3'
  let body = ''
  for (const [key, value] of Object.entries(fields)) {
    body += `--${boundary}\r\nContent-Disposition: form-data; name="${key}"\r\n\r\n${value}\r\n`
  }
  body += `--${boundary}--\r\n`
  const result = await BevyApi.kernelFetch({
    url,
    init: { headers: { 'Content-Type': `multipart/form-data; boundary=${boundary}` }, method, body },
    meta: META
  })
  if (!result.ok) throw new Error(`HTTP ${result.status}: ${result.statusText ?? result.body}`)
  if (!result.body) return undefined
  const parsed = JSON.parse(result.body) as { data?: T } & T
  return parsed.data ?? parsed
}
