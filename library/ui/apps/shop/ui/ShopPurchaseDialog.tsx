// ShopPurchaseDialog — one step of buying in the Shop, in the design system's Explorer modal (ShopModal), as the
// design's "Buy Items · Credits + MANA" and "Cart · Credits + MANA" flows draw it:
//  - Buy Item / Buy N Items: the balance, each item (its card, name, creator and Credits price), then CANCEL and BUY.
//    Where MANA is offered, the same step lists a payment option per currency (Credits, MANA) under the items, with the
//    buyer's balance and what each pays, and a cart adds its total under them. One option is picked at a time (the
//    design's checkboxes also allow both, which isn't offered yet).
//  - Insufficient Funds: what is missing, BUY CREDITS; where MANA is offered, both options shown off, as neither pays.
//  - Confirm in your wallet: while the buyer's own wallet asks them to sign.
//  - Processing, Purchase successful (what was bought, NEW), and the Error modal ("Oops! Something went wrong": what
//    happened, CANCEL and TRY AGAIN when it may be tried again, else CLOSE; or CANCEL and SWITCH TO <network> AND RETRY
//    when the buyer's wallet has to send it from that network).
// A NAME is one row whose card is the NAME's art. Only its simulation is bought here, so a NAME's Purchase successful has
// no next step past CONTINUE SHOPPING. A simulated NAME purchase (`simulation`, a testnet rehearsal that buys nothing)
// says so on every step it shows, its failures included: a note that no Credits are spent and the NAME isn't registered
// or reserved, SIMULATE PURCHASE for BUY NAME, and "Simulation complete" (the NAME without its NEW tag, named in its note,
// PREVIEW ON AVATAR) for Purchase successful.
// Loading, Authorize MANA (the allowance a self-custody wallet grants a sale's venue before its first MANA purchase
// there), a purchase still being confirmed (CHECK AGAIN, or RETRY, armed as BUY) and the sign-in request have no design
// of their own: they use the same modal and its parts. A cart paid in MANA may take more than one payment (one per sale
// venue), each signed on its own: Processing and Authorize MANA then say which one they are for ("Payment 1 of 2").
// Presentational: whoever shows it (the web Explorer's shell, which signs the purchase) decides the step and acts on
// the choice.
//
// Every unit is one row (two of an item are two rows), and the title counts them. Past two rows the list scrolls, with
// the Shop's scroll thumb beside it. Purchase successful shows what was bought three to a row; past two rows it scrolls.

import { useRef, useState } from 'react'
import { formatCreditsFull } from '@shop/checkout/lib/currency'
import { formatMana, formatManaPrice, type ManaWei } from '@shop/model/money'
import { GeneralDecentralandColor, GeneralPolygonMana } from '@dcl/one-ui'
import { ArmedButton, Button, ScrollThumb, Spinner } from '@hud/design'
import balanceCredit from '../assets/modal/balance-credit-small.svg'
import creditsCoin from '../assets/modal/credits-coin.svg'
import illustrationCredits from '../assets/modal/illustration-credits.png'
import illustrationError from '../assets/modal/illustration-error.png'
import illustrationSign from '../assets/modal/illustration-sign.png'
import illustrationSuccess from '../assets/modal/illustration-success.png'
import paymentCheck from '../assets/modal/payment-check.svg'
import priceCredit from '../assets/modal/price-credit.svg'
import { ShopItemCard } from './ShopItemCard'
import { ShopNameTile } from './ShopNameTile'
import { ShopModal, ShopModalHeader } from './ShopModal'
import styles from './ShopPurchaseDialog.module.css'

export interface ShopPurchaseItem {
  /** A NAME (its art in place of a card, "Decentraland Name" in place of a creator). */
  kind?: 'name'
  name: string
  /** The creator's display name. */
  creator: string
  /** The creator's profile picture, when known. */
  creatorFace?: string
  rarity: string
  thumbnail?: string
  categoryIcon?: React.ReactNode
  /** A smart wearable (its SMART badge where it was bought). */
  smart?: boolean
  /** The whole credits it costs. */
  credits: number
  /** The item's identity (its cart line): the units of one item share it, and only they count as one card. */
  key?: string
}

