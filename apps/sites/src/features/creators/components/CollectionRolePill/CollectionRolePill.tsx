import { useTranslation } from '@/data/lib/creators/intl'
import { getCollectionRole, type Collection } from '@/data/lib/creators/collections'
import { cx } from '@/data/lib/creators/cx'
import styles from './CollectionRolePill.module.css'

type Props = {
  collection: Collection
  address: string
  className?: string
}

/** "Collaborator" / "Sender" badge for collections the address can access without owning; nothing for owners. */
export function CollectionRolePill({ collection, address, className }: Props) {
  const { t } = useTranslation()
  const role = getCollectionRole(collection, address)
  if (!role) return null
  return (
    <span className={cx(styles.pill, className)} data-testid="collection-role" data-role={role}>
      {t(`collection_role.${role}`)}
    </span>
  )
}
