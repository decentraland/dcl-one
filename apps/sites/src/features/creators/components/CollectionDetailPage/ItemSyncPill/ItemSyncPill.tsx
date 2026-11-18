import { useTranslation } from '~/intl'
import { ItemRowStatus } from '~/lib/itemSync'
import { InfoTooltip } from '~/components/Tooltip'
import styles from './ItemSyncPill.module.css'

type Props = {
  status: ItemRowStatus
}

/** The item row's Status pill; every state but "published" explains itself in an (i) tooltip. */
export function ItemSyncPill({ status }: Props) {
  const { t } = useTranslation()
  const hint = status === ItemRowStatus.PUBLISHED ? null : t(`collection_detail_page.item_status.${status}_hint`)
  return (
    <span className={styles.pill} data-testid="item-sync-status" data-status={status}>
      {t(`collection_detail_page.item_status.${status}`)}
      {hint && <InfoTooltip content={hint} testId="item-sync-status-hint" />}
    </span>
  )
}
