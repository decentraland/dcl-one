// ShopPurchaseDialog — one step of buying with Credits, as the design's Buy Items and Cart flows draw it:
// Buy Item or Buy N Items (the balance, the items and their prices), Insufficient Funds (what is missing,
// BUY CREDITS), Processing and Purchase successful. Loading, a purchase still being confirmed, a failure
// and the sign-in request aren't drawn in the design; they use the same dialog. Presentational: whoever
// shows it (the web Explorer's shell, which signs the purchase) decides the step and acts on the choice.
//
// Every unit is one row (two of an item are two rows), and the title counts them. Past two rows the list
// scrolls, with the Shop's scroll thumb beside it, as the Cart flow draws it. Purchase successful shows
// up to three of what was bought (the design draws three).

import { useEffect, useRef, useState } from 'react'
import { formatCredits, formatCreditsFull } from '../../../../shop/checkout/lib/currency'
import { Button, ScrollThumb, Spinner } from '../../../design'
import balanceIcon from '../../../assets/shop/modal/balance-credit.svg'
import badge0 from '../../../assets/shop/modal/credits-badge-0.svg'
import badge1 from '../../../assets/shop/modal/credits-badge-1.svg'
import badge2 from '../../../assets/shop/modal/credits-badge-2.svg'
import badge3 from '../../../assets/shop/modal/credits-badge-3.svg'
import badge4 from '../../../assets/shop/modal/credits-badge-4.svg'
import badge5 from '../../../assets/shop/modal/credits-badge-5.svg'
import badge6 from '../../../assets/shop/modal/credits-badge-6.svg'
import badge7 from '../../../assets/shop/modal/credits-badge-7.svg'
import badgeC from '../../../assets/shop/modal/credits-badge-c.svg'
import checkFill from '../../../assets/shop/modal/check-fill.svg'
import checkMark from '../../../assets/shop/modal/check-mark.svg'
import checkRing from '../../../assets/shop/modal/check-ring.svg'
import checkRingOuter from '../../../assets/shop/modal/check-ring-outer.svg'
import floor from '../../../assets/shop/modal/floor.svg'
import successGlow from '../../../assets/shop/modal/success-glow.svg'
import successSpark from '../../../assets/shop/modal/success-spark.svg'
import { CreditsIcon } from './CreditsIcon'
import { ShopItemTile } from './ShopItemTile'
import { ShopModal, ShopModalGap } from './ShopModal'
import styles from './ShopPurchaseDialog.module.css'

export interface ShopPurchaseItem {
  name: string
  /** The creator's display name. */
  creator: string
  rarity: string
  thumbnail?: string
  categoryIcon?: React.ReactNode
  /** The whole credits it costs. */
  credits: number
}

export type ShopPurchaseStep =
  /** `count`: the units being bought (one unless said). */
  | { step: 'loading'; count?: number }
  | { step: 'confirm'; items: ShopPurchaseItem[]; balance: number }
  | { step: 'insufficient'; items: ShopPurchaseItem[]; balance: number; /** Credits short of the price. */ need: number }
  | { step: 'processing'; balance: number | null; count?: number }
  | { step: 'success'; items: ShopPurchaseItem[] }
  | { step: 'pending' }
  | { step: 'failed'; message: string; retry: boolean }
  | { step: 'signIn' }

export interface ShopPurchaseDialogProps {
  view: ShopPurchaseStep
  /** CANCEL, CLOSE or CONTINUE SHOPPING. */
  onClose: () => void
  onConfirm?: () => void
  onBuyCredits?: () => void
  onBackpack?: () => void
  onRetry?: () => void
  onSignIn?: () => void
}

/** How long BUY ITEM is up before it answers. */
export const ARM_MS = 500
/** Rows the list shows before it scrolls, and tiles Purchase successful shows, as drawn. */
const ROWS_SHOWN = 2
const TILES_SHOWN = 3
/** The design's scroll thumb beside the list: 160px. */
const LIST_THUMB = 160

const buyTitle = (count: number): string => (count === 1 ? 'Buy Item' : `Buy ${count} Items`)

/**
 * BUY ITEM, answering only once it has been up a moment: it appears where the loading step's disabled
 * button was, and a click meant for that, or for the card behind, must not spend a credit.
 */
function ArmedButton({ onClick, children }: { onClick?: () => void; children: React.ReactNode }): React.JSX.Element {
  const [armed, setArmed] = useState(false)
  useEffect(() => {
    const timer = setTimeout(() => setArmed(true), ARM_MS)
    return () => clearTimeout(timer)
  }, [])
  return (
    <Button size="modal" onClick={armed ? onClick : undefined} aria-disabled={armed ? undefined : true}>
      {children}
    </Button>
  )
}

