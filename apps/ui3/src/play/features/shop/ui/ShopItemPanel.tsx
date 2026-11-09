// The Shop's item panel, beside the grid while an item is selected: the item on its rarity art, its
// price, rarity and stock, creator, description and what it hides, and what can be done with it here.
// A primary sale priced in credits is bought in-world; anything else (a resale, MANA only, not for
// sale) is offered on the web Shop's item page.

import type { CardPrice } from '@shop/discovery/cardPrice'
import { Close, MaskIcon, rarityPanel } from '../../../design'
import hiddenIcon from '../../../assets/backpack/icon-hidden.webp'
import cartIcon from '../../../assets/shop/cart.svg'
import favoriteIcon from '../../../assets/shop/favorite.svg'
import externalIcon from '../../../assets/shop/external.svg'
import { buyableInWorld, PriceTag } from './PriceTag'
import { pressedWith, type PressedWith } from './pressedWith'
import styles from './ShopItemPanel.module.css'

/** A part the item hides on the avatar, as its chip shows it. */
export interface ShopItemHide {
  key: string
  label: string
  icon: React.ReactNode
}

export interface ShopItemPanelProps {
  name: string
  thumbnail?: string | null
  /** Its rarity (the art and the tag); `base` without one. */
  rarity: string
  /** Its category's glyph, before the name. */
  icon: React.ReactNode
  price: CardPrice
  /** What is left of its supply, as "35/1000", when known. */
  stock?: string
  creator: { name: string; picture?: string | null; color: string }
  /** The item's own description and what it hides, once known. */
  description?: string
  hides?: readonly ShopItemHide[]
  onClose: () => void
  /** Offers the item on the web Shop (resales, MANA-only, not for sale); absent when it has no page there. */
  onOpenOnWeb?: () => void
  /** BUY ITEM, with how it was pressed. Without it, or without ADD TO CART, that button is shown off. */
  onBuy?: (input: PressedWith) => void
  onAddToCart?: () => void
}

export function ShopItemPanel({ name, thumbnail, rarity, icon, price, stock, creator, description, hides = [], onClose, onOpenOnWeb, onBuy, onAddToCart }: ShopItemPanelProps): React.JSX.Element {
  return (
    <aside className={styles.panel} data-rarity={rarity} aria-label={name} style={{ '--panel-art': `url(${rarityPanel(rarity)})` } as React.CSSProperties}>
      <button type="button" className={styles.favourite} aria-label="Add to favourites" disabled title="Favourites are coming soon">
        <MaskIcon src={favoriteIcon} size={24} />
      </button>
      <button type="button" className={styles.close} aria-label="Close item" onClick={onClose}>
        <Close size={20} />
      </button>
      <div className={styles.image}>{thumbnail && <img src={thumbnail} alt="" />}</div>
      <div className={styles.info}>
        <div className={styles.title}>
          <span className={styles.name}>
            {icon}
            <span>{name}</span>
          </span>
          <span className={styles.price}>
            <PriceTag price={price} size={24} />
          </span>
        </div>
        <span className={styles.rarity}>
          {rarity}
          {stock != null ? ` ${stock}` : ''}
        </span>
        <hr className={styles.rule} />
        <span className={styles.creator}>
          {creator.picture ? <img className={styles.face} src={creator.picture} alt="" /> : <span className={styles.face} aria-hidden="true" />}
          <span>
            {/* The creator's name colour is the HUD's per-user one, known only at run time. */}
            By <span className={styles.creatorName} style={{ '--creator-color': creator.color } as React.CSSProperties}>{creator.name}</span>
          </span>
        </span>
        <hr className={styles.rule} />
        {description != null && description !== '' && (
          <div className={styles.section}>
            <span className={styles.label}>Description</span>
            <p className={styles.description}>{description}</p>
          </div>
        )}
        {hides.length > 0 && (
          <div className={`${styles.section} ${styles.hidesSection}`}>
            <span className={styles.label}>
              <MaskIcon src={hiddenIcon} size={20} />
              Hides
            </span>
            <div className={styles.hides}>
              {hides.map((h) => (
                <span key={h.key} className={styles.hide}>
                  {h.icon}
                  {h.label}
                </span>
              ))}
            </div>
          </div>
        )}
      </div>
      <div className={styles.actions}>
        {buyableInWorld(price) ? (
          <>
            <button type="button" className={styles.cart} disabled={onAddToCart == null} onClick={onAddToCart}>
              <MaskIcon src={cartIcon} size={22} />
              Add to cart
            </button>
            <button type="button" className={styles.buy} disabled={onBuy == null} onClick={onBuy && ((e) => onBuy(pressedWith(e)))}>
              Buy item
            </button>
          </>
        ) : (
          onOpenOnWeb != null && (
            <button type="button" className={styles.resale} onClick={onOpenOnWeb}>
              Buy resale
              <MaskIcon src={externalIcon} size={18} />
            </button>
          )
        )}
      </div>
    </aside>
  )
}
