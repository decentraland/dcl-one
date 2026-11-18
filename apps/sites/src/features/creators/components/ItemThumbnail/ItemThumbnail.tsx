import { type ReactNode } from 'react'
import { useTranslation } from '~/intl'
import { cx } from '~/lib/cx'
import { getRarityMediaBackground } from '~/lib/rarities'
import styles from './ItemThumbnail.module.css'

type Props = {
  src?: string | null
  rarity?: string | null
  /** Overlays (badges, hover masks) rendered over the artwork. */
  children?: ReactNode
  className?: string
  testId?: string
  /** Shows a spinner over the frame while the artwork is still being generated. */
  loading?: boolean
}

function LoadingSpinner({ testId }: { testId: string }) {
  const { t } = useTranslation()
  return (
    <span className={styles.spinner} role="status" aria-label={t('item_thumbnail.generating')} data-testid={testId} />
  )
}

/** An item's artwork over its rarity-tinted media field; neutral when the rarity is unknown or missing. */
export function ItemThumbnail({ src, rarity, children, className, testId = 'item-thumbnail', loading = false }: Props) {
  const backgroundImage = getRarityMediaBackground(rarity)
  return (
    <span
      className={cx(styles.frame, className)}
      data-testid={testId}
      data-loading={loading || undefined}
      aria-busy={loading || undefined}
      style={backgroundImage ? { backgroundImage } : undefined}
    >
      {src && <img className={styles.img} src={src} alt="" loading="lazy" data-testid={`${testId}-img`} />}
      {loading && <LoadingSpinner testId={`${testId}-loading`} />}
      {children}
    </span>
  )
}
