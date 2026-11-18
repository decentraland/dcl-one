import { useTranslation } from '~/intl'
import { type ReviewType } from '~/lib/curation'
import styles from './ReviewTypePill.module.css'

/** Whether a review is a collection's first one or an update to a collection approved before. */
export function ReviewTypePill({ type }: { type: ReviewType }) {
  const { t } = useTranslation()
  return (
    <span className={styles.pill} data-testid="review-type" data-type={type}>
      {t(`review_type.${type}`)}
    </span>
  )
}
