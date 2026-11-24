import { SparklesIcon } from '../Icons'
import { ItemFileThumbnail } from '../ItemThumbnail'
import type { Item } from '@/data/lib/creators/items'
import { useTranslation } from '@/data/lib/creators/intl'
import { cx } from '@/data/lib/creators/cx'
import styles from './ThumbnailMosaic.module.css'

export type MosaicThumbnail = {
  item: Pick<Item, 'isPublished'>
  hash: string
  rarity?: string | null
}

type Props = {
  /** Up to four thumbnails; extra entries are ignored. */
  thumbnails: MosaicThumbnail[]
  loading?: boolean
  /** Nothing to show yet (an empty collection): a neutral wash with a "No items" label instead of the bare media field. */
  empty?: boolean
  /** False for covers too small to fit the "No items" label next to the glyph. */
  showEmptyLabel?: boolean
  className?: string
  testId?: string
}

/** 2x2 thumbnail grid: fewer than 4 images stretch to fill; `empty` shows a "No items" state, otherwise zero images leave the bare media field. */
export function ThumbnailMosaic({
  thumbnails,
  loading = false,
  empty = false,
  showEmptyLabel = true,
  className,
  testId = 'thumbnail-mosaic'
}: Props) {
  const { t } = useTranslation()
  const urls = thumbnails.slice(0, 4)
  const isEmpty = empty && !loading
  return (
    <div
      className={cx(styles.mosaic, className)}
      data-testid={testId}
      data-count={urls.length}
      data-empty={isEmpty || undefined}
    >
      {isEmpty ? (
        <div className={styles.empty} data-testid={`${testId}-empty`}>
          <SparklesIcon />
          {showEmptyLabel && t('thumbnail_mosaic.no_items')}
        </div>
      ) : loading ? (
        <div className={cx(styles.loading, 'skeleton')} data-testid={`${testId}-loading`} aria-hidden />
      ) : (
        urls.map(({ item, hash, rarity }, index) => (
          <div className={styles.cell} key={`${index}-${hash}`} data-testid={`${testId}-cell`}>
            <ItemFileThumbnail item={item} hash={hash} rarity={rarity} testId={`${testId}-thumbnail`} />
          </div>
        ))
      )}
    </div>
  )
}
