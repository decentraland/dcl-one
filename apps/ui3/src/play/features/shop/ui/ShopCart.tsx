// ShopCart — the in-world Shop's cart, as the design's Cart flow draws it: a 380px popover under the header's
// cart button ("Your Cart (N)"), a card per item (its tile, name and creator, how many with − and + (− at
// one takes it out), its price; on hover, favourite and remove), the total and CHECKOUT. Past five cards the list scrolls, with
// the Shop's scroll thumb. An empty cart isn't drawn in the design; it says so, and CHECKOUT is off.
// Presentational: the page owns the cart and what each action does. Also ShopCartToast ("1 Item Added to
// Cart") and ShopCartBadge (the count over the cart button).

import { useRef } from 'react'
import { formatCredits, formatCreditsFull } from '@shop/checkout/lib/currency'
import { ScrollThumb } from '../../../design'
import closeIcon from '../../../assets/shop/cart/close.svg'
import heartIcon from '../../../assets/shop/cart/heart.svg'
import minusIcon from '../../../assets/shop/cart/minus.svg'
import plusIcon from '../../../assets/shop/cart/plus.svg'
import toastCheck from '../../../assets/shop/cart/toast-check.svg'
import trashIcon from '../../../assets/shop/cart/trash.svg'
import { CreditsIcon } from './CreditsIcon'
import { pressedWith, type PressedWith } from './pressedWith'
import { ShopItemTile } from './ShopItemTile'
import styles from './ShopCart.module.css'

export interface ShopCartLine {
  key: string
  name: string
  /** The creator's name (the page resolves it). */
  creator: React.ReactNode
  thumbnail?: string
  rarity: string
  categoryIcon?: React.ReactNode
  quantity: number
  /** Whole credits per unit. */
  credits: number
  /** Whether one more fits (its stock, the purchase's cap). */
  canIncrement: boolean
}

export interface ShopCartProps {
  lines: readonly ShopCartLine[]
  /** Whole credits, every line's units. */
  total: number
  onClose: () => void
  onQuantity: (key: string, quantity: number) => void
  onRemove: (key: string) => void
  /** CHECKOUT, with how it was pressed. */
  onCheckout: (input: PressedWith) => void
  className?: string
}

/** The design's thumb beside a scrolling list. */
const THUMB = 160

export function ShopCart({ lines, total, onClose, onQuantity, onRemove, onCheckout, className }: ShopCartProps): React.JSX.Element {
  const list = useRef<HTMLUListElement>(null)
  const title = useRef<HTMLHeadingElement>(null)
  const count = lines.reduce((n, l) => n + l.quantity, 0)
  // The button that removed a line goes with it: the focus moves to the cart's title, not to the page.
  const remove = (key: string): void => {
    title.current?.focus()
    onRemove(key)
  }
  return (
    <section className={`${styles.cart} ${className ?? ''}`.trim()} aria-label="Your cart">
      <div className={styles.top}>
        <div className={styles.head}>
          <h2 ref={title} className={styles.title} tabIndex={-1}>
            Your Cart ({count})
          </h2>
          <button type="button" className={styles.close} aria-label="Close cart" onClick={onClose}>
            <img src={closeIcon} alt="" width={30} height={30} />
          </button>
        </div>
        {lines.length === 0 ? (
          <p className={styles.empty}>Your cart is empty.</p>
        ) : (
          <div className={styles.listWrap}>
            <ul ref={list} className={styles.list}>
              {lines.map((line) => (
                <li key={line.key} className={styles.card}>
                  <span className={styles.tileSlot}>
                    <ShopItemTile className={styles.tile} thumbnail={line.thumbnail} rarity={line.rarity} categoryIcon={line.categoryIcon} />
                  </span>
                  <div className={styles.txt}>
                    <div className={styles.row}>
                      <div className={styles.about}>
                        <span className={styles.name} title={line.name}>
                          {line.name}
                        </span>
                        <span className={styles.creator}>{line.creator}</span>
                      </div>
                      <div className={styles.hover}>
                        <button type="button" className={styles.action} aria-label={`Add ${line.name} to favourites`} disabled title="Favourites are coming soon">
                          <img src={heartIcon} alt="" width={14.9284} height={14.9284} />
                        </button>
                        <button type="button" className={styles.action} aria-label={`Remove ${line.name}`} onClick={() => remove(line.key)}>
                          <img src={trashIcon} alt="" width={14.9284} height={14.9284} />
                        </button>
                      </div>
                    </div>
                    <div className={styles.row}>
                      <span className={styles.stepper}>
                        {/* At one, − takes the item out, as the trash does. */}
                        <button
                          type="button"
                          className={styles.step}
                          aria-label={line.quantity > 1 ? `One less ${line.name}` : `Remove ${line.name}`}
                          onClick={() => (line.quantity > 1 ? onQuantity(line.key, line.quantity - 1) : remove(line.key))}
                        >
                          <img src={minusIcon} alt="" width={16} height={16} />
                        </button>
                        <span className={styles.quantity} aria-label={`Quantity: ${line.quantity}`}>
                          {line.quantity}
                        </span>
                        <button type="button" className={styles.step} aria-label={`One more ${line.name}`} disabled={!line.canIncrement} onClick={() => onQuantity(line.key, line.quantity + 1)}>
                          <img src={plusIcon} alt="" width={16} height={16} />
                        </button>
                      </span>
                      <span className={styles.price} title={`${formatCreditsFull(line.credits)} Credits each`}>
                        <span className={styles.mark}>
                          <CreditsIcon size={22.4} className={styles.markIcon} />
                        </span>
                        {formatCredits(line.credits)}
                      </span>
                    </div>
                  </div>
                </li>
              ))}
            </ul>
            <ScrollThumb target={list} length={THUMB} className={styles.thumb} />
          </div>
        )}
      </div>
      <div className={styles.foot}>
        <div className={styles.total}>
          <span className={styles.totalLabel}>Total</span>
          <span className={styles.totalPrice} title={`${formatCreditsFull(total)} Credits`}>
            <span className={styles.totalMark}>
              <CreditsIcon size={28.8} className={styles.totalMarkIcon} />
            </span>
            {formatCredits(total)}
          </span>
        </div>
        <button type="button" className={styles.checkout} disabled={lines.length === 0} onClick={(e) => onCheckout(pressedWith(e))}>
          Checkout
        </button>
      </div>
    </section>
  )
}

/** "1 Item Added to Cart": the design's toast under the header's right end (put it in a live region). */
export function ShopCartToast({ children, className }: { children: React.ReactNode; className?: string }): React.JSX.Element {
  return (
    <div className={`${styles.toast} ${className ?? ''}`.trim()}>
      <img src={toastCheck} alt="" width={24} height={24} />
      <span>{children}</span>
    </div>
  )
}

/** The count over the header's cart button. */
export function ShopCartBadge({ count, className }: { count: number; className?: string }): React.JSX.Element {
  return (
    <span className={`${styles.badge} ${className ?? ''}`.trim()} aria-hidden="true">
      {count}
    </span>
  )
}
