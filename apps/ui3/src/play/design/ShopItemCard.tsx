// ShopItemCard — a Shop grid tile: the rarity texture with the thumbnail, a rarity-coloured corner flap
// with the category glyph and an optional discount tag, then on a dark caption the name, the creator and
// a row with the rarity tag and the price. Hovering or focusing the card swaps that row for its actions
// (TRY ON, BUY…); clicking the card selects it.

import { useState } from 'react'
import { rarityTile } from './rarityArt'
import styles from './ShopItemCard.module.css'

export interface ShopItemCardProps {
  name: string
  thumbnail?: string
  rarity?: string
  /** Display name of the creator. */
  creator?: string
  creatorPicture?: string
  /** The price line, e.g. a credits amount with its mark. Hidden while the actions show. */
  price?: React.ReactNode
  /** Discount tag over the thumbnail, e.g. "-20%". */
  discount?: string
  /** Body-part glyph shown in the top-left flap. */
  categoryIcon?: React.ReactNode
  selected?: boolean
  /** Without it the tile is not selectable. */
  onSelect?: () => void
  /** Shown in place of the rarity and price while the card is hovered or holds focus. */
  actions?: React.ReactNode
}

export function ShopItemCard({
  name,
  thumbnail,
  rarity,
  creator,
  creatorPicture,
  price,
  discount,
  categoryIcon,
  selected = false,
  onSelect,
  actions
}: ShopItemCardProps): React.JSX.Element {
  const [failed, setFailed] = useState(false)
  return (
    <div className={`${styles.card} ${selected ? styles.selected : ''} ${actions != null ? styles.hasActions : ''}`.trim()} data-rarity={rarity ?? 'base'}>
      {onSelect != null && <button type="button" className={styles.hit} aria-label={name} aria-pressed={selected} onClick={onSelect} />}
      <span className={styles.thumb} style={{ backgroundImage: `url(${rarityTile(rarity)})` }}>
        {thumbnail && !failed && <img className={styles.image} src={thumbnail} alt="" onError={() => setFailed(true)} />}
        {categoryIcon != null && (
          <span className={styles.flap}>
            <span className={styles.flapIcon}>{categoryIcon}</span>
          </span>
        )}
        {discount != null && <span className={styles.discount}>{discount}</span>}
      </span>
      <span className={styles.caption}>
        <span className={styles.name} title={name}>
          {name}
        </span>
        {creator != null && (
          <span className={styles.creator}>
            {creatorPicture ? <img className={styles.face} src={creatorPicture} alt="" /> : <span className={styles.face} aria-hidden="true" />}
            <span className={styles.creatorName}>{creator}</span>
          </span>
        )}
        <span className={styles.meta}>
          <span className={styles.rarity}>{rarity ?? 'base'}</span>
          {price != null && <span className={styles.price}>{price}</span>}
        </span>
        {actions != null && <span className={styles.actions}>{actions}</span>}
      </span>
    </div>
  )
}