/** A simulated NAME purchase's step: nothing is bought (see the header). */
type Simulated = { simulation?: true }

/** How the buyer pays a purchase: from the Credits balance, or in Polygon MANA. */
export type ShopPaymentMethod = 'credits' | 'mana'

/** The Credits option of Buy Item. */
export interface ShopCreditsChoice {
  /** Whole credits for everything; null when the Credits can't price it. */
  price: number | null
  /** Whole credits; null when it couldn't be read. */
  balance: number | null
  /** Whether the Credits pay it now. */
  pays: boolean
}

/** The Polygon MANA option of Buy Item, in wei. */
export interface ShopManaChoice {
  price: bigint
  balance: bigint
  /** Whether the buyer's MANA pays it now. */
  pays: boolean
}

/** Which of a checkout's payments a step is for: the `at`th of `of`, counted from 1. Said only when there are several. */
export interface ShopPaymentProgress {
  at: number
  of: number
}

export type ShopPurchaseStep =
  /** `count`: the units being bought (one unless said); `subject: 'name'`: a NAME is. */
  | ({ step: 'loading'; count?: number; subject?: 'name' } & Simulated)
  | ({ step: 'confirm'; items: ShopPurchaseItem[]; balance: number } & Simulated)
  /**
   * Insufficient Funds. `need`: the Credits short of the price. `mana`: where MANA is offered, the buyer's MANA and what
   * it would pay, neither enough: both options are shown off.
   */
  | ({ step: 'insufficient'; items: ShopPurchaseItem[]; balance: number; need: number; mana?: { price: bigint; balance: bigint } } & Simulated)
  | ({ step: 'processing'; balance: number | null; count?: number; subject?: 'name'; progress?: ShopPaymentProgress } & Simulated)
  | ({ step: 'success'; items: ShopPurchaseItem[] } & Simulated)
  /**
   * Buy Item with its payment options. `initial`: the buyer's earlier pick (the step shown again after a failure),
   * picked again while it still pays.
   */
  | { step: 'method'; items: ShopPurchaseItem[]; credits: ShopCreditsChoice; mana: ShopManaChoice; initial?: ShopPaymentMethod }
  /**
   * Authorize MANA. `venue`: what the allowance is for, named ("the Marketplace"); `network`: its chain, named;
   * `required`: the MANA this purchase needs, in wei. `waiting`: the wallet or the chain is confirming it; `failed`:
   * it didn't go through (`message` says why), and AUTHORIZE tries again.
   */
  | { step: 'authorize'; venue: string; network: string; required: bigint; state: 'ready' | 'waiting' | 'failed'; message?: string; progress?: ShopPaymentProgress }
  /**
   * Confirm in your wallet: the buyer's own wallet is asking them to sign. `close`: the label of the button the step it
   * covers offered to leave it (CANCEL on Authorize MANA), which leaves it the same way here; absent, nothing to press.
   */
  | { step: 'wallet'; close?: string }
  /**
   * Still being confirmed. `message`: what is known about it, in place of the step's own words. `check`: CHECK AGAIN
   * is offered (`checking` while it looks). `retry`: RETRY is offered in its place (the same purchase sent again, which
   * whoever shows the step makes safe).
   */
  | { step: 'pending'; message?: string; check?: 'ready' | 'checking'; retry?: boolean }
  /** `switchTo`: the network the wallet must move to, named, for SWITCH TO … AND RETRY in place of TRY AGAIN. */
  | ({ step: 'failed'; message: string; retry: boolean; switchTo?: string } & Simulated)
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
  /** SWITCH TO … AND RETRY: the wallet moved to the failed step's network, then the purchase tried again. */
  onSwitchNetwork?: () => void
  onSignIn?: () => void
  /** BUY on Buy Item with its payment options, with the option picked. */
  onPay?: (method: ShopPaymentMethod) => void
  /** AUTHORIZE (or TRY AGAIN) on Authorize MANA. */
  onAuthorize?: () => void
  /** CHECK AGAIN on a purchase still being confirmed. */
  onCheckAgain?: () => void
}

/** Rows the list shows before it scrolls, and the rows of cards Purchase successful shows before it scrolls. */
const ROWS_SHOWN = 2
const CARD_ROWS_SHOWN = 2
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
/** "this item", "this Name", "these items". */
const what = (items: readonly ShopPurchaseItem[]): string => (isName(items) ? 'this Name' : items.length === 1 ? 'this item' : 'these items')

