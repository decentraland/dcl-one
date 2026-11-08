// ShopPurchaseDialog — one step of buying with Credits, as the design's Buy Items, Cart and Buy NAMEs flows
// and its modals sheet draw it: Buy Item, Buy N Items or Buy Name (the balance, what is bought and its
// price), Insufficient Funds (what is missing, BUY CREDITS), Processing, Purchase successful, and the Error
// modal ("Oops! Something went wrong": what happened, CANCEL and TRY AGAIN when it may be tried again, else
// CLOSE). A NAME is one row whose tile is the NAME's art. Only its simulation is bought here, so a NAME's
// Purchase successful has no next step past CONTINUE SHOPPING.
// A simulated NAME purchase (`simulation`, a testnet rehearsal that buys nothing) says so on every step it
// shows, its failures included: a note that no Credits are spent and the NAME isn't registered or reserved,
// SIMULATE PURCHASE for BUY NAME, and "Simulation complete" (the NAME without its NEW tag, named in its
// note, PREVIEW ON AVATAR) for Purchase successful.
// Loading, a purchase still being confirmed and the sign-in request aren't drawn; they use the same dialog. Presentational: whoever
// shows it (the web Explorer's shell, which signs the purchase) decides the step and acts on the choice.
//
// Every unit is one row (two of an item are two rows), and the title counts them. Past two rows the list
// scrolls, with the Shop's scroll thumb beside it, as the Cart flow draws it. Purchase successful shows
// three of what was bought (the design draws three); more scroll sideways, with the same thumb below.

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
import errorBadge from '../../../assets/shop/modal/error-badge.svg'
import checkMark from '../../../assets/shop/modal/check-mark.svg'
import checkRing from '../../../assets/shop/modal/check-ring.svg'
import checkRingOuter from '../../../assets/shop/modal/check-ring-outer.svg'
import floor from '../../../assets/shop/modal/floor.svg'
import successGlow from '../../../assets/shop/modal/success-glow.svg'
import successSpark from '../../../assets/shop/modal/success-spark.svg'
import { CreditsIcon } from './CreditsIcon'
import { ShopItemTile } from './ShopItemTile'
import { ShopNameTile } from './ShopNameTile'
import { ShopModal, ShopModalGap } from './ShopModal'
import styles from './ShopPurchaseDialog.module.css'

export interface ShopPurchaseItem {
  /** A NAME (its tile is the NAME's art, and it has no creator or rarity), or an item. */
  kind?: 'name'
  name: string
  /** The creator's display name. */
  creator: string
  rarity: string
  thumbnail?: string
  categoryIcon?: React.ReactNode
  /** The whole credits it costs. */
  credits: number
}

/** A simulated NAME purchase's step: nothing is bought (see the header). */
type Simulated = { simulation?: true }

export type ShopPurchaseStep =
  /** `count`: the units being bought (one unless said); `subject: 'name'`: a NAME is. */
  | ({ step: 'loading'; count?: number; subject?: 'name' } & Simulated)
  | ({ step: 'confirm'; items: ShopPurchaseItem[]; balance: number } & Simulated)
  | ({ step: 'insufficient'; items: ShopPurchaseItem[]; balance: number; /** Credits short of the price. */ need: number } & Simulated)
  | ({ step: 'processing'; balance: number | null; count?: number; subject?: 'name' } & Simulated)
  | ({ step: 'success'; items: ShopPurchaseItem[] } & Simulated)
  | { step: 'pending' }
  | ({ step: 'failed'; message: string; retry: boolean } & Simulated)
  | { step: 'signIn' }

