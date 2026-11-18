// Item sizes on screen (lib/itemSize): stored files measured by content hash through the query cache, and the item
// a save would send measured before it is sent, so what the editor shows is what the save enforces.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useQueries, useQueryClient, type QueryClient, type UseQueryResult } from '@tanstack/react-query'
import { fetchContent, fetchContentSize } from '~/lib/builder'
import { buildMissingImage, catalystImageDeps, type ImageDeps } from '~/lib/catalystEntity'
import { ItemFileError } from '~/lib/itemFiles'
import {
  MAX_CONCURRENT_MEASUREMENTS,
  MEASUREMENT_TIMEOUT_MS,
  measureItem,
  measureItemBytes,
  type ItemSize,
  type MeasurableItem,
  type MeasureDeps
} from '~/lib/itemSize'
import { type Item } from '~/lib/items'
import { createLimiter } from '~/lib/limiter'
import { captureError } from '~/lib/monitoring'

const CONTENT_SIZE = 'content-size'
const contentSizeKey = (hash: string) => [CONTENT_SIZE, hash] as const

// One queue for every measurement in the app, so a whole collection's sizes never open dozens of requests together.
const limited = createLimiter(MAX_CONCURRENT_MEASUREMENTS)

/**
 * Measuring for the app: a content hash names bytes that never change, so each file's size is read once (or taken from
 * the upload or render that produced it) and kept for the session; a standard item saved without image.png gets the one
 * the approval flow would render for it at deploy.
 * Requests wait their turn in one queue and each gets `timeoutMs` once it starts: a stalled one fails (and blocks
 * nothing) instead of staying pending.
 */
export function itemSizeDeps(
  queryClient: QueryClient,
  { timeoutMs = MEASUREMENT_TIMEOUT_MS }: { timeoutMs?: number } = {}
): MeasureDeps {
  queryClient.setQueryDefaults([CONTENT_SIZE], { staleTime: Infinity, gcTime: Infinity })
  const imageDeps: ImageDeps = {
    ...catalystImageDeps,
    fetchContent: hash => limited(() => fetchContent(hash, AbortSignal.timeout(timeoutMs)))
  }
  return {
    sizeOf: hash =>
      queryClient.fetchQuery({
        queryKey: contentSizeKey(hash),
        queryFn: () => limited(() => fetchContentSize(hash, AbortSignal.timeout(timeoutMs))),
        retry: false
      }),
    remember: (hash, bytes) => queryClient.setQueryData(contentSizeKey(hash), bytes),
    imageOf: item => (item.contents[item.thumbnail] ? buildMissingImage(item, imageDeps) : Promise.resolve(null))
  }
}

export function useItemSizeDeps(): MeasureDeps {
  const queryClient = useQueryClient()
  return useMemo(() => itemSizeDeps(queryClient), [queryClient])
}

const sameInputs = (a: readonly unknown[], b: readonly unknown[]) =>
  a.length === b.length && a.every((value, index) => Object.is(value, b[index]))

/**
 * The bytes of the item `build` resolves to, counted as the catalyst counts them: what a save would send, measured
 * before it is sent; null while unknown. It measures again when one of `inputs` changes, compared by reference, so
 * they list the very objects the files come from. A build that can't be made yet (an ItemFileError) is null; a
 * measurement that fails is reported and null. Nothing is blocked on it: the save checks again.
 */
export function useMeasuredBytes(
  build: (() => Promise<MeasurableItem | null>) | null,
  inputs: readonly unknown[],
  flow: string
): number | null {
  const deps = useItemSizeDeps()
  const [current, setCurrent] = useState(inputs)
  if (!sameInputs(current, inputs)) setCurrent(inputs)
  const buildRef = useRef(build)
  useEffect(() => {
    buildRef.current = build
  })
  const enabled = build !== null
  const [result, setResult] = useState<{ inputs: readonly unknown[]; bytes: number | null } | null>(null)

  useEffect(() => {
    const run = buildRef.current
    if (!run) return
    let cancelled = false
    void run()
      .then(built => (built ? measureItemBytes(built, deps) : null))
      .catch((error: unknown) => {
        if (!(error instanceof ItemFileError)) captureError(error, { flow, step: 'measure_size' })
        return null
      })
      .then(bytes => {
        if (!cancelled) setResult({ inputs: current, bytes })
      })
    return () => {
      cancelled = true
    }
  }, [current, enabled, deps, flow])

  return enabled && result?.inputs === current ? result.bytes : null
}

/** A saved item's size key: its stored files plus what decides its limit and a missing rarity image. */
const itemSizeKey = (item: Item) =>
  [
    'item-size',
    item.id,
    item.type,
    item.data.category ?? null,
    item.rarity ?? null,
    item.thumbnail,
    Object.entries(item.contents).sort(([a], [b]) => a.localeCompare(b))
  ] as const

export type ItemSizes = {
  /** By item id; an item in neither this nor `unknown` is still being measured. */
  sizes: Map<string, ItemSize>
  /** The ids of the items whose size could not be measured. */
  unknown: Set<string>
  isMeasuring: boolean
  /** Measures the item again: a failed measurement is not retried on its own. */
  remeasure: (item: Item) => Promise<void>
}

/**
 * The saved items' sizes as the catalyst counts them, measured once `enabled`. A measurement that fails is reported
 * and leaves the item in `unknown`, never at a made-up size.
 */
export function useCatalystItemSizes(items: Item[], { enabled = true }: { enabled?: boolean } = {}): ItemSizes {
  const queryClient = useQueryClient()
  const deps = useItemSizeDeps()
  const queries = useMemo(
    () =>
      items.map(item => ({
        queryKey: itemSizeKey(item),
        queryFn: () =>
          measureItem({ item }, deps).catch((error: unknown) => {
            captureError(error, { flow: 'item_size', itemId: item.id })
            throw error
          }),
        enabled,
        staleTime: Infinity,
        retry: false
      })),
    [items, deps, enabled]
  )
  // Stable, so the combined result keeps its identity until a size actually changes.
  const remeasure = useCallback(
    (item: Item) => queryClient.refetchQueries({ queryKey: itemSizeKey(item), exact: true }),
    [queryClient]
  )
  const combine = useCallback(
    (results: UseQueryResult<ItemSize>[]): ItemSizes => {
      const sizes = new Map<string, ItemSize>()
      const unknown = new Set<string>()
      results.forEach((result, index) => {
        const id = items[index]?.id
        if (!id) return
        if (result.data) sizes.set(id, result.data)
        else if (result.isError) unknown.add(id)
      })
      return { sizes, unknown, isMeasuring: enabled && results.some(result => result.isPending), remeasure }
    },
    [items, enabled, remeasure]
  )
  return useQueries({ queries, combine })
}
