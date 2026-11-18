import { type Item } from '~/lib/items'
import { BuilderServerError, baseUrl } from './transport'

export const getContentsStorageUrl = (hash = '') => `${baseUrl()}/storage/contents/${hash}`

/**
 * A stored file's size in bytes. The bucket's CORS allows only GET, so this reads the headers of a GET and cancels it
 * before the body downloads; a response that doesn't say its length is read to the end instead, since item sizes are
 * enforced with this and a 0 would let an item through. `signal` (a timeout) cancels the request and the body read.
 */
export async function fetchContentSize(hash: string, signal?: AbortSignal): Promise<number> {
  const controller = new AbortController()
  const cancel = () => controller.abort(signal?.reason)
  if (signal?.aborted) cancel()
  signal?.addEventListener('abort', cancel, { once: true })
  try {
    // no-cache for the same reason as fetchContent: a cached copy from an <img> has no CORS headers.
    const response = await fetch(getContentsStorageUrl(hash), { cache: 'no-cache', signal: controller.signal })
    const length = Number(response.headers.get('content-length'))
    if (!response.ok || length > 0) controller.abort()
    if (!response.ok) throw new BuilderServerError(`Could not read ${hash} (${response.status})`, response.status)
    return length > 0 ? length : (await response.blob()).size
  } finally {
    signal?.removeEventListener('abort', cancel)
  }
}

/** One stored file by hash, from public storage. */
export async function fetchContent(hash: string, signal?: AbortSignal): Promise<Blob> {
  // Storage omits `Vary: Origin`, so a file the page already showed in an <img> sits in the HTTP cache without
  // CORS headers and a plain cached fetch of it fails. Revalidating gets a response with them.
  const response = await fetch(getContentsStorageUrl(hash), { signal, cache: 'no-cache' })
  if (!response.ok) {
    await response.body?.cancel()
    throw new BuilderServerError(`Could not download ${hash} (${response.status})`, response.status)
  }
  return response.blob()
}

/** Downloads every file of a saved item from public storage, keyed by path, for the local preview/editor. */
export async function fetchItemContents(item: Item): Promise<Record<string, Blob>> {
  const entries = await Promise.all(
    Object.entries(item.contents).map(async ([path, hash]) => [path, await fetchContent(hash)] as const)
  )
  return Object.fromEntries(entries)
}
