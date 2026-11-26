// ShopItemTile — the item as the Shop's dialogs show it: the Backpack's tile (the rarity texture, the
// thumbnail, the rarity-tinted corner flap with the category glyph) at 150px, with the NEW tag once bought.
// Static: nothing on it is interactive.

import { useState } from 'react'
import { rarityTile } from '@hud/design'
import styles from './ShopItemTile.module.css'

export interface ShopItemTileProps {
  thumbnail?: string
  rarity?: string
  /** Body-part glyph shown in the top-left flap. */
  categoryIcon?: React.ReactNode
  isNew?: boolean
  className?: string
}

export function ShopItemTile({ thumbnail, rarity, categoryIcon, isNew = false, className }: ShopItemTileProps): React.JSX.Element {
  const [failed, setFailed] = useState(false)
  return (
    <span className={`${styles.tile} ${className ?? ''}`.trim()} data-rarity={rarity || 'base'} style={{ backgroundImage: `url(${rarityTile(rarity || 'base')})` }}>
      {thumbnail && !failed && <img className={styles.image} src={thumbnail} alt="" onError={() => setFailed(true)} />}
      {categoryIcon != null && (
        <span className={styles.flap}>
          <span className={styles.flapTint} aria-hidden="true" />
          <span className={styles.flapIcon}>{categoryIcon}</span>
        </span>
      )}
      {isNew && <span className={styles.new}>New</span>}
    </span>
  )
}