/**
 * The button a step offers to leave it, as each step below draws it, or null where it offers none (Processing, and
 * Confirm in your wallet itself). Confirm in your wallet keeps it, so a wallet that never answers can't trap the buyer.
 */
export function shopStepLeave(view: { step: ShopPurchaseStep['step']; retry?: boolean; switchTo?: string }): string | null {
  switch (view.step) {
    case 'processing':
    case 'wallet':
      return null
    case 'success':
    case 'pending':
      return 'Continue shopping'
    case 'failed':
      return view.retry || view.switchTo != null ? 'Cancel' : 'Close'
    default:
      return 'Cancel'
  }
}

export function ShopPurchaseDialog({
  view,
  onClose,
  onConfirm,
  onBuyCredits,
  onBackpack,
  onPreview,
  onRetry,
  onSwitchNetwork,
  onSignIn,
  onPay,
  onAuthorize,
  onCheckAgain
}: ShopPurchaseDialogProps): React.JSX.Element {
  const cancel = (
    <Button variant="dark" size="dialog" onClick={onClose}>
      Cancel
    </Button>
  )
  switch (view.step) {
    case 'loading': {
      const count = view.count ?? 1
      const name = view.subject === 'name'
      return (
        <ShopModal
          label={buyTitle(count, name)}
          busy
          actions={
            <>
              {cancel}
              <Button size="dialog" disabled>
                {buyLabel(count, name, view.simulation)}
              </Button>
            </>
          }
        >
          <ShopModalHeader>
            <h2 className={styles.title}>{buyTitle(count, name)}</h2>
          </ShopModalHeader>
          <Working label="Checking the price" />
          <SimulationNote shown={view.simulation} />
        </ShopModal>
      )
    }
    case 'confirm': {
      const count = view.items.length
      const name = isName(view.items)
      // BUY ITEM answers only once it has been up a moment (ArmedButton): it appears where the loading step's
      // disabled button was, and a click meant for that, or for the card behind, must not spend a credit.
      return (
        <ShopModal
          label={buyTitle(count, name)}
          actions={
            <>
              {cancel}
              <ArmedButton size="dialog" onClick={onConfirm}>
                {buyLabel(count, name, view.simulation)}
              </ArmedButton>
            </>
          }
        >
          <ShopModalHeader>
            <h2 className={styles.title}>{buyTitle(count, name)}</h2>
            <Balance credits={view.balance} />
          </ShopModalHeader>
          <ItemList items={view.items} />
          <SimulationNote shown={view.simulation} />
        </ShopModal>
      )
    }
    case 'insufficient': {
      const { mana } = view
      return (
        <ShopModal
          label="Insufficient Funds"
          actions={
            <>
              {cancel}
              <Button size="dialog" onClick={onBuyCredits}>
                Buy credits
              </Button>
            </>
          }
        >
          <ShopModalHeader>
            <Illustration kind="credits" />
            <h2 className={styles.title}>Insufficient Funds</h2>
            <Balance credits={view.balance} mana={mana?.balance} />
          </ShopModalHeader>
          <p className={styles.text}>
            {mana ? (
              `Your Credits and MANA are not enough for ${what(view.items)}. Buy Credits or add MANA to continue.`
            ) : (
              <>
                You will need to buy <strong>{formatCreditsFull(view.need)} Credits</strong> to purchase {what(view.items)}.
              </>
            )}
          </p>
          <ItemList items={view.items} />
          {mana && (
            <PaymentOptions
              credits={{ price: view.balance + view.need, balance: view.balance, pays: false }}
              mana={{ price: mana.price, balance: mana.balance, pays: false }}
              picked={null}
            />
          )}
          {mana && view.items.length > 1 && <OrderSummary count={view.items.length} credits={view.balance + view.need} />}
          <SimulationNote shown={view.simulation} />
        </ShopModal>
      )
    }
    case 'processing':
      return (
        <ShopModal
          label={buyTitle(view.count ?? 1, view.subject === 'name')}
          busy
          actions={
            <Button size="dialog" disabled>
              Processing...
            </Button>
          }
        >
          <ShopModalHeader>
            <h2 className={styles.title}>{buyTitle(view.count ?? 1, view.subject === 'name')}</h2>
            {view.balance != null && <Balance credits={view.balance} />}
            <PaymentCount progress={view.progress} />
          </ShopModalHeader>
          <Working label={view.simulation ? 'Simulating your purchase' : 'Processing your purchase'} />
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
              <Button variant="dark" size="dialog" onClick={onClose}>
                Continue shopping
              </Button>
              {view.simulation ? (
                <Button size="dialog" onClick={onPreview}>
                  Preview on avatar
                </Button>
              ) : isName(view.items) ? null : (
                <Button size="dialog" onClick={onBackpack}>
                  Go to backpack
                </Button>
              )}
            </>
          }
        >
          <ShopModalHeader>
            <Illustration kind="success" />
            <h2 className={styles.title}>{title}</h2>
          </ShopModalHeader>
          <BoughtCards items={view.items} isNew={!view.simulation} />
          {view.simulation && (
            <p className={styles.simulation} role="note">
              {simulationDone(view.items)}
            </p>
          )}
        </ShopModal>
      )
    }
    case 'method':
      return <MethodStep view={view} cancel={cancel} onPay={onPay} />
    case 'authorize': {
      const waiting = view.state === 'waiting'
      return (
        <ShopModal
          label="Authorize MANA"
          busy={waiting}
          actions={
            <>
              {cancel}
              {/* It appears where BUY was: a click meant for that one doesn't authorize. */}
              <ArmedButton size="dialog" disabled={waiting} onClick={onAuthorize}>
                {waiting ? 'Authorizing...' : view.state === 'failed' ? 'Try again' : 'Authorize'}
              </ArmedButton>
            </>
          }
        >
          <ShopModalHeader>
            <h2 className={styles.title}>Authorize MANA</h2>
            <PaymentCount progress={view.progress} />
          </ShopModalHeader>
          <div className={styles.texts}>
            <p className={styles.text}>
              To buy with MANA, allow <strong>{view.venue}</strong> on {view.network} to spend MANA from your wallet. This purchase needs{' '}
              <strong>{formatMana(view.required as ManaWei, { group: true })} MANA</strong>.
            </p>
            <p className={styles.text}>
              The allowance is unlimited and stays after this purchase, so buying there again won’t ask for it. You can revoke it at any time in the Shop’s
              Settings on the website.
            </p>
          </div>
          {waiting && <Working label="Waiting for the authorization to confirm" size="small" />}
          {view.state === 'failed' && view.message && (
            <p className={styles.message} role="alert">
              {view.message}
            </p>
          )}
        </ShopModal>
      )
    }
    case 'wallet':
      return (
        <ShopModal
          label="Confirm in your wallet"
          busy
          actions={
            <>
              {view.close && (
                <Button variant="dark" size="dialog" onClick={onClose}>
                  {view.close}
                </Button>
              )}
              {/* The design's button in its loading state: the label kept out of sight, the spinner in its place. */}
              <span className={styles.waiting} role="status" aria-label="Waiting for your wallet">
                <Spinner variant="shop" size={22.667} />
              </span>
            </>
          }
        >
          <ShopModalHeader>
            <Illustration kind="sign" />
            <h2 className={styles.title}>Confirm in your wallet</h2>
          </ShopModalHeader>
          <p className={styles.text}>Sign the transaction in your wallet to complete the purchase. Keep this window open.</p>
        </ShopModal>
      )
    case 'pending':
      return (
        <ShopModal
          label="Purchase in progress"
          actions={
            <>
              <Button variant="dark" size="dialog" onClick={onClose}>
                Continue shopping
              </Button>
              {view.retry ? (
                <ArmedButton size="dialog" onClick={onRetry}>
                  Retry
                </ArmedButton>
              ) : (
                view.check && (
                  <Button size="dialog" disabled={view.check === 'checking'} onClick={onCheckAgain}>
                    {view.check === 'checking' ? 'Checking...' : 'Check again'}
                  </Button>
                )
              )}
            </>
          }
        >
          <ShopModalHeader>
            <h2 className={styles.title}>Purchase in progress</h2>
          </ShopModalHeader>
          {/* A status: what it says changes while the dialog stays up (RETRY opening, minutes later), and is announced. */}
          <p className={styles.text} role="status">
            {view.message ?? 'Your purchase was sent and is still being confirmed. It will be in your Backpack once it is.'}
          </p>
        </ShopModal>
      )
    case 'failed':
      return (
        <ShopModal
          label="Oops! Something went wrong"
          actions={
            <>
              <Button variant="dark" size="dialog" onClick={onClose}>
                {view.retry || view.switchTo != null ? 'Cancel' : 'Close'}
              </Button>
              {view.switchTo != null ? (
                <Button size="dialog" onClick={onSwitchNetwork}>
                  Switch to {view.switchTo} and retry
                </Button>
              ) : (
                view.retry && (
                  <Button size="dialog" onClick={onRetry}>
                    Try again
                  </Button>
                )
              )}
            </>
          }
        >
          <ShopModalHeader>
            <Illustration kind="error" />
            <h2 className={styles.title}>Oops! Something went wrong</h2>
          </ShopModalHeader>
          <p className={styles.message} role="alert">
            <strong>{view.simulation ? "We couldn't complete the simulation this time." : "We couldn't complete your purchase this time."}</strong> {view.message}
          </p>
          {view.simulation && (
            <p className={styles.simulation} role="note">
              {SIMULATION_DONE}
            </p>
          )}
        </ShopModal>
      )
    case 'signIn':
      return (
        <ShopModal
          label="Sign in to buy"
          actions={
            <>
              {cancel}
              <Button size="dialog" onClick={onSignIn}>
                Sign in
              </Button>
            </>
          }
        >
          <ShopModalHeader>
            <h2 className={styles.title}>Sign in to buy</h2>
          </ShopModalHeader>
          <p className={styles.text}>Buying needs an account: sign in to pay with your Credits.</p>
        </ShopModal>
      )
  }
}

