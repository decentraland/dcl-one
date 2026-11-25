// ShopItemCard — the item as the design system's item-card draws it in the Shop's dialogs: Brand/Purple under two
// radial glows of the item's rarity (its Light from the top left, the rarity itself from the bottom right; exotic uses
// its Light for both), a 2.5px rim of the Light, the rarity's corner flap with the category glyph, the item's own image
// over the whole frame, the SMART badge for a smart wearable, the amount bought (x2) and the NEW tag once bought. Any size: the caller sets it (and --flap-scale, for the flap), and the
// tag keeps its size in its corner. Static: nothing on it is interactive. A NAME has its own art (ShopNameTile).

import { useState } from 'react'
import smartBolt from '../assets/modal/smart-bolt.svg'
import styles from './ShopItemCard.module.css'

export interface ShopItemCardProps {
  thumbnail?: string
  rarity?: string
  /** The category glyph in the top-left flap. */
  categoryIcon?: React.ReactNode
  isNew?: boolean
  /** A smart wearable: the SMART badge in the top-right corner. */
  smart?: boolean
  /** How many of it, past one: "x2" in the bottom-right corner. */
  amount?: number
  className?: string
}

export function ShopItemCard({ thumbnail, rarity, categoryIcon, isNew = false, smart = false, amount = 1, className }: ShopItemCardProps): React.JSX.Element {
  const [failed, setFailed] = useState(false)
  return (
    <span className={`${styles.card} ${className ?? ''}`.trim()} data-rarity={rarity || 'base'}>
      <span className={styles.art}>
        {thumbnail && !failed && <img className={styles.image} src={thumbnail} alt="" onError={() => setFailed(true)} />}
        {categoryIcon != null && (
          <span className={styles.flap}>
            <svg className={styles.flapShape} viewBox="0 0 45.283 45.1207" aria-hidden="true" focusable="false">
              <path
                className={styles.flapSolid}
                d="M45.283 0C42.3351 0 39.5085 1.16715 37.428 3.24349L3.23252 37.3411C1.16235 39.4071 0 42.2045 0 45.1207V0C0 0 35.1599 0 45.283 0Z"
              />
              <path
                className={styles.flapShadow}
                d="M0.000103047 36.6252V44.7172C0.0197062 41.8279 1.18007 39.0602 3.23262 37.0118L29.7932 10.5046C29.8293 10.4686 29.8656 10.4328 29.9022 10.3973L0.000103047 36.6252Z"
              />
            </svg>
            <span className={styles.flapIcon}>{categoryIcon}</span>
          </span>
        )}
      </span>
      {smart && (
        <span className={styles.smart}>
          <img src={smartBolt} alt="" aria-hidden="true" width={8.408} height={12.115} />
          SMART
        </span>
      )}
      {amount > 1 && <span className={styles.amount}>x{amount}</span>}
      {isNew && <span className={styles.new}>New</span>}
    </span>
  )
}
