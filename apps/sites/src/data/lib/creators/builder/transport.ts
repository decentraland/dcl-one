/**
 * Client for the builder-api: the node serving the page, or the environment's on a Decentraland host.
 * Requests are signed through the lib/auth chokepoint; response bodies are the `{ ok, data }` envelope.
 */
import { catalystUrl } from '../../catalyst/bases'
import { config } from '../config'
import { isDecentralandHost } from '../config/env'
import { signedFetch } from '../auth'

/** Carries the HTTP status so callers can tell "no access / gone" from a transient failure. */
export class BuilderServerError extends Error {
  status: number
  /** The error envelope's `data`, when the server sent one. */
  data?: unknown

  constructor(message: string, status: number, data?: unknown) {
    super(message)
    this.name = 'BuilderServerError'
    this.status = status
    this.data = data
  }
}

/**
 * builder-server refusing what was sent (a 4xx): sending it again fails the same way, and it's the user's to fix.
 * Anything else is transient, or something someone must look at.
 */
export function isRefusedRequest(error: unknown): boolean {
  return error instanceof BuilderServerError && error.status >= 400 && error.status < 500
}

/** The builder-api: the environment's on a Decentraland host, else that of the node serving the page. */
export const baseUrl = (host = typeof window === 'undefined' ? '' : window.location.host) =>
  isDecentralandHost(host) ? config.get('BUILDER_SERVER_URL') : `${catalystUrl()}/builder-api/v1`

export const UNPUBLISHED_COLLECTION_STATUS = 409

export const COLLECTION_LOCKED_STATUS = 423
export const ALREADY_PUBLISHED_STATUS = 409

export async function request<T>(
  address: string | undefined,
  method: string,
  path: string,
  query = '',
  body?: unknown,
  expectData = true
): Promise<T> {
  const init: RequestInit = { method }
  if (body instanceof FormData) {
    init.body = body
  } else if (body !== undefined) {
    init.body = JSON.stringify(body)
    init.headers = { 'Content-Type': 'application/json' }
  }
  const response = await signedFetch(address, baseUrl(), `${path}${query}`, init)
  const parsed = (await response.json().catch(() => {
    throw new BuilderServerError(
      `builder-server returned a non-JSON response: ${method} ${path} (${response.status})`,
      response.status
    )
  })) as { ok?: boolean; data?: T; error?: string }
  if (!response.ok || parsed.ok === false || (expectData && parsed.data === undefined)) {
    throw new BuilderServerError(
      parsed.error ?? `builder-server request failed: ${method} ${path} (${response.status})`,
      response.status,
      parsed.data
    )
  }
  return parsed.data as T
}