/** "Payment 1 of 2": the payment a step is for, in a checkout of several; nothing for a checkout of one. */
function PaymentCount({ progress }: { progress?: ShopPaymentProgress }): React.JSX.Element | null {
  if (!progress || progress.of < 2) return null
  return (
    <p className={styles.count} role="status">
      Payment {progress.at} of {progress.of}
    </p>
  )
}

/** "Your Balance: ©700 ◇3,400": the Credits (— while unread), and the MANA where it is offered. */
function Balance({ credits, mana }: { credits: number | null; mana?: bigint | null }): React.JSX.Element {
  return (
    <p className={styles.balance}>
      <span>Your Balance:</span>
      <img src={creditsCoin} alt="" aria-hidden="true" width={16} height={16} />
      <span className={styles.balanceAmount} title={credits == null ? undefined : `${formatCreditsFull(credits)} Credits`}>
        {credits == null ? '—' : formatCreditsFull(credits)}
      </span>
      {mana != null && (
        <>
          <GeneralPolygonMana size={16} aria-hidden="true" />
          <span className={styles.balanceAmount} title={`${formatMana(mana as ManaWei, { group: true })} MANA`}>
            {formatMana(mana as ManaWei, { group: true, decimals: 2 })}
          </span>
        </>
      )}
    </p>
  )
}

