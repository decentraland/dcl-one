import { useQuery } from '@tanstack/react-query'
import { fetchContent } from '@/data/lib/creators/builder'
import { isImageFile } from '@/data/lib/creators/itemFiles'
import { ItemType, hasEmptyModelMetrics, type Item } from '@/data/lib/creators/items'
import { countModelMetrics } from '@/data/lib/creators/modelMetrics'
import { captureError } from '@/data/lib/creators/monitoring'

/**
 * Metrics counted from the model file of a wearable saved without geometry (a preview read of an empty scene),
 * or null when the stored metrics are fine or the file can't be counted.
 */
export function useRepairedItemMetrics(item: Item | null) {
  const mainFile = item?.data.representations[0]?.mainFile
  const hash = mainFile ? item?.contents[mainFile] : undefined
  const enabled =
    !!item &&
    item.type === ItemType.WEARABLE &&
    hasEmptyModelMetrics(item.metrics ?? {}) &&
    !!mainFile &&
    !isImageFile(mainFile) &&
    !!hash
  const query = useQuery({
    queryKey: ['repaired-item-metrics', item?.id, hash],
    queryFn: async () => {
      try {
        return await countModelMetrics(await fetchContent(hash!))
      } catch (error) {
        captureError(error, { flow: 'item_metrics_repair', itemId: item?.id })
        return null
      }
    },
    enabled,
    staleTime: Infinity,
    retry: false
  })
  return enabled ? (query.data ?? null) : null
}
