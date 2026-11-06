// ShopPurchaseDialog — one step of buying an item with Credits, as the design's Buy Items flow draws it:
// Buy Item (the balance, the item and its price), Insufficient Funds (what is missing, BUY CREDITS),
// Processing and Purchase successful. Loading, a purchase still being confirmed, a failure and the
// sign-in request aren't drawn in the design; they use the same dialog. Presentational: whoever shows it
// (the web Explorer's shell, which signs the purchase) decides the step and acts on the choice.

import { formatCredits, formatCreditsFull } from '../../../../shop/checkout/lib/currency'
import { Button, Spinner } from '../../../design'
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
  | { step: 'loading' }
  | { step: 'confirm'; item: ShopPurchaseItem; balance: number }
  | { step: 'insufficient'; item: ShopPurchaseItem; balance: number; /** Credits short of the price. */ need: number }
  | { step: 'processing'; balance: number | null }
  | { step: 'success'; item: ShopPurchaseItem }
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

export function ShopPurchaseDialog({ view, onClose, onConfirm, onBuyCredits, onBackpack, onRetry, onSignIn }: ShopPurchaseDialogProps): React.JSX.Element {
  const cancel = (
    <Button variant="dark" size="modal" onClick={onClose}>
      Cancel
    </Button>
  )
  switch (view.step) {
    case 'loading':
      return (
        <ShopModal label="Buy Item" busy actions={<>{cancel}<Button size="modal" disabled>Buy item</Button></>}>
          <h2 className={styles.title}>Buy Item</h2>
          <ShopModalGap count={2} />
          <Working label="Checking the price" />
          <ShopModalGap />
        </ShopModal>
      )
    case 'confirm':
      return (
        <ShopModal label="Buy Item" actions={<>{cancel}<Button size="modal" onClick={onConfirm}>Buy item</Button></>}>
          <h2 className={styles.title}>Buy Item</h2>
          <Balance credits={view.balance} />
          <ShopModalGap count={2} />
          <ItemRow item={view.item} />
          <ShopModalGap />
        </ShopModal>
      )
    case 'insufficient':
      return (
        <ShopModal label="Insufficient Funds" actions={<>{cancel}<Button size="modal" onClick={onBuyCredits}>Buy credits</Button></>}>
          <CreditsBadge />
          <h2 className={styles.title}>Insufficient Funds</h2>
          <Balance credits={view.balance} />
          <ShopModalGap />
          <p className={styles.note}>
            You will need to buy <strong>{formatCreditsFull(view.need)} Credits</strong> to purchase this item.
          </p>
          <ShopModalGap />
          <ItemRow item={view.item} />
          <ShopModalGap />
        </ShopModal>
      )
    case 'processing':
      return (
        <ShopModal
          label="Buy Item"
          busy
          actions={
            <Button size="modal" disabled>
              Processing...
            </Button>
          }
        >
          <h2 className={styles.title}>Buy Item</h2>
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
          <span className={styles.bought}>
            <img className={styles.boughtGlow} src={successGlow} alt="" aria-hidden="true" />
            <img className={styles.boughtSpark} src={successSpark} alt="" aria-hidden="true" />
            <ShopItemTile className={styles.boughtTile} thumbnail={view.item.thumbnail} rarity={view.item.rarity} categoryIcon={view.item.categoryIcon} isNew />
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
