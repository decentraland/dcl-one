// Paying for an in-world Shop purchase in Polygon MANA (./purchase holds the rest of the purchase): the MANA quote
// and balance, BUY with MANA through the web Shop's executor with the wallet bound to the sale (./manaGuard), the
// attempt kept at each checkpoint (./manaPurchase), Authorize MANA, the chain's word on a sale that went
// (./manaEvidence), and RETRY of a sale whose answer was lost. A cart is paid in MANA whole or not at all, one sale per
// venue, run one after another as the Credits' checkout runs its groups. It works on ./purchase's operations, through
// the host that purchase gives it: the dialog, the operation's record and state, the watch, and the shop-wide hold.

import type { Eip1193Provider } from '../../../data/auth/wallet'
import type { ShopClient } from '@shop/api/shop'
import { partitionCart, type CartGroup } from '@shop/cart/partition'
import type { CartStore } from '../../../shop/cart/store'
import type { ChainReader } from '../../../shop/checkout/chain/rpc'
import { reviewCart, type ResolvedLine } from '../../../shop/checkout/purchase/cart'
import type { ApprovalRequest, GroupResult } from '../../../shop/checkout/purchase/executor'
import type { RailContext } from '../../../shop/checkout/purchase/rail'
import { createSpendGuard } from '@shop/checkout/purchase/spend-guard'
import { gaslessEnabled, settlementChainId } from '../../../shop/checkout/chain/config'
import { chainLabel } from '@shop/model/chain'
import { friendlyError, isRejection } from '../../../shop/checkout/lib/errors'
import { setAuthorization } from '../../../shop/checkout/lib/authorizations'
import type { ManaWei, UsdCents } from '@shop/model/money'
import type { CartLine, PurchaseOffer } from '@shop/model/purchase'
import type { Session } from '@shop/model/settlement'
import type { StoredLogin } from '@explorer-core/features/auth/sso'
import { SHOP_PROTOCOL, TERMINAL_STATES, type ShopBuyRequest, type ShopLine, type ShopOperationState } from '@dcl/one-ui/apps/shop/shopProtocol'
import type { ShopPaymentProgress } from '@dcl/one-ui/apps/shop/ui'
import { checkManaTerms, registryFor, type ManaPurchaseIntent, type ManaSaleTerms } from './manaGuard'
import {
  compactManaAttempt,
  createManaAttempt,
  manaDispatched,
  manaFingerprint,
  manaLastDispatch,
  manaHasRoom,
  manaSaleTerms,
  storedManaTerms,
  type ManaAttemptRecord
} from './manaPurchase'
import { boundedReader, deadline, manaIncluded, resolveMana, verifiedVenue } from './manaEvidence'
import { readMetaTxNonce, relaySignedMetaTransaction } from '../../../shop/checkout/chain/metatx'
import { STORAGE_REFUSED, type Ledger } from './ledger'
import { assertWalletAccount, WalletRefusal } from './walletGuard'
import type { Checkout, DialogSlot, ManaQuote, ManaQuotePart, Operation, Quote, QuotePart, ShellPurchaseHandlers, ShellPurchaseView, ShellShopServices, ShellShopWallet } from './purchase'

/** A MANA sale the chain says can no longer run: it never did. */
const MANA_NOT_BOUGHT = "This purchase didn't go through. Nothing was bought and no MANA was spent."
const MANA_NOT_BOUGHT_PART = 'one of its transactions didn’t go through; no MANA was spent for it.'
/** A MANA sale the relay answered, late, that it didn't send. */
const MANA_NOT_SENT = 'This purchase couldn’t go out. Nothing was bought and no MANA was spent.'
const MANA_NOT_SENT_PART = 'one of its transactions couldn’t go out; no MANA was spent for it.'
/** A MANA sale that provably never left this page, as one transaction of a checkout. */
const MANA_NEVER_WENT_PART = 'one of its transactions never went out; no MANA was spent for it.'
/** How a MANA checkout says the rest waited for a sale the executor didn't see confirmed. */
const MANA_AFTER_PENDING_PART = 'the rest wasn’t sent while an earlier part was still being confirmed; no MANA was spent for it.'
/** How a MANA checkout says the rest was cancelled on Authorize MANA. */
const MANA_REST_CANCELLED_PART = 'the rest was cancelled before it was paid; no MANA was spent for it.'
/** What a step in progress adds while part of its checkout is held behind a sale not yet on chain: it never went. */
const restHeld = (cart: boolean): string => ` Part of this purchase wasn’t sent while another part is being confirmed${cart ? '; it’s still in your cart' : ''}.`
/** How a MANA checkout says this page couldn't hold the rest's own operation. */
const MANA_REST_UNHELD_PART = 'this browser couldn’t keep the rest of the purchase to this tab; no MANA was spent for it.'
/** A MANA sale's refusal (nothing of it went), as one transaction of a checkout: it spent no MANA. */
const noManaSpent = (message: string): string => {
  const said = message.trim()
  return /no mana was spent/i.test(said) ? said : `${said}${/[.!?]$/.test(said) ? '' : '.'} No MANA was spent for it.`
}
/** A sale that can't be built for MANA, for one item or several. */
const cannotPayMana = (quote: Quote): string =>
  quote.items.length === 1 ? "This item can't be bought with MANA right now. No MANA was spent." : "These items can't be bought with MANA right now. No MANA was spent."