/** A simulation's note ahead of it running, under the step it shows on. */
function SimulationNote({ shown }: { shown?: boolean }): React.JSX.Element | null {
  if (!shown) return null
  return (
    <p className={styles.simulation} role="note">
      {SIMULATION_AHEAD}
    </p>
  )
}

/** The modal-illustration above a title, as the design system's sheet draws each. */
function Illustration({ kind }: { kind: 'success' | 'credits' | 'sign' | 'error' }): React.JSX.Element {
  const src = { success: illustrationSuccess, credits: illustrationCredits, sign: illustrationSign, error: illustrationError }[kind]
  return (
    <span className={styles.illustration} aria-hidden="true">
      <img className={styles[kind]} src={src} alt="" />
    </span>
  )
}

/** Units of the same item, next to each other, as one card with their amount: the steps list every unit. */
function amounts(items: ShopPurchaseItem[]): { item: ShopPurchaseItem; amount: number }[] {
  // By identity, never by looks: two items drawn alike are still two. An item without one stands alone.
  const same = (a: ShopPurchaseItem, b: ShopPurchaseItem): boolean => a.key != null && a.key === b.key
  const out: { item: ShopPurchaseItem; amount: number }[] = []
  for (const item of items) {
    const last = out.at(-1)
    if (last && same(last.item, item)) last.amount++
    else out.push({ item, amount: 1 })
  }
  return out
}