export function ShopPurchaseDialog({ view, onClose, onConfirm, onBuyCredits, onBackpack, onRetry, onSignIn }: ShopPurchaseDialogProps): React.JSX.Element {
  const cancel = (
    <Button variant="dark" size="modal" onClick={onClose}>
      Cancel
    </Button>
  )
  switch (view.step) {
    case 'loading': {
      const count = view.count ?? 1
      return (
        <ShopModal label={buyTitle(count)} busy actions={<>{cancel}<Button size="modal" disabled>{count === 1 ? 'Buy item' : 'Buy items'}</Button></>}>
          <h2 className={styles.title}>{buyTitle(count)}</h2>
          <ShopModalGap count={2} />
          <Working label="Checking the price" />
          <ShopModalGap />
        </ShopModal>
      )
    }
    case 'confirm': {
      const count = view.items.length
      return (
        <ShopModal label={buyTitle(count)} actions={<>{cancel}<ArmedButton onClick={onConfirm}>{count === 1 ? 'Buy item' : 'Buy items'}</ArmedButton></>}>
          <h2 className={styles.title}>{buyTitle(count)}</h2>
          <Balance credits={view.balance} />
          <ShopModalGap count={2} />
          <ItemList items={view.items} />
          <ShopModalGap />
        </ShopModal>
      )
    }
    case 'insufficient':
      return (
        <ShopModal label="Insufficient Funds" actions={<>{cancel}<Button size="modal" onClick={onBuyCredits}>Buy credits</Button></>}>
          <CreditsBadge />
          <h2 className={styles.title}>Insufficient Funds</h2>
          <Balance credits={view.balance} />
          <ShopModalGap />
          <p className={styles.note}>
            You will need to buy <strong>{formatCreditsFull(view.need)} Credits</strong> to purchase {view.items.length === 1 ? 'this item' : 'these items'}.
          </p>
          <ShopModalGap />
          <ItemList items={view.items} />
          <ShopModalGap />
        </ShopModal>
      )
    case 'processing':
      return (
        <ShopModal
          label={buyTitle(view.count ?? 1)}
          busy
          actions={
            <Button size="modal" disabled>
              Processing...
            </Button>
          }
        >
          <h2 className={styles.title}>{buyTitle(view.count ?? 1)}</h2>
          {view.balance != null && <Balance credits={view.balance} />}
          <ShopModalGap count={2} />
          <Working label="Processing your purchase" />
          <ShopModalGap />
        </ShopModal>
      )
    case 'success':
      return (
        <ShopModal
          label="Purchase successful!"
          actions={
            <>
              <Button variant="tint" size="modal" onClick={onClose}>
                Continue shopping
              </Button>
              <Button size="modal" onClick={onBackpack}>
                Go to backpack
              </Button>
            </>
          }
        >
          <SuccessBadge />
          <h2 className={styles.title}>Purchase successful!</h2>
          <ShopModalGap />
          <span className={`${styles.bought} ${view.items.length > 1 ? styles.boughtMany : ''}`.trim()}>
            <img className={styles.boughtGlow} src={successGlow} alt="" aria-hidden="true" />
            <img className={styles.boughtSpark} src={successSpark} alt="" aria-hidden="true" />
            <span className={styles.boughtRow}>
              {view.items.slice(0, TILES_SHOWN).map((item, i) => (
                <ShopItemTile key={i} thumbnail={item.thumbnail} rarity={item.rarity} categoryIcon={item.categoryIcon} isNew />
              ))}
            </span>
          </span>
          <ShopModalGap />
        </ShopModal>
      )
    case 'pending':
      return (
        <ShopModal
          label="Purchase in progress"
          actions={
            <Button variant="tint" size="modal" onClick={onClose}>
              Continue shopping
            </Button>
          }
        >
          <h2 className={styles.title}>Purchase in progress</h2>
          <p className={styles.note}>Your purchase was sent and is still being confirmed. It will be in your Backpack once it is.</p>
          <ShopModalGap />
        </ShopModal>
      )
    case 'failed':
      return (
        <ShopModal
          label="Purchase failed"
          actions={
            <>
              <Button variant="dark" size="modal" onClick={onClose}>
                Close
              </Button>
              {view.retry && (
                <Button size="modal" onClick={onRetry}>
                  Try again
                </Button>
              )}
            </>
          }
        >
          <h2 className={styles.title}>Purchase failed</h2>
          <p className={styles.note} role="alert">
            {view.message}
          </p>
          <ShopModalGap />
        </ShopModal>
      )
    case 'signIn':
      return (
        <ShopModal label="Sign in to buy" actions={<>{cancel}<Button size="modal" onClick={onSignIn}>Sign in</Button></>}>
          <h2 className={styles.title}>Sign in to buy</h2>
          <p className={styles.note}>Buying needs an account: sign in to pay with your Credits.</p>
          <ShopModalGap />
        </ShopModal>
      )
  }
}

