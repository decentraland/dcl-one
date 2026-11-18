import { type CSSProperties } from 'react'
import { Rarity } from '@dcl/schemas'
import { useTranslation } from '~/intl'
import { formatRarityLabel, isRarity } from '~/lib/rarities'
import styles from './RarityPill.module.css'

const LIGHT_OVERRIDES: Partial<Record<string, string>> = {
  legendary: 'var(--color-rarity-legendary-light)'
}

type Props = {
  rarity: string
  /** Appends the max supply — "LEGENDARY (100)" / "EPIC (1K)" — the default everywhere items are listed. */
  showSupply?: boolean
  size?: 'default' | 'large'
  testId?: string
}

/** The colored rarity label used in every item list, detail and review table. */
export function RarityPill({ rarity, showSupply = true, size = 'default', testId = 'rarity-pill' }: Props) {
  const { t } = useTranslation()
  const known = isRarity(rarity)
  const style = known
    ? ({
        '--rarity-color': Rarity.getColor(rarity as Rarity),
        '--rarity-light': LIGHT_OVERRIDES[rarity] ?? Rarity.getGradient(rarity as Rarity)[0]
      } as CSSProperties)
    : undefined
  const label = known ? t(`collection_detail_page.rarity.${rarity}`) : rarity

  return (
    <span className={styles.pill} data-testid={testId} data-rarity={rarity} data-size={size} style={style}>
      {showSupply ? formatRarityLabel(label, rarity) : label}
    </span>
  )
}