/**
 * What was bought, one card per item with its amount, each tagged NEW (`isNew`), three to a row; past two rows they
 * scroll, the thumb beside them.
 */
function BoughtCards({ items, isNew = true }: { items: ShopPurchaseItem[]; isNew?: boolean }): React.JSX.Element {
  const scroller = useRef<HTMLDivElement>(null)
  const bought = amounts(items)
  const scrolls = bought.length > 3 * CARD_ROWS_SHOWN
  const cards = bought.map(({ item, amount }, i) => (
    <span key={i} className={styles.boughtItem} {...(scrolls ? { role: 'listitem', 'aria-label': amount > 1 ? `${item.name} x${amount}` : item.name } : {})}>
      <Card item={item} isNew={isNew} labelled={!scrolls} size="bought" amount={amount} />
    </span>
  ))
  if (!scrolls) return <div className={styles.bought}>{cards}</div>
  return (
    <div className={styles.listWrap}>
      {/* A list that scrolls takes the focus (Tab reaches it; the arrow keys scroll it). */}
      <div ref={scroller} className={`${styles.bought} ${styles.boughtScrolls}`} role="list" aria-label={`${items.length} items bought`} tabIndex={0}>
        {cards}
      </div>
      <ScrollThumb target={scroller} length={LIST_THUMB} className={styles.listThumb} />
    </div>
  )
}

