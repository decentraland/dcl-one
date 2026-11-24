import { useQuery } from '@tanstack/react-query'
import { fetchContent, getContentsStorageUrl } from '@/data/lib/creators/builder'
import type { Item } from '@/data/lib/creators/items'
import { createLimiter } from '@/data/lib/creators/limiter'
import { privateKey } from '@/data/lib/creators/queryKeys'
import { useWallet } from '../store/wallet'
import { useObjectURL } from './useObjectURL'

const readDraftFile = createLimiter(4)

/**
 * Where an item's stored file `hash` shows: its storage URL once the item is published; for a draft, whose files only
 * its signed readers get, an object URL of the file read by the session, revoked once it no longer shows.
 * @returns null without a file, while a draft's file loads, or when it can't be read
 */
export function useItemFileSrc(item: Pick<Item, 'isPublished'>, hash: string | null | undefined): string | null {
  const address = useWallet(state => state.session?.address)
  const draft = !!hash && !item.isPublished
  const { data } = useQuery({
    queryKey: privateKey(address, 'item-file', hash),
    queryFn: ({ signal }) => readDraftFile(() => fetchContent(hash!, signal), signal),
    enabled: draft && !!address,
    staleTime: Infinity
  })
  const objectUrl = useObjectURL(draft && address ? data : undefined)
  if (!hash) return null
  return draft ? objectUrl : getContentsStorageUrl(hash)
}
