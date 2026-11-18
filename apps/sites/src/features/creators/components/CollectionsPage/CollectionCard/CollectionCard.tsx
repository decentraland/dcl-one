import { useIntl } from 'react-intl'
import { Link, useLocation } from 'react-router-dom'
import { useTranslation } from '~/intl'
import { useWallet } from '~/store/wallet'
import { formatTimeAgo } from '~/lib/time'
import { type Collection } from '~/lib/collections'
import { CollectionMosaic } from '~/components/CollectionMosaic'
import { CollectionRolePill } from '~/components/CollectionRolePill'
import { CollectionStatusPill } from '~/components/CollectionStatusPill'
import { ReviewReplyBadge } from '~/components/ReviewReplyBadge'
import styles from './CollectionCard.module.css'

type Props = {
  collection: Collection
}

/**
 * A collection in the grid: 2x2 item mosaic over a translucent footer with name, status, item count
 * and last update. Below the mobile breakpoint it reshapes into the design's horizontal row card.
 */
export function CollectionCard({ collection }: Props) {
  const { t } = useTranslation()
  const { locale } = useIntl()
  const address = useWallet(state => state.session?.address)
  const { search } = useLocation()

  return (
    <Link
      className={styles.card}
      data-testid="collection-card"
      to={`/collections/${collection.id}`}
      state={{ listSearch: search }}
    >
      <div className={styles.media}>
        <CollectionMosaic collectionId={collection.id} itemCount={collection.itemCount} />
        {address && <CollectionRolePill className={styles.roleBadge} collection={collection} address={address} />}
        <ReviewReplyBadge className={styles.replyBadge} collection={collection} address={address} />
      </div>
      <div className={styles.body}>
        <div className={styles.nameRow}>
          <h3 className={styles.name} data-testid="collection-card-name" title={collection.name}>
            {collection.name}
          </h3>
          <CollectionStatusPill collection={collection} />
        </div>
        <span className={styles.meta} data-testid="collection-card-items">
          {t('collections_page.item_count', { count: collection.itemCount })}
        </span>
        <span className={styles.meta} data-testid="collection-card-updated">
          {t('collections_page.updated_ago', {
            timeAgo: formatTimeAgo(collection.lastActivityAt ?? collection.updatedAt, locale)
          })}
        </span>
        <span className={styles.manage} data-testid="collection-card-manage" aria-hidden>
          {t('collections_page.manage')}
        </span>
      </div>
    </Link>
  )
}