/** The rows, scrolling past ROWS_SHOWN with the design's thumb beside them. */
function ItemList({ items }: { items: ShopPurchaseItem[] }): React.JSX.Element {
  const scroller = useRef<HTMLDivElement>(null)
  if (items.length <= ROWS_SHOWN) {
    return (
      <div className={styles.list}>
        {items.map((item, i) => (
          <ItemRow key={i} item={item} />
        ))}
      </div>
    )
  }
  return (
    <div className={styles.listWrap}>
      <div ref={scroller} className={`${styles.list} ${styles.listScrolls}`} role="list" aria-label={`${items.length} items`}>
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
 * An item's card, or a NAME's art. The NAME's art is the same for every NAME, so where nothing beside it names the NAME
 * (`labelled`: a bought card on its own), it carries the NAME as its label.
 */
function Card({
  item,
  isNew = false,
  labelled = false,
  size,
  amount = 1
}: {
  item: ShopPurchaseItem
  isNew?: boolean
  labelled?: boolean
  size: 'row' | 'bought'
  amount?: number
}): React.JSX.Element {
  return item.kind === 'name' ? (
    <ShopNameTile isNew={isNew} label={labelled ? `${item.name}.dcl.eth` : undefined} />
  ) : size === 'row' ? (
    <ShopItemCard className={styles.rowCard} thumbnail={item.thumbnail} rarity={item.rarity} categoryIcon={item.categoryIcon} />
  ) : (
    <ShopItemCard
      className={styles.boughtCard}
      thumbnail={item.thumbnail}
      rarity={item.rarity}
      categoryIcon={item.categoryIcon}
      isNew={isNew}
      smart={item.smart === true}
      amount={amount}
    />
  )
}

/**
 * The design's cart-item: the card, then the name and the creator (their picture and name) over the Credits price. A
 * NAME's has the NAME and "Decentraland Name".
 */
function ItemRow({ item }: { item: ShopPurchaseItem }): React.JSX.Element {
  const name = item.kind === 'name'
  return (
    <div className={styles.item}>
      <Card item={item} size="row" />
      <div className={styles.info}>
        <div className={styles.about}>
          <span className={styles.name} title={item.name}>
            {item.name}
          </span>
          <span className={styles.creator}>
            {name ? (
              'Decentraland Name'
            ) : (
              <>
                <span className={styles.face} aria-hidden="true">
                  {item.creatorFace && <img src={item.creatorFace} alt="" />}
                </span>
                <span className={styles.creatorName}>{item.creator}</span>
              </>
            )}
          </span>
        </div>
        {/* An item the Credits can't price (only MANA buys it) shows no Credits price, never "0". */}
        {item.credits > 0 && (
          <span className={styles.price} title={`${formatCreditsFull(item.credits)} Credits`}>
            <img src={creditsCoin} alt="" aria-hidden="true" width={20} height={20} />
            {formatCreditsFull(item.credits)}
          </span>
        )}
      </div>
    </div>
  )
}

/** Where the item would be: the spinner, while the price is read or the purchase confirms. `size="small"`: under text. */
function Working({ label, size }: { label: string; size?: 'small' }): React.JSX.Element {
  return (
    <div className={`${styles.working} ${size === 'small' ? styles.workingSmall : ''}`.trim()} role="status" aria-label={label}>
      <Spinner variant="shop" size={30} />
    </div>
  )
}

/**
 * Buy Item with its payment options: the balance, the items, an option per currency, a cart's total, then CANCEL or BUY.
 * The step opens with a pick: the buyer's earlier one while it pays, else the Credits when they pay, else MANA. After
 * that only the buyer picks, one option at a time (radios): picking one moves the pick to it. A pick that stops
 * paying (a balance read again) is dropped, and BUY waits for the buyer to pick an option that pays: the same BUY never
 * spends another way than the one picked, and a later read where the dropped option pays again doesn't pick it back.
 */
function MethodStep({
  view,
  cancel,
  onPay
}: {
  view: Extract<ShopPurchaseStep, { step: 'method' }>
  cancel: React.ReactNode
  onPay?: (method: ShopPaymentMethod) => void
}): React.JSX.Element {
  const { credits, mana } = view
  const pays = (method: ShopPaymentMethod): boolean => (method === 'credits' ? credits.pays : mana.pays)
  const first = (): ShopPaymentMethod | null => (view.initial && pays(view.initial) ? view.initial : credits.pays ? 'credits' : mana.pays ? 'mana' : null)
  const [picked, setPicked] = useState<ShopPaymentMethod | null>(first)
  // Dropped as this render reads it (React renders again at once), so no later view sees the old pick come back.
  if (picked && !pays(picked)) setPicked(null)
  const method = picked && pays(picked) ? picked : null
  const count = view.items.length
  return (
    <ShopModal
      label={buyTitle(count)}
      actions={
        <>
          {cancel}
          {/* Armed as BUY ITEM is: it appears where the loading step's disabled button was. */}
          <ArmedButton size="dialog" disabled={method == null} onClick={() => method && onPay?.(method)}>
            {buyLabel(count)}
          </ArmedButton>
        </>
      }
    >
      <ShopModalHeader>
        <h2 className={styles.title}>{buyTitle(count)}</h2>
        <Balance credits={credits.balance} mana={mana.balance} />
      </ShopModalHeader>
      <ItemList items={view.items} />
      <PaymentOptions credits={credits} mana={mana} picked={method} onPick={setPicked} />
      {count > 1 && <OrderSummary count={count} credits={method === 'mana' ? null : credits.price} mana={method === 'mana' ? mana.price : null} />}
    </ShopModal>
  )
}

/** MANA per credit at the prices given ("10"), or null where the Credits can't price it. */
function manaPerCredit(mana: bigint, credits: number | null): string | null {
  if (credits == null || credits <= 0 || mana <= 0n) return null
  return formatMana((mana / BigInt(credits)) as ManaWei, { group: true, decimals: 2 })
}

/**
 * The design's payment options: one per currency, each with its mark, its name over the buyer's balance, and what it
 * pays (MANA with its rate to Credits). An option that can't pay is shown off. Radios, drawn as the design's
 * checkboxes; `onPick` absent: none may be picked (Insufficient Funds).
 */
function PaymentOptions({
  credits,
  mana,
  picked,
  onPick
}: {
  credits: ShopCreditsChoice
  mana: ShopManaChoice
  picked: ShopPaymentMethod | null
  onPick?: (method: ShopPaymentMethod) => void
}): React.JSX.Element {
  const rate = manaPerCredit(mana.price, credits.price)
  return (
    // One is picked at a time: radios, drawn as the design's checkboxes.
    <div className={styles.options} role="radiogroup" aria-label="Payment method">
      <PaymentOption
        label="Credits"
        checked={picked === 'credits'}
        disabled={onPick == null || !credits.pays}
        onChange={() => onPick?.('credits')}
        mark={<img src={creditsCoin} alt="" aria-hidden="true" width={32} height={32} />}
        balance={
          <>
            Credits Balance:
            <img src={balanceCredit} alt="" aria-hidden="true" width={12} height={12} />
            <span title={credits.balance == null ? undefined : `${formatCreditsFull(credits.balance)} Credits`}>
              {credits.balance == null ? '—' : formatCreditsFull(credits.balance)}
            </span>
          </>
        }
        price={
          credits.price != null && (
            <span className={styles.optionAmount} title={`${formatCreditsFull(credits.price)} Credits`}>
              <img src={priceCredit} alt="" aria-hidden="true" width={20} height={20} />
              <span className={styles.optionFigure}>{formatCreditsFull(credits.price)}</span>
            </span>
          )
        }
      />
      <PaymentOption
        label="MANA"
        checked={picked === 'mana'}
        disabled={onPick == null || !mana.pays}
        onChange={() => onPick?.('mana')}
        mark={<GeneralDecentralandColor size={32} aria-hidden="true" />}
        balance={
          <>
            MANA Balance:
            <GeneralPolygonMana size={12} aria-hidden="true" />
            <span title={`${formatMana(mana.balance as ManaWei, { group: true })} MANA`}>{formatMana(mana.balance as ManaWei, { group: true, decimals: 2 })}</span>
          </>
        }
        price={
          <>
            <span className={styles.optionAmount} title={`${formatMana(mana.price as ManaWei, { group: true })} MANA`}>
              <GeneralPolygonMana size={20} aria-hidden="true" />
              <span className={styles.optionFigure}>{formatManaPrice(mana.price as ManaWei)}</span>
            </span>
            {rate != null && <span className={styles.optionRate}>1 Credit = {rate} MANA</span>}
          </>
        }
      />
    </div>
  )
}

/** One payment option: a native radio, out of sight, drawn as the design system's filled checkbox. */
function PaymentOption({
  label,
  checked,
  disabled,
  onChange,
  mark,
  balance,
  price
}: {
  label: string
  checked: boolean
  disabled: boolean
  onChange: () => void
  mark: React.ReactNode
  balance: React.ReactNode
  price: React.ReactNode
}): React.JSX.Element {
  return (
    <label className={styles.option} data-checked={checked || undefined} data-disabled={disabled || undefined}>
      <input type="radio" name="shop-payment-method" className={styles.optionInput} checked={checked} disabled={disabled} onChange={onChange} />
      <span className={styles.check} aria-hidden="true">
        <span className={styles.checkBox}>{checked && <img src={paymentCheck} alt="" width={16} height={16} />}</span>
      </span>
      <span className={styles.optionMark}>{mark}</span>
      <span className={styles.optionInfo}>
        <span className={styles.optionLabel}>{label}</span>
        <span className={styles.optionBalance}>{balance}</span>
      </span>
      <span className={styles.optionPrice}>{price}</span>
    </label>
  )
}

/** A cart's total under its options: "Total: N Items" and what it comes to in the option picked. */
function OrderSummary({ count, credits, mana = null }: { count: number; credits: number | null; mana?: bigint | null }): React.JSX.Element {
  return (
    <div className={styles.summary}>
      <span className={styles.summaryRule} aria-hidden="true" />
      <div className={styles.summaryRow}>
        <span>Total: {count} Items</span>
        <span className={styles.summaryPrices}>
          {credits != null && (
            <span className={styles.summaryPrice} title={`${formatCreditsFull(credits)} Credits`}>
              <img src={creditsCoin} alt="" aria-hidden="true" width={16} height={16} />
              {formatCreditsFull(credits)}
            </span>
          )}
          {mana != null && (
            <span className={styles.summaryPrice} title={`${formatMana(mana as ManaWei, { group: true })} MANA`}>
              <GeneralPolygonMana size={16} aria-hidden="true" />
              {formatManaPrice(mana as ManaWei)}
            </span>
          )}
        </span>
      </div>
    </div>
  )
}
