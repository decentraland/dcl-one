// ShopItemCard — a Shop grid tile, the design system's ExplorerShopCard (DCL-ONE UI Style Guide, Shop page):
// on a dark card, the ItemCard thumbnail (the rarity glow over Brand/Purple, its shine border, a
// rarity-coloured corner flap with the category glyph and an optional discount tag), then the name, the
// creator and a row with the rarity label and the price. Hovering or focusing the card swaps the creator
// and that row for its actions (BUY…); clicking the card selects it. Hovered, focused or selected, it
// takes the design's ring. Its width is the grid's column: the design's 203px, never stretched.

import { useState } from 'react'
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
  /** Shown in place of the creator, rarity and price while the card is hovered or holds focus. */
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
      <span className={styles.thumb}>
        {thumbnail && !failed && <img className={styles.image} src={thumbnail} alt="" onError={() => setFailed(true)} />}
        {categoryIcon != null && (
          <span className={styles.flap}>
            <span className={styles.flapIcon}>{categoryIcon}</span>
          </span>
        )}
        {discount != null && <span className={styles.discount}>{discount}</span>}
      </span>
      <span className={styles.info}>
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

/** A ShopItemCard's place while the grid loads: the design's loading card, its thumbnail and two bars
 *  swept by the Skeleton's band. */
export function ShopItemSkeleton(): React.JSX.Element {
  return (
    <span className={styles.skeleton} aria-hidden="true">
      <span className={`${styles.sheen} ${styles.skeletonTile}`} />
      <span className={styles.skeletonInfo}>
        <span className={`${styles.sheen} ${styles.skeletonBar}`} />
        <span className={`${styles.sheen} ${styles.skeletonBar} ${styles.skeletonBarShort}`} />
      </span>
    </span>
  )
}