/** A MANA sale sent through the relay, not yet found on chain. */
const MANA_CHECKING = 'Checking your purchase. You can close this window.'
/** RETRY: the signed execution of the sale, sent again. */
const MANA_SIGN_IN_TO_RETRY = 'Sign in again with the same account to retry this purchase.'
const MANA_RETRY = 'You can retry this same purchase fee-free. It can complete only once.'
/** No room left for another send's evidence: no more retries, and the chain still decides. */
const MANA_NO_MORE_RETRIES = "This purchase can't be retried again here. We're still checking it on chain, and it will show up here once it settles."
/** How long after its latest send a MANA sale not yet on chain may be retried. */
export const RETRY_AFTER_MS = 15 * 60_000
/** How long the buyer's MANA may take to read before MANA is left out of the purchase dialog. */
export const MANA_BALANCE_TIMEOUT_MS = 4000

type Resolved = Parameters<ShellShopServices['run']>[2]

/** What ./purchase lends the MANA flow: its services and record, the dialog, and its operations' own steps. */
export interface ManaFlowHost {
  svc: ShellShopServices
  ledger: Ledger
  slot: DialogSlot
  /** The wallet the host injected, read when it is needed. */
  wallet(): ShellShopWallet | null
  signedIn(): { login: StoredLogin; buyer: string } | null
  loginKey(login: StoredLogin): string
  show(op: Operation, view: ShellPurchaseView, handlers: ShellPurchaseHandlers): void
  hide(op: Operation): void
  fail(op: Operation, message: string, retry?: () => void): void
  close(op: Operation, state: ShopOperationState): void
  move(op: Operation, state: ShopOperationState, extra?: Partial<Operation>): void
  publish(op: Operation): void
  keep(op: Operation): boolean
  settled(op: Operation, state: 'succeeded' | 'failed' | 'cancelled', extra?: Partial<Operation>, message?: string, part?: string): void
  restart(op: Operation, req: ShopBuyRequest, ended: 'cancelled' | 'failed'): void
  reconcile(op: Operation): Promise<void>
  check(op: Operation & { buyer: string }, store: CartStore): Promise<boolean>
  checkAgain(op: Operation, view: (check: 'ready' | 'checking') => void): Promise<void>
  reviewPart(part: QuotePart, quote: Quote): Promise<{ resolved: Resolved } | { why: string; retry: 'cancelled' | null }>
  stillPending(op: Operation): boolean
  /** A new operation for `items` (one sale of a cart), held by this page before anything of it is stored. */
  create(req: ShopBuyRequest, buyer: string | null, items: ShopLine[]): Promise<Operation>
  /** A checkout of several sales, as its dialog shows it: in progress, successful, or what went and what didn't. */
  presentCheckout(co: Checkout): void
  /** Whether a confirmed purchase is running: nothing else opens or retries meanwhile. */
  busy(): boolean
  /** Takes the shop-wide hold; the release it returns lets go of it once, however often it is called. */
  hold(): () => void
}

export interface ManaFlow {
  /** Purchase in progress, for an operation paid in MANA: CHECK AGAIN, RETRY, sign in as the buyer, or no more retries. */
  showPending(op: Operation): void
  /** One look at an unsettled purchase paid in MANA: true once it settled. */
  checkMana(op: Operation & { buyer: string }, record: ManaAttemptRecord): Promise<boolean>
  readManaInTime(buyer: string): Promise<bigint | null>
  quoteMana(
    cartLines: CartLine[],
    offers: Record<string, PurchaseOffer | null | undefined>,
    session: Session,
    buyer: string,
    client: ShopClient,
    creditsBalance: UsdCents | null,
    manaBalance: bigint
  ): Promise<ManaQuoted | null>
  settleMana(op: Operation, req: ShopBuyRequest, quote: Quote): Promise<void>
}

/** quoteMana's answer: the lines priced in MANA, one sale per group in the order they run, and the buyer's MANA. */
export interface ManaQuoted {
  priced: { line: CartLine; offer: PurchaseOffer }[]
  sales: { group: CartGroup; terms: ManaSaleTerms; fingerprint: string; price: bigint }[]
  /** Every sale's MANA together, in wei. */
  price: bigint
  balance: bigint
  pays: boolean
}

/**
 * How one sale's run ended, for whoever shows it. `went`: it was handed off (or the executor says it went), pending and
 * followed by the watch; `confirmed` once the executor saw it confirmed. `settled`: the watch, or a late refusal on a
 * closed dialog, already ended it. `cancelled`: closed on Authorize MANA, nothing of it went. `refused`: nothing of it
 * went, and it is back to reviewing; `retry` says how TRY AGAIN may follow.
 */
type ManaRun = { kind: 'went'; confirmed: boolean } | { kind: 'settled' } | { kind: 'cancelled' } | { kind: 'refused'; message: string; retry: 'cancelled' | 'failed' | null }

/** One sale as the chain and the server read it now: its lines resolved, and its terms. */
type ManaSale = { resolved: Resolved; terms: ManaSaleTerms }

