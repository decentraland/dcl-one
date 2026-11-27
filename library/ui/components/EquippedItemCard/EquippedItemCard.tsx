import { useState, type ReactNode } from 'react'
import { Button } from '../Button/Button'
import rarityStyles from '../WearableCard/rarity.module.css'
import styles from './EquippedItemCard.module.css'

export type EquippedItemCardProps = {
  thumbnail?: string
  name?: string
  /**
   * Tints the picture, the flap and the tag, and is the tag's text. One of the foundations' rarities
   * in lower case; another value is drawn as `base` and shown as given. `base` without it.
   */
  rarity?: string
  /** Where the item is sold: the card shows a link to it. */
  shopUrl?: string
  /** The link's text. "Shop" by default. */
  shopLabel?: string
  /** A price line under the rarity tag. */
  price?: string
  /** The glyph in the top-left flap, e.g. the wearable category's. */
  categoryIcon?: ReactNode
  /**
   * Keeps the link inside the card at all times. Without it the link shows under the card while it is
   * hovered, over whatever sits below.
   */
  pinShop?: boolean
  /** What a click on the card does (an emote's card plays it): the picture is then a button. */
  onAction?: () => void
  /** The button's name; needed with `onAction`. */
  actionLabel?: string
}

/**
 * An item a profile shows, for display only: the thumbnail on its rarity's colour with a corner flap,
 * then the name on one line, the rarity tag and an optional price. With `shopUrl`, hovering frames
 * and lifts the card and shows a link to the item's sale.
 */
export function EquippedItemCard({
  thumbnail,
  name,
  rarity,
  shopUrl,
  shopLabel = 'Shop',
  price,
  categoryIcon,
  pinShop = false,
  onAction,
  actionLabel
}: EquippedItemCardProps) {
  // The picture that failed to load: a new one is tried again.
  const [failedSrc, setFailedSrc] = useState<string>()
  const failed = failedSrc != null && failedSrc === thumbnail
  const shop = shopUrl == null ? undefined : pinShop ? styles.pinnedShop : styles.hasShop
  const picture = (
    <>
      {thumbnail && !failed ? (
        <img className={styles.thumb} src={thumbnail} alt="" onError={() => setFailedSrc(thumbnail)} />
      ) : (
        <span className={styles.placeholder} />
      )}
      {categoryIcon != null && <span className={styles.corner}>{categoryIcon}</span>}
    </>
  )
  return (
    <div
      className={[styles.card, rarityStyles.tone, shop].filter(Boolean).join(' ')}
      data-rarity={rarity ?? 'base'}
      data-action={onAction != null ? '' : undefined}
      // The whole card answers the pointer; the picture's button is the same action for the keyboard.
      onClick={onAction == null ? undefined : (e) => {
        if (e.target instanceof Element && e.target.closest('a, button') != null) return
        onAction()
      }}
    >
      {onAction != null ? (
        <button type="button" className={styles.thumbWrap} aria-label={actionLabel} onClick={onAction}>
          {picture}
        </button>
      ) : (
        <div className={styles.thumbWrap}>{picture}</div>
      )}
      <span className={styles.name} title={name}>
        {name}
      </span>
      {/* Always rendered, so every card of a row has the same height. */}
      <span className={styles.rarityTag}>{rarity ?? 'base'}</span>
      {price != null && <span className={styles.price}>{price}</span>}
      {shopUrl != null && (
        <Button
          appearance="hud"
          href={shopUrl}
          target="_blank"
          rel="noopener"
          size="sm"
          className={styles.shopBtn}
        >
          {shopLabel}
        </Button>
      )}
    </div>
  )
}
