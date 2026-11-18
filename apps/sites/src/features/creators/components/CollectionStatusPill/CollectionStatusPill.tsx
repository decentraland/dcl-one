import type { ReactNode } from 'react'
import { LinkIcon } from '~/components/Icons'
import { useTranslation } from '~/intl'
import { InfoTooltip } from '~/components/Tooltip'
import { CollectionDisplayStatus, getCollectionDisplayStatus, type Collection } from '~/lib/collections'
import styles from './CollectionStatusPill.module.css'

type Props = {
  collection: Collection
  /** Overrides the status derived from the collection, e.g. once its curation has loaded. */
  status?: CollectionDisplayStatus
  /** Shown as an (i) tooltip inside the pill; the caller decides which status deserves one. */
  hint?: ReactNode
}

export function CollectionStatusPill({ collection, status: statusOverride, hint }: Props) {
  const { t } = useTranslation()
  const status = statusOverride ?? getCollectionDisplayStatus(collection)
  return (
    <span className={styles.pill} data-testid="collection-status" data-status={status}>
      {status === CollectionDisplayStatus.LINKED && <LinkIcon aria-hidden />}
      {t(`collection_status.${status}`)}
      {hint && <InfoTooltip placement="right" content={hint} testId="collection-status-hint" />}
    </span>
  )
}