export function createManaFlow(host: ManaFlowHost): ManaFlow {
  const { svc, ledger, slot, wallet: injected, signedIn, loginKey, show, hide, fail, close, move, publish, keep, settled, restart, reconcile, check, checkAgain, reviewPart, stillPending, create, presentCheckout, busy } = host

  /**
   * Purchase in progress. One paid in MANA is being checked on chain (CHECK AGAIN looks now). Once none of its sends is
   * on its way and the latest went RETRY_AFTER_MS ago with nothing on chain yet, this page (holding it) offers RETRY:
   * the signed execution, sent again, so it can complete only once.
   */
  const showPending = (op: Operation): void => {
    const kind = manaPendingKind(op)
    op.manaShown = kind
    // While part of its checkout was held behind a sale not yet on chain, whichever of its sales is shown says so, whatever
    // else it offers: settling them may take a long while. CHECK AGAIN and RETRY stay this sale's own.
    const held = op.checkout?.heldBy ? restHeld(op.checkout.cart) : ''
    if (kind === 'retry') {
      show(op, { step: 'pending', message: MANA_RETRY + held, retry: true }, { onClose: () => hide(op), onRetry: () => void retryMana(op) })
      return
    }
    const message = (kind === 'capped' ? MANA_NO_MORE_RETRIES : kind === 'signin' ? MANA_SIGN_IN_TO_RETRY : MANA_CHECKING) + held
    const view = (check: 'ready' | 'checking'): void => show(op, { step: 'pending', check, message }, { onClose: () => hide(op), onCheckAgain: () => void checkAgain(op, view) })
    view('ready')
  }
  /**
   * What a MANA purchase's Purchase in progress offers now: RETRY once it may and the wait passed, and only for its buyer
   * signed in here (anyone else is asked to sign in as them), else CHECK AGAIN.
   */
  const manaPendingKind = (op: Operation): 'check' | 'retry' | 'signin' | 'capped' => {
    const latest = op.mana ? manaLastDispatch(op.mana) : null
    if (!op.mana || !manaRetryable(op) || latest == null || svc.now() - latest < RETRY_AFTER_MS) return 'check'
    if (!manaHasRoom(compactManaAttempt(op.mana))) return 'capped'
    return signedIn()?.buyer === op.buyer ? 'retry' : 'signin'
  }
  /** Whether this page may retry the MANA purchase now: it holds it, nothing runs, and the chain read last allows it. */
  const manaRetryable = (op: Operation): boolean =>
    !!op.mana && !!op.release && !op.foreign && !busy() && !op.manaSending && op.manaRetry === true && !TERMINAL_STATES.includes(op.state)

  /**
   * One look at an unsettled purchase paid in MANA: true once it settled. A sale none of whose sends was dispatched
   * provably never left this page (cancelled). Otherwise only the chain ends it (./manaEvidence): the execution of a
   * send this page signed, or its nonce consumed by something else. Anything less keeps it as it is, and a dialog up
   * for it shows whether RETRY is open now.
   */
  const checkMana = async (op: Operation & { buyer: string }, record: ManaAttemptRecord): Promise<boolean> => {
    // Back to reviewing: its sale provably didn't go (a marked refusal), and the dialog up for it says so with TRY AGAIN.
    // That dialog ends it (failed, or a fresh attempt); the watch stops here.
    if (op.state === 'reviewing') return true
    // Held here, with no send of it in flight on this page: a send left `signing` has no sender to finish it (the page
    // that signed it went, or its retry ended before its request), so what can't be evidence is dropped first, and the
    // record is judged as it then stands.
    let attempt = record
    if (op.release && !op.foreign && !op.manaSending) {
      const kept = compactManaAttempt(record)
      if (JSON.stringify(kept) !== JSON.stringify(record)) {
        // Judged only once storage holds the tidied record (read back); until then, a sale that went is still read
        // on chain from the record as it stands, and anything else waits for the next look.
        if (keeper(op, { latch: false })(kept)) attempt = kept
        else if (!manaDispatched(record)) return false
      }
    }
    // The relay refused every send that went (answering, maybe, after the send's deadline): proven unsent, it ends
    // failed (on the dialog still up for it, with TRY AGAIN; quietly when that was closed), not as one that never left.
    // One sale of a checkout ends through the checkout, which says what went and what didn't, even when its own step
    // is the one up: TRY AGAIN would buy that sale's items alone.
    if (!op.manaSending && !manaDispatched(attempt) && attempt.sends.some((s) => s.stage === 'refused') && !attempt.sends.some((s) => s.stage === 'signing')) {
      if (slot.holds(op) && !op.checkout) {
        op.state = 'reviewing'
        publish(op)
        const req: ShopBuyRequest = { v: SHOP_PROTOCOL, items: op.items, method: 'credits', ...(op.cart ? { cart: true as const } : {}), ...(op.input ? { input: op.input } : {}) }
        fail(op, MANA_NOT_SENT, () => restart(op, req, 'failed'))
      } else settled(op, 'failed', {}, MANA_NOT_SENT, MANA_NOT_SENT_PART)
      return true
    }
    if (!manaDispatched(attempt)) {
      // A send still `signing` here belongs to a live sender (this page's own), or to a page that holds it elsewhere.
      if (op.manaSending || attempt.sends.some((s) => s.stage === 'signing')) return false
      settled(op, 'cancelled', {}, undefined, MANA_NEVER_WENT_PART)
      return true
    }
    if (!op.chainId) return false
    const terms = storedManaTerms(attempt)
    let venue
    try {
      venue = registryFor({ chainId: op.chainId, sale: terms }).venue
    } catch {
      return false
    }
    if (!venue.metaTx) return false
    const resolution = await resolveMana(attempt, { chainId: op.chainId, buyer: op.buyer, venue: { address: venue.address, metaTx: venue.metaTx }, evidence: svc.manaEvidence() })
    // Settled, or put back on a dialog that ends it, while the chain was read: this look has nothing left to say.
    if (!stillPending(op)) return true
    if (resolution.kind === 'succeeded') {
      settled(op, 'succeeded', { txHash: resolution.txHash })
      return true
    }
    if (resolution.kind === 'failed') {
      settled(op, 'failed', {}, MANA_NOT_BOUGHT, MANA_NOT_BOUGHT_PART)
      return true
    }
    op.manaRetry = resolution.retry
    // The dialog up for it follows what it may offer now: RETRY once the wait passed, or no longer.
    if (slot.holds(op) && !busy() && !TERMINAL_STATES.includes(op.state) && manaPendingKind(op) !== op.manaShown) showPending(op)
    return false
  }

  /** The buyer's MANA on the chain the page settles on, or null when it can't be read in time: then MANA isn't offered. */
  const readManaInTime = (buyer: string): Promise<bigint | null> =>
    deadline(
      Promise.resolve().then(() => svc.manaBalance(buyer, settlementChainId())),
      MANA_BALANCE_TIMEOUT_MS,
      'The MANA balance read'
    ).catch(() => null)

  /**
   * Paying in MANA, or null where it can't be offered: an item with no MANA price, a sale the chain and the server
   * can't settle now, or one the MANA guard can't bound (a USD-priced listing, a venue the registry doesn't list, a
   * chain phase 1 doesn't buy on). A cart is paid in MANA whole or not at all, so one item MANA can't buy leaves MANA
   * out for all of it. Each group is one sale (the Collection Store's mints together, a Marketplace's trades together),
   * built here from the lines as they read now and pinned: paying it later takes exactly that sale, or asks for a new
   * confirmation. A balance short of the total still offers it.
   */
  const quoteMana = async (
    cartLines: CartLine[],
    offers: Record<string, PurchaseOffer | null | undefined>,
    session: Session,
    buyer: string,
    client: ShopClient,
    creditsBalance: UsdCents | null,
    manaBalance: bigint
  ): Promise<ManaQuoted | null> => {
    // Only through the relay (a sale that can be retried at its nonce): none with the relay off.
    if (!(svc.rail().gasless ?? gaslessEnabled())) return null
    const priced = cartLines.map((line) => ({ line: { ...line, method: 'mana' as const }, offer: offers[line.id]! }))
    const partition = partitionCart(priced, session, { creditsBalance, manaBalance: manaBalance as ManaWei })
    if (partition.refused.length > 0 || partition.groups.length === 0) return null
    const groups: { group: CartGroup; price: bigint }[] = []
    for (const group of partition.groups) {
      if (group.key.method !== 'mana' || group.total.method !== 'mana' || group.total.amount <= 0n) return null
      groups.push({ group, price: group.total.amount })
    }
    const sales: ManaQuoted['sales'] = []
    try {
      const review = await reviewCart(priced, session, buyer, svc.resolvers(client))
      const resolved = new Map(review.buyable.map((r) => [r.line.id, r]))
      for (const { group, price } of groups) {
        const lines = group.lineIds.map((id) => resolved.get(id))
        if (lines.some((l) => l == null)) return null
        const terms = manaSaleTerms(group, lines as ResolvedLine[], buyer)
        checkManaTerms({ buyer, chainId: group.key.chainId, sale: terms })
        sales.push({ group, terms, fingerprint: manaFingerprint(terms), price })
      }
    } catch (e) {
      console.warn('[shell/shop] MANA is not offered for this purchase', e)
      return null
    }
    // Only at venues whose execution evidence is verified: anywhere else a lost answer could never end.
    if (sales.some((s) => !verifiedVenue(s.group.key.chainId, s.terms.venue))) return null
    const price = sales.reduce((sum, s) => sum + s.price, 0n)
    return { priced, sales, price, balance: manaBalance, pays: manaBalance >= price }
  }

  /**
   * BUY with MANA picked: each sale paid from the buyer's own MANA through the web Shop's executor (runManaGroup), with
   * the wallet bound to the sale the buyer was shown (./manaGuard) and the attempt kept at each checkpoint
   * (./manaPurchase). A first MANA purchase at a venue asks for its allowance first (Authorize MANA). Whatever may have
   * gone out is followed to its end and never sent again; what provably didn't may be tried again. One sale is the
   * operation's own; a cart of several runs them as a checkout (runManaCart).
   */
  const settleMana = async (op: Operation, req: ShopBuyRequest, quote: Quote): Promise<void> => {
    op.unwatchWallet?.()
    const mana = quote.mana
    if (!mana || mana.parts.length === 0) return
    const again = (): void => restart(op, req, 'cancelled')
    const current = (): boolean => {
      const who = signedIn()
      return !!who && who.buyer === quote.buyer && loginKey(who.login) === quote.login && injected()?.kind(quote.buyer) === quote.kind && !quote.stale
    }
    const wallet = injected()?.provider(quote.buyer)
    if (!wallet || !current()) {
      fail(op, quote.stale ?? 'Your sign-in changed meanwhile. Open the item again to buy it.')
      return
    }
    try {
      await assertWalletAccount(wallet, quote.buyer, true)
    } catch (e) {
      if (op.state !== 'reviewing' || !slot.holds(op)) return
      fail(op, e instanceof WalletRefusal ? e.message : "Your wallet didn't answer. Try again.", again)
      return
    }
    if (op.state !== 'reviewing' || !slot.holds(op)) return
    if (!current()) {
      fail(op, quote.stale ?? 'Your sign-in changed meanwhile. Open the item again to buy it.')
      return
    }
    if (!ledger.writable()) {
      fail(op, STORAGE_REFUSED, again)
      return
    }
    // The shop-wide hold. One sale lets go of it once it is kept as gone (or this ends): the operation's own lock keeps
    // the purchase. A cart keeps it until its last sale has run.
    const release = host.hold()
    try {
      if (mana.parts.length > 1) await runManaCart(op, req, quote, mana, wallet, current)
      else {
        const run = await runMana(op, quote, mana.parts[0], wallet, current, { release })
        if (run.kind === 'cancelled') close(op, 'cancelled')
        else if (run.kind === 'refused') fail(op, run.message, run.retry === 'cancelled' ? again : run.retry === 'failed' ? () => restart(op, req, 'failed') : undefined)
      }
    } finally {
      release()
      // Followed once the dialog says so: a watch that settles at once then updates it.
      if ((op.state as ShopOperationState) === 'pending') void reconcile(op)
    }
  }

  /**
   * BUY with MANA for a cart whose items take several sales (one per venue and call: the Collection Store's mints, a
   * Marketplace's trades), as the Credits' checkout runs its groups. Every sale is reviewed against the one the buyer was
   * shown before any runs, so one that changed is refused with nothing spent, never found after another was paid. Then
   * each runs as an operation of its own (its record, lock and attempt), one after another under the one confirmation
   * and the shop-wide hold, the dialog saying which payment it is on. The next sale starts only once the one before it
   * is on chain as itself (the executor saw it confirmed, and the transaction is that sale's own execution past its
   * checkpoint), final or not: one handed off and not shown so holds the rest (a later sale at the same venue would sign
   * at its nonce), what comes after it never starts, and its step says so. A sale that didn't go ends the checkout the
   * same way. What went stands, each followed by the watch to the chain's finalized word, and the dialog shows the
   * checkout as a whole.
   */
  const runManaCart = async (op: Operation, req: ShopBuyRequest, quote: Quote, mana: ManaQuote, wallet: Eip1193Provider, current: () => boolean): Promise<void> => {
    const again = (): void => restart(op, req, 'cancelled')
    const of = mana.parts.length
    show(op, { step: 'processing', balance: null, count: quote.items.length, progress: { at: 1, of } }, { onClose: () => {} })
    const sales: ManaSale[] = []
    for (const sale of mana.parts) {
      const sold = await saleOf(sale, quote)
      if ('why' in sold) {
        fail(op, sold.why, sold.retry === 'cancelled' ? again : undefined)
        return
      }
      sales.push(sold)
    }
    const co: Checkout = { ops: [], displays: quote.items, parts: of, cart: req.cart === true, failures: [], mana: true }
    for (const [i, sale] of mana.parts.entries()) {
      let run = op
      if (i > 0) {
        // An earlier sale the watch already found never ran (its nonce went to another call): the checkout ends there.
        if (co.ops.some((o) => o.state === 'failed')) break
        run = await create(req, quote.buyer, sale.part.items)
        slot.hand(run)
      }
      run.checkout = co
      co.ops.push(run)
      // A later sale's own lock (the first one's was checked when it opened).
      if (i > 0 && !run.release) {
        co.failures.push(MANA_REST_UNHELD_PART)
        move(run, 'cancelled')
        break
      }
      const ran = await runMana(run, quote, sale, wallet, current, { release: () => {}, cart: { progress: { at: i + 1, of }, sale: sales[i] } })
      if (ran.kind === 'went') {
        // The last: nothing comes after it to wait for it.
        if (i === of - 1) break
        // Confirmed, and found on chain as this sale itself past its checkpoint: its nonce is spent by it, so the next may
        // go, while the watch waits for the chain to finalize it.
        const onChain = ran.confirmed && stillPending(run) && (await included(run))
        // The watch may have ended it while that was read: its end decides. Bought goes on; not bought stops, its reason
        // kept by the checkout.
        if (run.state === 'succeeded') continue
        if (!stillPending(run)) break
        if (onChain) continue
        // Handed off and not shown on chain as itself: what comes after it never starts, and the step shown says so.
        co.failures.push(MANA_AFTER_PENDING_PART)
        co.heldBy = run
        break
      }
      // Ended already: bought, or (its reason kept by the checkout) not, and then nothing more starts.
      if (ran.kind === 'settled') {
        if (run.state === 'succeeded') continue
        break
      }
      if (i === 0) {
        // Nothing of the checkout went: as one purchase that didn't, and TRY AGAIN reads the whole cart's terms anew.
        if (ran.kind === 'cancelled') close(op, 'cancelled')
        else fail(op, ran.message, ran.retry === 'cancelled' ? again : ran.retry === 'failed' ? () => restart(op, req, 'failed') : undefined)
        return
      }
      // Nothing of this sale went; what came before stands, and what comes after never starts.
      co.failures.push(ran.kind === 'cancelled' ? MANA_REST_CANCELLED_PART : noManaSpent(ran.message))
      move(run, ran.kind === 'cancelled' ? 'cancelled' : 'failed')
      break
    }
    presentCheckout(co)
  }

  /** Whether `op`'s sale is on chain as itself past its checkpoint, final or not (./manaEvidence manaIncluded). */
  const included = async (op: Operation): Promise<boolean> => {
    if (!op.mana || !op.buyer || !op.chainId) return false
    try {
      const { venue } = registryFor({ chainId: op.chainId, sale: storedManaTerms(op.mana) })
      if (!venue.metaTx) return false
      return await manaIncluded(op.mana, { chainId: op.chainId, buyer: op.buyer, venue: { address: venue.address, metaTx: venue.metaTx }, evidence: svc.manaEvidence() })
    } catch (e) {
      console.warn('[shell/shop] could not read whether a MANA sale is on chain', e)
      return false
    }
  }

  /**
   * One sale as the chain and the server read it now: the one the buyer was shown (its lines and its exact terms), or
   * why it can't be bought, and whether TRY AGAIN may read the terms anew.
   */
  const saleOf = async (sale: ManaQuotePart, quote: Quote): Promise<ManaSale | { why: string; retry: 'cancelled' | null }> => {
    let review: Awaited<ReturnType<typeof reviewPart>>
    try {
      review = await reviewPart(sale.part, quote)
    } catch (e) {
      console.warn('[shell/shop] could not review the MANA purchase', e)
      return { why: "We couldn't start this purchase. No MANA was spent.", retry: 'cancelled' }
    }
    if ('why' in review) return review
    const { resolved } = review
    const { group } = sale.part
    const lines = group.lineIds.map((id) => resolved.get(id))
    let terms: ManaSaleTerms
    try {
      if (lines.some((l) => l == null)) throw new Error('A line of the sale could not be resolved')
      terms = manaSaleTerms(group, lines as ResolvedLine[], quote.buyer)
    } catch (e) {
      console.warn('[shell/shop] could not build the MANA sale', e)
      return { why: cannotPayMana(quote), retry: 'cancelled' }
    }
    if (manaFingerprint(terms) !== sale.fingerprint) {
      const what = quote.items.length === 1 ? 'Its price or terms changed.' : 'The price or terms of one of these items changed.'
      return { why: `${what} Try again to see the new ones. No MANA was spent.`, retry: 'cancelled' }
    }
    return { resolved, terms }
  }

  /**
   * One sale, as `op`'s own, through the executor: never sent twice, and followed once it went. Alone (no `cart`), it
   * shows itself in progress once kept as gone and lets go of the shop-wide hold (`release`) there; in a cart, the
   * checkout goes on under Processing ("Payment 2 of 3") with the sale it reviewed (`cart.sale`), and keeps the hold.
   * Whoever ran it shows how it ended (ManaRun).
   */
  const runMana = async (
    op: Operation,
    quote: Quote,
    sale: ManaQuotePart,
    wallet: Eip1193Provider,
    current: () => boolean,
    opts: { release: () => void; cart?: { progress: ShopPaymentProgress; sale: ManaSale } }
  ): Promise<ManaRun> => {
    const { part } = sale
    const { group } = part
    const chainId = group.key.chainId
    const { cart } = opts
    /** Nothing of it went out: back to reviewing, so TRY AGAIN (a fresh read of the terms) may follow. */
    const refused = (message: string, retry: 'cancelled' | 'failed' | null = 'cancelled'): ManaRun => {
      op.state = 'reviewing'
      publish(op)
      return { kind: 'refused', message, retry }
    }
    const working: ShellPurchaseView = { step: 'processing', balance: null, count: quote.items.length, ...(cart ? { progress: cart.progress } : {}) }
    op.items = part.items
    op.displays = part.displays
    op.chainId = chainId
    move(op, 'processing')
    if (ledger.failed(op.id) || ledger.saved(op.buyer ?? '', op.id)?.state !== 'processing') return refused(STORAGE_REFUSED)
    show(op, working, { onClose: () => {} })
    // The sale as the chain and the server read it now: the one the buyer was shown, or a new confirmation.
    const sold = cart?.sale ?? (await saleOf(sale, quote))
    if ('why' in sold) return refused(sold.why, sold.retry)
    const { resolved, terms } = sold
    // The operation's own cart holds the group's MANA lines; its record holds the attempt before the wallet is asked anything.
    const store = (op.store ??= ledger.storeFor(op))
    store.clear()
    store.restore(part.priced.map((p) => p.line))
    const keepAttempt = keeper(op)
    // The checkpoint is a finalized block: no reorg can move the sale below it.
    const timeouts = svc.manaTimeouts()
    const attempt = createManaAttempt(terms, keepAttempt, {
      mana: registryFor({ chainId, sale: terms }).mana.address,
      checkpoint: () => svc.manaEvidence().finalized(chainId),
      now: svc.now,
      sendMs: timeouts.sendMs,
      // One record for the operation: a retry's attempt and this one change the same record, never a copy of it.
      load: () => op.mana,
      // Kept as gone: pending from here and followed by the watch, however long the relay and the executor take. Alone,
      // it shows as in progress, closable; in a cart, Processing stays up until the checkout has run.
      onDispatch: () => {
        if (op.state === 'processing') move(op, 'pending')
        if (!cart && slot.holds(op) && !TERMINAL_STATES.includes(op.state)) showPending(op)
        opts.release()
        void reconcile(op)
      }
    })
    if (!keepAttempt(structuredClone(attempt.record))) return refused(STORAGE_REFUSED)
    const intent: ManaPurchaseIntent = {
      buyer: quote.buyer,
      chainId,
      sale: terms,
      current: () => current() && attempt.open(),
      handOff: (a) => attempt.handOff(a),
      mayApprove: () => attempt.approving()
    }
    let provider: Eip1193Provider
    try {
      provider = svc.manaGuard(attempt.observe(wallet), intent)
    } catch (e) {
      return refused(e instanceof WalletRefusal ? `${e.message} No MANA was spent.` : cannotPayMana(quote))
    }
    const rail = svc.rail()
    const relay = attempt.relay(rail.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a)))
    // Only through the relay: a sale whose answer is lost can then be retried at its nonce and land once.
    // Each read bounded too: a receipt read that stalls gives way, and the rails' own wait ends on its deadline.
    const ctx: RailContext = { buyer: quote.buyer, provider, reader: boundedReader(svc.reader(), timeouts.readMs), client: quote.client, walletKind: quote.kind, ...rail, fetchImpl: relay, relayOnly: true }
    let unmirror = (): void => {}
    let result: GroupResult
    try {
      store.startCheckout([group])
      const mirror = (): void => {
        const progress = store.getSnapshot().checkout?.groups[0]
        if (!progress) return
        op.progress = { ...progress }
        keep(op)
      }
      mirror()
      unmirror = store.subscribe(mirror)
      op.manaSending = true
      try {
        result = await svc.run(
          { ctx, store, guard: createSpendGuard(), requestApproval: (request) => authorize(op, request, ctx, terms, attempt, working, cart?.progress) },
          group,
          resolved
        )
      } finally {
        op.manaSending = false
      }
      unmirror()
    } catch (e) {
      unmirror()
      console.warn('[shell/shop] the MANA purchase stopped unexpectedly', e)
      if (attempt.dispatched()) {
        move(op, 'pending')
        if (!cart) showPending(op)
        return { kind: 'went', confirmed: false }
      }
      return refused("We couldn't start this purchase. No MANA was spent.")
    }
    // Cancelled while it asked for the allowance: nothing of the sale went, and nothing more goes. Any evidence the sale
    // went (a dispatch, a hash, a confirmation) is followed below instead, whatever the dialog was asked.
    if (!attempt.open() && result.status === 'failed' && result.failure.kind !== 'reverted' && !attempt.dispatched()) return { kind: 'cancelled' }
    // A sale that went (or that the executor says went): whatever it read (a receipt for the relay's hash, a revert, no
    // answer), only the chain's evidence for this sale's own signed execution settles it (./manaEvidence). A reverted
    // transaction doesn't retire the sale's nonce: the sale may still run, so it is followed like any other.
    // Settled meanwhile by the watch (the chain answered first): nothing left to do here.
    if (TERMINAL_STATES.includes(op.state)) return { kind: 'settled' }
    // A sale that went: pending, as its dispatch already showed it. The watch reads the chain; one look now too, with
    // whatever the relay answered since (its hash), rather than at the watch's next turn.
    if (attempt.dispatched() || result.status === 'confirmed' || result.status === 'pending') {
      if (op.state !== 'pending') {
        move(op, 'pending')
        if (!cart && slot.holds(op)) showPending(op)
      }
      void check(op as Operation & { buyer: string }, store).catch(() => false)
      return { kind: 'went', confirmed: result.status === 'confirmed' }
    }
    const message = result.status === 'skipped' ? "This purchase couldn't run. No MANA was spent." : result.failure.message
    // Shown as pending at its dispatch, then the relay's late answer proved nothing went (a marked refusal): it ends
    // unbought, on the dialog still up for it, or quietly when that was closed (its item is free again).
    if (op.state === 'pending' && !slot.holds(op)) {
      settled(op, 'failed', {}, message, message)
      return { kind: 'settled' }
    }
    return refused(message, result.status === 'skipped' ? 'cancelled' : 'failed')
  }

  /**
   * Writes `op`'s MANA attempt and reads it back: true only when storage holds exactly the record given. A checkpoint a
   * send depends on (`latch`, the default) also refuses once any write for the operation failed this page's lifetime,
   * and then writes nothing: storage never says a send went that this page refused. The watch's tidying of a record
   * (dropping what can't be evidence) needs only this write to stick.
   */
  const keeper =
    (op: Operation, opts: { latch?: boolean } = {}) =>
    (record: ManaAttemptRecord): boolean => {
      const latched = (): boolean => opts.latch !== false && ledger.failed(op.id)
      if (latched()) return false
      const before = op.mana
      op.mana = record
      const kept = keep(op) && !latched() && JSON.stringify(ledger.saved(op.buyer ?? '', op.id)?.mana) === JSON.stringify(record)
      if (!kept) {
        op.mana = before
        // Refused after it was written (a write failed meanwhile): what was kept before goes back.
        if (JSON.stringify(ledger.saved(op.buyer ?? '', op.id)?.mana) === JSON.stringify(record)) keep(op)
      }
      return kept
    }

  /**
   * RETRY: the signed execution of the sale, sent again through the relay, inside the same operation and its lock. No
   * new signature and no wallet: only what the buyer signed goes, at the nonce it names, so it can complete only once.
   * It goes only for the buyer signed in here (at the click, and again right before it is handed off), only once the
   * wait since the latest request passed (the attempt holds it to that too), and only while the chain still reads that
   * nonce: otherwise nothing is sent and the chain decides. A refused or lost retry ends only itself: the purchase is
   * still followed to its end.
   */
  const retryMana = async (op: Operation): Promise<void> => {
    const record = op.mana
    if (!record || !op.buyer || !op.chainId || !record.nonce || !record.data || !record.signed) return
    // A RETRY pressed once it no longer holds (the buyer signed out, say): what it shows follows what it offers now.
    if (manaPendingKind(op) !== 'retry') {
      if (slot.holds(op) && !busy() && !TERMINAL_STATES.includes(op.state)) showPending(op)
      return
    }
    const chainId = op.chainId
    const buyer = op.buyer
    // `retry` holds only for its buyer signed in here.
    const who = signedIn()
    if (!who) return
    const login = loginKey(who.login)
    /** The same login, still the buyer's, signed in here. */
    const sameBuyer = (): boolean => {
      const now = signedIn()
      return !!now && now.buyer === buyer && loginKey(now.login) === login
    }
    const signed = record.signed
    const terms = storedManaTerms(record)
    const nonce = BigInt(record.nonce)
    /** The shop-wide hold, from RETRY until the retry is kept as gone (or ends). */
    let release = (): void => {}
    let attempt: ReturnType<typeof createManaAttempt>
    let venue
    try {
      const book = registryFor({ chainId, sale: terms })
      venue = book.venue
      attempt = createManaAttempt(terms, keeper(op), {
        mana: book.mana.address,
        checkpoint: () => svc.manaEvidence().finalized(chainId),
        now: svc.now,
        sendMs: svc.manaTimeouts().sendMs,
        retryAfterMs: RETRY_AFTER_MS,
        from: record,
        load: () => op.mana,
        // Kept as gone: in progress from here, closable, and followed by the watch, however long the relay takes.
        onDispatch: () => {
          op.manaRetry = undefined
          if (slot.holds(op) && !TERMINAL_STATES.includes(op.state)) showPending(op)
          release()
          void reconcile(op)
        }
      })
    } catch (e) {
      console.warn('[shell/shop] could not retry the MANA purchase', e)
      return
    }
    const metaTx = venue.metaTx
    if (!metaTx) return
    const rail = svc.rail()
    release = host.hold()
    op.manaSending = true
    show(op, { step: 'processing', balance: null, count: op.items.reduce((n, l) => n + l.quantity, 0) }, { onClose: () => {} })
    try {
      // Only while the chain still reads the sale's nonce: past it, the signed execution can never run.
      const timeouts = svc.manaTimeouts()
      const read = await readMetaTxNonce(boundedReader(svc.reader(), timeouts.readMs), { address: venue.address as `0x${string}`, chainId: chainId as Parameters<ChainReader['receipt']>[0], metaTx }, buyer)
      // The sign-in may have changed while the nonce was read: then nothing goes.
      if (read === nonce && sameBuyer()) {
        attempt.retry()
        if (attempt.handOff({ rail: 'relay', nonce, data: record.data })) {
          const send = attempt.relay(rail.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a)))
          attempt.answered(await relaySignedMetaTransaction({ from: buyer, to: venue.address, data: signed, relayerUrl: rail.relayerUrl, fetchImpl: send }))
        }
      }
    } catch (e) {
      // A nonce that couldn't be read, a refusal, a lost answer: each ends this send only, as kept; the chain decides.
      console.warn('[shell/shop] the MANA retry ended', e)
    } finally {
      op.manaSending = false
      release()
      // A retry that ended before its request (its dispatch couldn't be kept) left nothing that can be evidence: dropped,
      // so the purchase may be retried again.
      attempt.retire()
    }
    op.manaRetry = undefined
    // Shown as still in progress at once, closable; the chain is read in the background, by the watch and one look now.
    // Signed out meanwhile: what it shows asks to sign in as the buyer, once a look allows RETRY again.
    if (slot.holds(op) && !TERMINAL_STATES.includes(op.state)) showPending(op)
    void reconcile(op)
    void check(op as Operation & { buyer: string }, (op.store ??= ledger.storeFor(op))).catch(() => false)
  }

  /**
   * The executor's request for MANA's allowance (requestApproval): Authorize MANA. AUTHORIZE grants it from the buyer's
   * click, through the guarded wallet and the purchase's relay transport; true once it is set, and the dialog back on
   * `working` (Processing). CANCEL (or Escape) closes the purchase: false at once, and nothing more goes for it, so an
   * approval confirmed later never starts the sale. In a cart, it says which payment it is for (`progress`).
   */
  const authorize = (
    op: Operation,
    request: ApprovalRequest,
    ctx: RailContext,
    terms: ManaSaleTerms,
    attempt: ReturnType<typeof createManaAttempt>,
    working: ShellPurchaseView,
    progress?: ShopPaymentProgress
  ): Promise<boolean> =>
    new Promise((resolve) => {
      let answered = false
      let state: 'ready' | 'waiting' | 'failed' = 'ready'
      let message: string | undefined
      const answer = (granted: boolean): void => {
        if (answered) return
        answered = true
        resolve(granted)
      }
      const draw = (): void => {
        if (answered) return
        // The dialog went to something else: no one can answer this step, so the purchase ends here, unsent.
        if (!slot.holds(op)) {
          attempt.close()
          answer(false)
          return
        }
        show(
          op,
          {
            step: 'authorize',
            venue: terms.kind === 'listings' ? 'the Marketplace' : 'the Collection Store',
            network: chainLabel(request.auth.chainId),
            required: request.requiredWei,
            state,
            ...(message ? { message } : {}),
            ...(progress ? { progress } : {})
          },
          {
            // Only while this step is the question: once answered (the allowance set and the sale on its way), a late
            // CANCEL from this view does nothing.
            onClose: () => {
              if (answered) return
              attempt.close()
              answer(false)
            },
            onAuthorize: () => void grant()
          }
        )
      }
      /** AUTHORIZE: the approve, inside the buyer's click and only for its lifetime (the guard's `mayApprove`). */
      const grant = async (): Promise<void> => {
        if (state === 'waiting' || answered) return
        state = 'waiting'
        message = undefined
        draw()
        try {
          await attempt.authorize(() =>
            setAuthorization({ auth: request.auth, active: true, provider: ctx.provider, account: ctx.buyer, walletKind: ctx.walletKind, reader: ctx.reader, relayerUrl: ctx.relayerUrl, fetchImpl: ctx.fetchImpl })
          )
        } catch (e) {
          if (answered) return
          state = 'failed'
          message = isRejection(e) ? 'You declined the authorization in your wallet.' : friendlyError(e, "The authorization didn't go through. Try again.")
          draw()
          return
        }
        if (answered) return
        if (!attempt.open()) {
          answer(false)
          return
        }
        // Set: the dialog moves on to the purchase itself before the sale is asked for.
        show(op, working, { onClose: () => {} })
        answer(true)
      }
      draw()
    })

  return { showPending, checkMana, readManaInTime, quoteMana, settleMana }
}
