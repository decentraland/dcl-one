// The Shop's item panel, beside the grid while an item is selected: the item on its rarity art, its
// price, rarity and stock, creator, description and what it hides, and what can be done with it here.
// A primary sale priced in credits is bought in-world; anything else (a resale, MANA only, not for
// sale) is offered on the web Shop's item page.

import type { CardPrice } from '../../../shop/discovery/cardPrice'
import type { CatalogItem } from '../../../shop/discovery/lib/catalogItem'
import { MAX_SUPPLY } from '../../../shop/discovery/lib/rarity'
import { Close, Heart, MaskIcon, rarityPanel } from '../../design'
import { shortAddr, userColor } from '../../lib/identity'
import { CategoryIcon } from '../backpack/categoryIcons'
import { useProfile } from '../session/profileStore'
import hiddenIcon from '../../assets/backpack/icon-hidden.webp'
import cartIcon from '../../assets/shop/cart.svg'
import externalIcon from '../../assets/shop/external.svg'
import { useItemEntity } from './itemEntity'
import { buyableInWorld, PriceTag } from './PriceTag'
import styles from './ShopItemPanel.module.css'

const CATEGORY_NAMES: Record<string, string> = { hands_wear: 'Handwear', body_shape: 'Body shape', upper_body: 'Upper body', lower_body: 'Lower body', top_head: 'Top head', facial_hair: 'Facial hair' }
const categoryName = (c: string): string => CATEGORY_NAMES[c] ?? c.replace(/[_-]+/g, ' ')

export interface ShopItemPanelProps {
  item: CatalogItem
  price: CardPrice
  /** The item's URN, for its entity (description, hides); null when it has none. */
  urn: string | null
  onClose: () => void
  /** Offers the item on the web Shop (resales, MANA-only, not for sale). */
  onOpenOnWeb: () => void
  /** Buying in-world (the next flow); without them the buttons are shown but do nothing yet. */
  onBuy?: () => void
  onAddToCart?: () => void
}

export function ShopItemPanel({ item, price, urn, onClose, onOpenOnWeb, onBuy, onAddToCart }: ShopItemPanelProps): React.JSX.Element {
  const rarity = item.rarity || 'base'
  const creator = useProfile(item.creator)
  const entity = useItemEntity(urn)
  const max = MAX_SUPPLY[rarity]
  const stock = item.available != null && max != null ? ` ${item.available}/${max}` : ''
  const creatorName = creator?.name || shortAddr(item.creator)
  const category = item.category === 'emote' ? 'emote' : (item.wearableCategory ?? item.category)
  return (
    <aside className={styles.panel} data-rarity={rarity} aria-label={item.name} style={{ '--panel-art': `url(${rarityPanel(rarity)})` } as React.CSSProperties}>
      <button type="button" className={styles.favourite} aria-label="Add to favourites" disabled title="Favourites are coming soon">
        <Heart size={20} />
      </button>
      <button type="button" className={styles.close} aria-label="Close item" onClick={onClose}>
        <Close size={20} />
      </button>
      <div className={styles.image}>{item.thumbnail && <img src={item.thumbnail} alt="" />}</div>
      <div className={styles.info}>
        <div className={styles.title}>
          <span className={styles.name}>
            <CategoryIcon category={category} size={28} />
            <span>{item.name}</span>
          </span>
          <span className={styles.price}>
            <PriceTag price={price} size={24} />
          </span>
        </div>
        <span className={styles.rarity}>
          {rarity}
          {stock}
        </span>
        <hr className={styles.rule} />
        <span className={styles.creator}>
          {creator?.picture ? <img className={styles.face} src={creator.picture} alt="" /> : <span className={styles.face} aria-hidden="true" />}
          <span>
            By <span style={{ color: userColor(item.creator, creatorName, creator?.hasClaimedName) }}>{creatorName}</span>
          </span>
        </span>
        <hr className={styles.rule} />
        {entity.data != null && entity.data.description !== '' && (
          <div className={styles.section}>
            <span className={styles.label}>Description</span>
            <p className={styles.description}>{entity.data.description}</p>
          </div>
        )}
        {entity.data != null && entity.data.hides.length > 0 && (
          <div className={`${styles.section} ${styles.hidesSection}`}>
            <span className={styles.label}>
              <MaskIcon src={hiddenIcon} size={20} />
              Hides
            </span>
            <div className={styles.hides}>
              {entity.data.hides.map((h) => (
                <span key={h} className={styles.hide}>
                  <CategoryIcon category={h} size={22} />
                  {categoryName(h)}
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
            <button type="button" className={styles.buy} disabled={onBuy == null} onClick={onBuy}>
              Buy item
            </button>
          </>
        ) : (
          <button type="button" className={styles.resale} onClick={onOpenOnWeb}>
            Buy resale
            <MaskIcon src={externalIcon} size={18} />
          </button>
        )}
      </div>
    </aside>
  )
}