/** "Your Credits Balance: ©700". */
function Balance({ credits }: { credits: number }): React.JSX.Element {
  return (
    <p className={styles.balance}>
      <span>Your Credits Balance:</span>
      <span className={styles.balanceAmount} title={`${formatCreditsFull(credits)} Credits`}>
        <img src={balanceIcon} alt="" aria-hidden="true" />
        {formatCredits(credits)}
      </span>
    </p>
  )
}

/** The rows, scrolling past ROWS_SHOWN with the design's thumb beside them. */
function ItemList({ items }: { items: ShopPurchaseItem[] }): React.JSX.Element {
  const scroller = useRef<HTMLDivElement>(null)
  if (items.length <= ROWS_SHOWN) {
    return (
      <>
        {items.map((item, i) => (
          <ItemRow key={i} item={item} />
        ))}
      </>
    )
  }
  return (
    <div className={styles.listWrap}>
      <div ref={scroller} className={styles.list} role="list" aria-label={`${items.length} items`}>
        {items.map((item, i) => (
          <div key={i} role="listitem">
            <ItemRow item={item} />
          </div>
        ))}
      </div>
      <ScrollThumb target={scroller} length={LIST_THUMB} className={styles.listThumb} />
    </div>
  )
}

/** The design's CreditsItem: the tile, then the name, creator and rarity over the price. */
function ItemRow({ item }: { item: ShopPurchaseItem }): React.JSX.Element {
  return (
    <div className={styles.item}>
      <ShopItemTile thumbnail={item.thumbnail} rarity={item.rarity} categoryIcon={item.categoryIcon} />
      <div className={styles.info}>
        <div className={styles.about}>
          <span className={styles.name} title={item.name}>
            {item.name}
          </span>
          <span className={styles.creator}>{item.creator}</span>
          <span className={styles.rarity} data-rarity={item.rarity || 'base'}>
            {item.rarity || 'base'}
          </span>
        </div>
        <span className={styles.price} title={`${formatCreditsFull(item.credits)} Credits`}>
          <span className={styles.mark}>
            <CreditsIcon size={28.8} className={styles.markIcon} />
          </span>
          {formatCredits(item.credits)}
        </span>
      </div>
    </div>
  )
}

/** Where the item would be: the spinner, while the price is read or the purchase confirms. */
function Working({ label }: { label: string }): React.JSX.Element {
  return (
    <div className={styles.working} role="status" aria-label={label}>
      <Spinner variant="shop" size={30} />
    </div>
  )
}

/** Insufficient Funds' credit mark on its floor glow, as the design composes it. */
function CreditsBadge(): React.JSX.Element {
  return (
    <span className={styles.badgeImg} aria-hidden="true">
      <img className={styles.floor} src={floor} alt="" />
      <span className={styles.badge}>
        <img className={styles.cb0} src={badge0} alt="" />
        <img className={styles.cb1} src={badge1} alt="" />
        <img className={styles.cb2} src={badge2} alt="" />
        <img className={styles.cb3} src={badge3} alt="" />
        <img className={styles.cb4} src={badge4} alt="" />
        <img className={styles.cb5} src={badge5} alt="" />
        <img className={styles.cb6} src={badge6} alt="" />
        <img className={styles.cb7} src={badge7} alt="" />
        <img className={styles.cbC} src={badgeC} alt="" />
      </span>
    </span>
  )
}

/** Purchase successful's check on its floor glow. */
function SuccessBadge(): React.JSX.Element {
  return (
    <span className={`${styles.badgeImg} ${styles.successImg}`} aria-hidden="true">
      <img className={styles.floor} src={floor} alt="" />
      <span className={styles.badge}>
        <img className={styles.checkRingOuter} src={checkRingOuter} alt="" />
        <img className={styles.checkFill} src={checkFill} alt="" />
        <img className={styles.checkRing} src={checkRing} alt="" />
        <span className={styles.check}>
          <img src={checkMark} alt="" />
        </span>
      </span>
    </span>
  )
}
