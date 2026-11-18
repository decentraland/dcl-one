import { useMemo } from 'react'
import { useWallet } from '~/store/wallet'
import { useCollectionPreview } from '~/hooks/useCollections'
import { ThumbnailMosaic } from '~/components/ThumbnailMosaic'

type Props = {
  collectionId: string
  itemCount: number
  showEmptyLabel?: boolean
  className?: string
}

/** A collection's cover: the first item thumbnails (same request the legacy builder's CollectionImage makes). */
export function CollectionMosaic({ collectionId, itemCount, showEmptyLabel, className }: Props) {
  const address = useWallet(state => state.session?.address)
  const { data: previews, isLoading } = useCollectionPreview(address, collectionId, itemCount)
  const thumbnails = useMemo(
    () => (previews ?? []).map(item => ({ url: item.thumbnailUrl, rarity: item.rarity })),
    [previews]
  )
  return (
    <ThumbnailMosaic
      thumbnails={thumbnails}
      loading={itemCount > 0 && isLoading}
      empty={itemCount === 0}
      showEmptyLabel={showEmptyLabel}
      className={className}
      testId="collection-mosaic"
    />
  )
}
