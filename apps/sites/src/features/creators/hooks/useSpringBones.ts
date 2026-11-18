import { useMemo } from 'react'
import { useQueries, type QueryClient } from '@tanstack/react-query'
import { fetchContent } from '~/lib/builder'
import { type Item } from '~/lib/items'
import {
  getInitialSpringBoneParams,
  getRepresentationModelHashes,
  hasSpringBones,
  isSpringBoneItem,
  parseSpringBones,
  type BoneNode,
  type SpringBoneParamsByHash
} from '~/lib/springBones'

export type SpringBonesModel = {
  hash: string
  /** Body shapes whose representation uses this model. */
  bodyShapes: string[]
  bones: BoneNode[]
}

export type SpringBonesData = {
  isLoading: boolean
  /** Some model couldn't be read: its spring bones are unknown, not absent. */
  isError: boolean
  /** Models with spring bones, in representation order. */
  models: SpringBonesModel[]
  /** Saved params (or seeded defaults) per model hash: the form's pristine state. */
  initialParams: SpringBoneParamsByHash
  bonesByHash: Record<string, BoneNode[]>
}

const EMPTY: SpringBonesData = { isLoading: false, isError: false, models: [], initialParams: {}, bonesByHash: {} }

function springBonesQuery(hash: string, blob?: Blob) {
  return {
    queryKey: ['spring-bones', hash],
    queryFn: async () => parseSpringBones(await (blob ?? (await fetchContent(hash))).arrayBuffer()),
    staleTime: Infinity
  }
}

/** Parses a model's spring bones through the same cache the editor form reads. */
export function springBonesFetcher(queryClient: QueryClient): (hash: string, blob?: Blob) => Promise<BoneNode[]> {
  return (hash, blob) => queryClient.fetchQuery(springBonesQuery(hash, blob))
}

/** Parses each representation GLB of a wearable for spring bones and merges the item's saved params. */
export function useSpringBones(item: Item | null): SpringBonesData {
  const hashes = useMemo(() => (item && isSpringBoneItem(item) ? getRepresentationModelHashes(item) : []), [item])
  const queries = useQueries({ queries: hashes.map(hash => springBonesQuery(hash)) })
  const isLoading = queries.some(query => query.isLoading)
  const isError = queries.some(query => query.isError)
  // One scalar stands in for the per-query results: a dependency array must keep its length. Not
  // memoized on purpose: `queries` is a new array every render, so a useMemo keyed on it would
  // recompute every render anyway — this map is what produces the stable key the real memo uses.
  const dataVersion = queries.map(query => query.dataUpdatedAt).join('|')
  return useMemo(() => {
    if (!item || hashes.length === 0) return EMPTY
    const models: SpringBonesModel[] = []
    const initialParams: SpringBoneParamsByHash = {}
    const bonesByHash: Record<string, BoneNode[]> = {}
    hashes.forEach((hash, index) => {
      const bones = queries[index].data
      if (!bones || !hasSpringBones(bones)) return
      const bodyShapes = item.data.representations
        .filter(representation => item.contents[representation.mainFile] === hash)
        .flatMap(representation => representation.bodyShapes)
      models.push({ hash, bodyShapes, bones })
      bonesByHash[hash] = bones
      initialParams[hash] = getInitialSpringBoneParams(item, hash, bones)
    })
    return { isLoading, isError, models, initialParams, bonesByHash }
    // `queries` is a fresh array every render; dataVersion tracks the results that matter.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [item, hashes, isLoading, isError, dataVersion])
}
