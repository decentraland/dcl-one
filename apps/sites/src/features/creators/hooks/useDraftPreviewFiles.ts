import { useQuery } from '@tanstack/react-query'
import { fetchContent } from '@/data/lib/creators/builder'
import type { Item } from '@/data/lib/creators/items'
import { privateKey } from '@/data/lib/creators/queryKeys'
import { useWallet } from '../store/wallet'

/**
 * A draft's model files, read signed by the session and keyed by path, for the preview to load from memory: the
 * preview's own requests are unsigned, and a draft's files are private until it is published.
 * @returns undefined for a published item, while the files load, or when one can't be read
 */
export function useDraftPreviewFiles(item: Item | null): Record<string, Blob> | undefined {
  const address = useWallet(state => state.session?.address)
  const paths = [...new Set(item?.data.representations.flatMap(representation => representation.contents) ?? [])]
  const files = paths.map(path => [path, item?.contents[path]] as const)
  const { data } = useQuery({
    queryKey: privateKey(address, 'item-preview-files', item?.id, files),
    queryFn: async ({ signal }) =>
      Object.fromEntries(await Promise.all(files.map(async ([path, hash]) => [path, await fetchContent(hash!, signal)]))),
    enabled: !!item && !item.isPublished && !!address && files.every(([, hash]) => hash),
    staleTime: Infinity
  })
  return item && !item.isPublished ? data : undefined
}
