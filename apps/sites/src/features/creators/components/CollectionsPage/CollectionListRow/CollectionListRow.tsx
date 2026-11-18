import { useIntl } from 'react-intl'
import { Link, useLocation } from 'react-router-dom'
import { useTranslation } from '@/data/lib/creators/intl'
import { useWallet } from '@/features/creators/store/wallet'
import { formatTimeAgo } from '@/data/lib/creators/time'
import { type Collection } from '@/data/lib/creators/collections'
import { CollectionMosaic } from '@/features/creators/components/CollectionMosaic'
import { CollectionActionsMenu } from '@/features/creators/components/CollectionActionsMenu'
import { CollectionRolePill } from '@/features/creators/components/CollectionRolePill'
import { CollectionStatusPill } from '@/features/creators/components/CollectionStatusPill'
import { ReviewReplyBadge } from '@/features/creators/components/ReviewReplyBadge'
import styles from './CollectionListRow.module.css'

type Props = {
  collection: Collection
}

const DATE_FORMAT = { month: 'short', day: 'numeric', year: 'numeric' } as const

export function CollectionListRow({ collection }: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  const address = useWallet(state => state.session?.address)
  const { search } = useLocation()

  return (
    <article className={styles.row} data-testid="collection-row">
      <div className={styles.nameCell}>
        <div className={styles.thumb}>
          <CollectionMosaic collectionId={collection.id} itemCount={collection.itemCount} showEmptyLabel={false} />
        </div>
        {/* Stretched over the whole row (see RowLink) so the row is one real link. */}
        <Link className={styles.rowLink} to={`/collections/${collection.id}`} state={{ listSearch: search }}>
          <h3 className={styles.name} title={collection.name}>
            {collection.name}
          </h3>
        </Link>
        {address && <CollectionRolePill collection={collection} address={address} />}
      </div>
      <div className={styles.cell} data-testid="collection-row-items">
        {t('collections_page.item_count', { count: collection.itemCount })}
      </div>
      <div className={styles.cell}>
        <CollectionStatusPill collection={collection} />
        <ReviewReplyBadge collection={collection} address={address} />
      </div>
      <div className={styles.dateCell} data-testid="collection-row-updated">
        <strong>{formatTimeAgo(collection.lastActivityAt ?? collection.updatedAt, intl.locale)}</strong>
        <span>{intl.formatDate(collection.lastActivityAt ?? collection.updatedAt, DATE_FORMAT)}</span>
      </div>
      <div className={styles.cell} data-testid="collection-row-created">
        {intl.formatDate(collection.createdAt, DATE_FORMAT)}
      </div>
      <div className={styles.actionsCell}>
        {address && (
          <CollectionActionsMenu
            collection={collection}
            address={address}
            variant="row"
            showRoles={false}
            label={t('collections_page.row_actions')}
          />
        )}
      </div>
    </article>
  )
}
