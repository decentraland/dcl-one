// Results of saved items, persisted across reloads in IndexedDB. Keys are built from content hashes plus
// everything else the checks read, so an entry never goes stale; the stamp drops every entry at once when the
// rule book or our mapping of its findings changes.
import { clear, createStore, del, get, set, type UseStore } from 'idb-keyval'
import { type ValidationResult } from './types'

/** Bump when the mapping from rule book findings to issues changes, so old entries are never read again. */
export const CACHE_REVISION = 2

const STAMP_KEY = 'stamp'
const STAMP = JSON.stringify([CACHE_REVISION, __VALIDATOR_VERSION__])

let ready: Promise<UseStore | null> | null = null

function open(): Promise<UseStore | null> {
  ready ??= (async () => {
    // Private windows and blocked site data: run without persistence.
    if (typeof indexedDB === 'undefined') return null
    try {
      const store = createStore('item-validation', 'results')
      if ((await get<string>(STAMP_KEY, store)) !== STAMP) {
        await clear(store)
        await set(STAMP_KEY, STAMP, store)
      }
      return store
    } catch {
      return null
    }
  })()
  return ready
}

export async function readCached(key: string): Promise<ValidationResult | undefined> {
  try {
    const store = await open()
    return store ? await get<ValidationResult>(key, store) : undefined
  } catch {
    return undefined
  }
}

export async function writeCached(key: string, result: ValidationResult): Promise<void> {
  try {
    const store = await open()
    if (store) await set(key, result, store)
  } catch {
    // Quota or a closed connection: the result is only lost for the next reload.
  }
}

export async function deleteCached(keys: string[]): Promise<void> {
  try {
    const store = await open()
    if (store) await Promise.all(keys.map(key => del(key, store)))
  } catch {
    // Nothing to drop.
  }
}

/** Specs only: forget the opened store so the next access re-reads the stamp. */
export function resetCacheConnection(): void {
  ready = null
}

/**
 * The persisted entry's key for a react-query key. The stamp is part of it so a tab still running an older
 * rule book never writes results a newer tab would read, even after that tab re-stamped the store.
 */
export function cacheKey(queryKey: readonly unknown[]): string {
  return JSON.stringify([STAMP, queryKey])
}

/**
 * Serves `run` from the store when `queryKey` is given and a result is there, persisting a fresh one. A run
 * that throws is never stored. `null` skips persistence (in-memory sources).
 */
export async function cachedRun(
  queryKey: readonly unknown[] | null,
  run: () => Promise<ValidationResult>
): Promise<ValidationResult> {
  if (!queryKey) return run()
  const key = cacheKey(queryKey)
  const hit = await readCached(key)
  if (hit) return hit
  const result = await run()
  await writeCached(key, result)
  return result
}
