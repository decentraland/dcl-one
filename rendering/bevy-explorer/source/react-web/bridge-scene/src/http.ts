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
  | 'places'

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

/** A signed request the service answered with an error status; keeps the status and body. */
export class HttpError extends Error {
  constructor(
    readonly status: number,
    readonly body: string,
    statusText?: string
  ) {
    super(`HTTP ${status}: ${statusText ?? body}`)
  }
}

// Catalyst/public GET. MUST go through the engine's kernelFetch, not the browser's fetch():
// the scene runs in the engine's COEP (cross-origin-isolated) context, where a raw fetch() to
// the catalyst is blocked (no CORP). kernelFetch performs the request natively in the engine.
export async function getJson<T>(url: string): Promise<T | undefined> {
  const r = await BevyApi.kernelFetch({
    url,
    init: { headers: { 'Content-Type': 'application/json' }, method: 'GET' },
    meta: META
  })
  if (!r.ok) throw new HttpError(r.status, r.body, r.statusText)
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
  if (!result.ok) throw new HttpError(result.status, result.body, result.statusText)
  if (!result.body) return undefined
  const parsed = JSON.parse(result.body) as { data?: T } & T
  return parsed.data ?? parsed
}

// Both scene runtimes provide TextEncoder (native: dcl_deno's init.js; web: the worker), but the
// scene SDK's lib types don't declare it.
declare const TextEncoder: { new (): { encode(input: string): Uint8Array } }

/** A binary file section of a multipart form. */
export type FormFile = { name: string; filename: string; contentType: string; bytes: Uint8Array }

const BASE64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'

/** The bytes of a base64 `data:` URL. The scene runtime has no global `atob` on native. */
export function dataUrlBytes(dataUrl: string): Uint8Array {
  const b64 = dataUrl.slice(dataUrl.indexOf(',') + 1).replace(/[^A-Za-z0-9+/]/g, '')
  const out = new Uint8Array(Math.floor((b64.length * 3) / 4))
  let n = 0
  for (let i = 0; i < b64.length; i += 4) {
    const a = BASE64.indexOf(b64[i])
    const b = BASE64.indexOf(b64[i + 1])
    const c = i + 2 < b64.length ? BASE64.indexOf(b64[i + 2]) : -1
    const d = i + 3 < b64.length ? BASE64.indexOf(b64[i + 3]) : -1
    out[n++] = (a << 2) | (b >> 4)
    if (c >= 0) out[n++] = ((b & 15) << 4) | (c >> 2)
    if (d >= 0) out[n++] = ((c & 3) << 6) | d
  }
  return out.subarray(0, n)
}

// Signed multipart/form-data POST/PUT: text fields, then optional binary file sections. The body
// is built as bytes, which the scene's fetch sends as-is on native (deno) and on web (the worker's
// fetch). The social-api community create/edit endpoints expect multipart.
export async function signedForm<T>(
  url: string,
  method: 'POST' | 'PUT',
  fields: Record<string, string>,
  files: FormFile[] = []
): Promise<T | undefined> {
  // a fresh boundary per request, so no field value or file can contain it by chance
  const boundary = `----DclBridgeFormBoundary${Math.random().toString(36).slice(2)}${Date.now().toString(36)}`
  const enc = new TextEncoder()
  const parts: Uint8Array[] = []
  for (const [key, value] of Object.entries(fields)) {
    parts.push(enc.encode(`--${boundary}\r\nContent-Disposition: form-data; name="${key}"\r\n\r\n${value}\r\n`))
  }
  for (const f of files) {
    parts.push(
      enc.encode(
        `--${boundary}\r\nContent-Disposition: form-data; name="${f.name}"; filename="${f.filename}"\r\n` +
          `Content-Type: ${f.contentType}\r\n\r\n`
      ),
      f.bytes,
      enc.encode('\r\n')
    )
  }
  parts.push(enc.encode(`--${boundary}--\r\n`))
  const body = new Uint8Array(parts.reduce((len, p) => len + p.length, 0))
  let at = 0
  for (const p of parts) {
    body.set(p, at)
    at += p.length
  }
  const result = await BevyApi.kernelFetch({
    url,
    init: { headers: { 'Content-Type': `multipart/form-data; boundary=${boundary}` }, method, body },
    meta: META
  })
  if (!result.ok) throw new HttpError(result.status, result.body, result.statusText)
  if (!result.body) return undefined
  const parsed = JSON.parse(result.body) as { data?: T } & T
  return parsed.data ?? parsed
}