export interface ShopPurchaseDialogProps {
  view: ShopPurchaseStep
  /** CANCEL, CLOSE or CONTINUE SHOPPING. */
  onClose: () => void
  onConfirm?: () => void
  onBuyCredits?: () => void
  onBackpack?: () => void
  /** A simulated NAME's PREVIEW ON AVATAR. */
  onPreview?: () => void
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

const buyTitle = (count: number, name = false): string => (name ? 'Buy Name' : count === 1 ? 'Buy Item' : `Buy ${count} Items`)
const buyLabel = (count: number, name = false, simulation = false): string => (simulation ? 'Simulate purchase' : name ? 'Buy name' : count === 1 ? 'Buy item' : 'Buy items')
/** What a simulation spends and keeps: nothing, said before it runs and once it has. */
const SIMULATION_AHEAD = 'Simulation: no Credits will be spent, and this NAME won’t be registered or reserved.'
const SIMULATION_DONE = 'This was a simulation: no Credits were spent, and this NAME wasn’t registered or reserved.'
/** The same, once it ran, naming the NAME it ran for. */
const simulationDone = (items: readonly ShopPurchaseItem[]): string =>
  isName(items) ? `This was a simulation: no Credits were spent, and ${items[0].name} wasn’t registered or reserved.` : SIMULATION_DONE
/** Whether a step's rows are a NAME. */
const isName = (items: readonly ShopPurchaseItem[]): boolean => items.length === 1 && items[0].kind === 'name'

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

export function ShopPurchaseDialog({ view, onClose, onConfirm, onBuyCredits, onBackpack, onPreview, onRetry, onSignIn }: ShopPurchaseDialogProps): React.JSX.Element {
  const cancel = (
    <Button variant="dark" size="modal" onClick={onClose}>
      Cancel
    </Button>
  )
  switch (view.step) {
    case 'loading': {
      const count = view.count ?? 1
      const name = view.subject === 'name'
      return (
        <ShopModal label={buyTitle(count, name)} busy actions={<>{cancel}<Button size="modal" disabled>{buyLabel(count, name, view.simulation)}</Button></>}>
          <h2 className={styles.title}>{buyTitle(count, name)}</h2>
          <ShopModalGap count={2} />
          <Working label="Checking the price" />
          <ShopModalGap />
          <SimulationNote shown={view.simulation} />
        </ShopModal>
      )
    }
    case 'confirm': {
      const count = view.items.length
      const name = isName(view.items)
      return (
        <ShopModal label={buyTitle(count, name)} actions={<>{cancel}<ArmedButton onClick={onConfirm}>{buyLabel(count, name, view.simulation)}</ArmedButton></>}>
          <h2 className={styles.title}>{buyTitle(count, name)}</h2>
          <Balance credits={view.balance} />
          <ShopModalGap count={2} />
          <ItemList items={view.items} />
          <ShopModalGap />
          <SimulationNote shown={view.simulation} />
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
            You will need to buy <strong>{formatCreditsFull(view.need)} Credits</strong> to purchase {isName(view.items) ? 'this Name' : view.items.length === 1 ? 'this item' : 'these items'}.
          </p>
          <ShopModalGap />
          <ItemList items={view.items} />
          <ShopModalGap />
          <SimulationNote shown={view.simulation} />
        </ShopModal>
      )
    case 'processing':
      return (
        <ShopModal
          label={buyTitle(view.count ?? 1, view.subject === 'name')}
          busy
          actions={
            <Button size="modal" disabled>
              Processing...
            </Button>
          }
        >
          <h2 className={styles.title}>{buyTitle(view.count ?? 1, view.subject === 'name')}</h2>
          {view.balance != null && <Balance credits={view.balance} />}
          <ShopModalGap count={2} />
          <Working label={view.simulation ? 'Simulating your purchase' : 'Processing your purchase'} />
          <ShopModalGap />
          <SimulationNote shown={view.simulation} />
        </ShopModal>
      )
    case 'success': {
      const title = view.simulation ? 'Simulation complete' : 'Purchase successful!'
      return (
        <ShopModal
          label={title}
          actions={
            <>
              <Button variant="tint" size="modal" onClick={onClose}>
                Continue shopping
              </Button>
              {view.simulation ? (
                <Button size="modal" onClick={onPreview}>
                  Preview on avatar
                </Button>
              ) : isName(view.items) ? null : (
                <Button size="modal" onClick={onBackpack}>
                  Go to backpack
                </Button>
              )}
            </>
          }
        >
          <SuccessBadge />
          <h2 className={styles.title}>{title}</h2>
          <ShopModalGap />
          <BoughtTiles items={view.items} isNew={!view.simulation} />
          <ShopModalGap />
          {view.simulation && (
            <>
              <p className={styles.simulation} role="note">
                {simulationDone(view.items)}
              </p>
              <ShopModalGap />
            </>
          )}
        </ShopModal>
      )
    }
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
          label="Oops! Something went wrong"
          actions={
            <>
              <Button variant="dark" size="modal" onClick={onClose}>
                {view.retry ? 'Cancel' : 'Close'}
              </Button>
              {view.retry && (
                <Button size="modal" onClick={onRetry}>
                  Try again
                </Button>
              )}
            </>
          }
        >
          <ErrorBadge />
          <h2 className={styles.title}>Oops! Something went wrong</h2>
          <ShopModalGap />
          <p className={styles.message} role="alert">
            <strong>{view.simulation ? "We couldn't complete the simulation this time." : "We couldn't complete your purchase this time."}</strong> {view.message}
          </p>
          <ShopModalGap />
          {view.simulation && (
            <>
              <p className={styles.simulation} role="note">
                {SIMULATION_DONE}
              </p>
              <ShopModalGap />
            </>
          )}
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

/** A simulation's note ahead of it running, under the step it shows on. */
function SimulationNote({ shown }: { shown?: boolean }): React.JSX.Element | null {
  if (!shown) return null
  return (
    <>
      <p className={styles.simulation} role="note">
        {SIMULATION_AHEAD}
      </p>
      <ShopModalGap />
    </>
  )
}

/**
 * What was bought, on the success glow, each tagged NEW (`isNew`); past TILES_SHOWN the row scrolls
 * sideways, the thumb below it.
 */
function BoughtTiles({ items, isNew = true }: { items: ShopPurchaseItem[]; isNew?: boolean }): React.JSX.Element {
  const row = useRef<HTMLSpanElement>(null)
  const scrolls = items.length > TILES_SHOWN
  return (
    <span className={`${styles.bought} ${items.length > 1 ? styles.boughtMany : ''} ${scrolls ? styles.boughtScrolls : ''}`.trim()}>
      <img className={styles.boughtGlow} src={successGlow} alt="" aria-hidden="true" />
      <img className={styles.boughtSpark} src={successSpark} alt="" aria-hidden="true" />
      {/* A row that scrolls takes the focus (Tab reaches it; the arrow keys scroll it). */}
      <span
        ref={row}
        className={styles.boughtRow}
        role={scrolls ? 'list' : undefined}
        aria-label={scrolls ? `${items.length} items bought` : undefined}
        tabIndex={scrolls ? 0 : undefined}
      >
        {items.map((item, i) =>
          scrolls ? (
            <span key={i} role="listitem" aria-label={item.name} className={styles.boughtItem}>
              <Tile item={item} isNew={isNew} />
            </span>
          ) : (
            <Tile key={i} item={item} isNew={isNew} labelled />
          )
        )}
      </span>
      {scrolls && <ScrollThumb target={row} length={LIST_THUMB} axis="x" className={styles.boughtThumb} />}
    </span>
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

/**
 * An item's tile, or a NAME's art. The NAME's art is the same for every NAME, so where nothing beside it
 * names the NAME (`labelled`: a bought tile on its own), it carries the NAME as its label.
 */
function Tile({ item, isNew = false, labelled = false }: { item: ShopPurchaseItem; isNew?: boolean; labelled?: boolean }): React.JSX.Element {
  return item.kind === 'name' ? (
    <ShopNameTile isNew={isNew} label={labelled ? `${item.name}.dcl.eth` : undefined} />
  ) : (
    <ShopItemTile thumbnail={item.thumbnail} rarity={item.rarity} categoryIcon={item.categoryIcon} isNew={isNew} />
  )
}

/**
 * The design's CreditsItem: the tile, then the name, creator and rarity over the price. A NAME's has the
 * NAME and "Decentraland Name", centred 32px above its price.
 */
function ItemRow({ item }: { item: ShopPurchaseItem }): React.JSX.Element {
  const name = item.kind === 'name'
  return (
    <div className={styles.item}>
      <Tile item={item} />
      <div className={`${styles.info} ${name ? styles.infoName : ''}`.trim()}>
        <div className={styles.about}>
          <span className={styles.name} title={item.name}>
            {item.name}
          </span>
          <span className={styles.creator}>{name ? 'Decentraland Name' : item.creator}</span>
          {!name && (
            <span className={styles.rarity} data-rarity={item.rarity || 'base'}>
              {item.rarity || 'base'}
            </span>
          )}
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

/** The Error modal's mark on its floor glow. */
function ErrorBadge(): React.JSX.Element {
  return (
    <span className={`${styles.badgeImg} ${styles.errorImg}`} aria-hidden="true">
      <img className={styles.floor} src={floor} alt="" />
      <img className={styles.errorMark} src={errorBadge} alt="" width={62} height={62} />
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
