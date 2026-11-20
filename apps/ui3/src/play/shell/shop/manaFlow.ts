// Paying for an in-world Shop purchase in Polygon MANA (./purchase holds the rest of the purchase): the MANA quote
// and balance, BUY with MANA through the web Shop's executor with the wallet bound to the sale (./manaGuard), the
// attempt kept at each checkpoint (./manaPurchase), Authorize MANA, the chain's word on a sale that went
// (./manaEvidence), and RETRY of a sale whose answer was lost. It works on ./purchase's operations, through the host
// that purchase gives it: the dialog, the operation's record and state, the watch, and the shop-wide hold.

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
import { SHOP_PROTOCOL, TERMINAL_STATES, type ShopBuyRequest, type ShopOperationState } from '@dcl/one-ui/apps/shop/shopProtocol'
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
import { boundedReader, deadline, resolveMana, verifiedVenue } from './manaEvidence'
import { readMetaTxNonce, relaySignedMetaTransaction } from '../../../shop/checkout/chain/metatx'
import { STORAGE_REFUSED, type Ledger } from './ledger'
import { assertWalletAccount, WalletRefusal } from './walletGuard'
import type { DialogSlot, ManaQuote, Operation, Quote, QuotePart, ShellPurchaseHandlers, ShellPurchaseView, ShellShopServices, ShellShopWallet } from './purchase'

/** A MANA sale the chain says can no longer run: it never did. */
const MANA_NOT_BOUGHT = "This purchase didn't go through. Nothing was bought and no MANA was spent."
const MANA_NOT_BOUGHT_PART = 'one of its transactions didn’t go through; no MANA was spent for it.'
/** A MANA sale the relay answered, late, that it didn't send. */
const MANA_NOT_SENT = 'This purchase couldn’t go out. Nothing was bought and no MANA was spent.'
const MANA_NOT_SENT_PART = 'one of its transactions couldn’t go out; no MANA was spent for it.'
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
  ): Promise<(Omit<ManaQuote, 'part'> & { group: CartGroup; priced: { line: CartLine; offer: PurchaseOffer }[] }) | null>
  settleMana(op: Operation, req: ShopBuyRequest, quote: Quote): Promise<void>
}

export function createManaFlow(host: ManaFlowHost): ManaFlow {
  const { svc, ledger, slot, wallet: injected, signedIn, loginKey, show, hide, fail, close, move, publish, keep, settled, restart, reconcile, check, checkAgain, reviewPart, stillPending, busy } = host

  /**
   * Purchase in progress. One paid in MANA is being checked on chain (CHECK AGAIN looks now). Once none of its sends is
   * on its way and the latest went RETRY_AFTER_MS ago with nothing on chain yet, this page (holding it) offers RETRY:
   * the signed execution, sent again, so it can complete only once.
   */
  const showPending = (op: Operation): void => {
    const kind = manaPendingKind(op)
    op.manaShown = kind
    if (kind === 'retry') {
      show(op, { step: 'pending', message: MANA_RETRY, retry: true }, { onClose: () => hide(op), onRetry: () => void retryMana(op) })
      return
    }
    const message = kind === 'capped' ? MANA_NO_MORE_RETRIES : kind === 'signin' ? MANA_SIGN_IN_TO_RETRY : MANA_CHECKING
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
    if (!op.manaSending && !manaDispatched(attempt) && attempt.sends.some((s) => s.stage === 'refused') && !attempt.sends.some((s) => s.stage === 'signing')) {
      if (slot.holds(op)) {
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
      settled(op, 'cancelled')
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
   * Paying one item in MANA, or null where it can't be offered: no MANA price for it, a sale the chain and the server
   * can't settle now, or one the MANA guard can't bound (a USD-priced listing, a venue the registry doesn't list, a
   * chain phase 1 doesn't buy on). The sale is built here, from the lines as they read now, and pinned: paying it
   * later takes exactly this sale, or asks for a new confirmation. A balance short of the price still offers it.
   */
  const quoteMana = async (
    cartLines: CartLine[],
    offers: Record<string, PurchaseOffer | null | undefined>,
    session: Session,
    buyer: string,
    client: ShopClient,
    creditsBalance: UsdCents | null,
    manaBalance: bigint
  ): Promise<(Omit<ManaQuote, 'part'> & { group: CartGroup; priced: { line: CartLine; offer: PurchaseOffer }[] }) | null> => {
    // Only through the relay (a sale that can be retried at its nonce): none with the relay off.
    if (!(svc.rail().gasless ?? gaslessEnabled())) return null
    const priced = cartLines.map((line) => ({ line: { ...line, method: 'mana' as const }, offer: offers[line.id]! }))
    const partition = partitionCart(priced, session, { creditsBalance, manaBalance: manaBalance as ManaWei })
    const group = partition.groups[0]
    if (partition.refused.length > 0 || partition.groups.length !== 1 || group.key.method !== 'mana' || group.total.method !== 'mana') return null
    const price: bigint = group.total.amount
    if (price <= 0n) return null
    let terms: ManaSaleTerms
    try {
      const review = await reviewCart(priced, session, buyer, svc.resolvers(client))
      const resolved = new Map(review.buyable.map((r) => [r.line.id, r]))
      const lines = group.lineIds.map((id) => resolved.get(id))
      if (lines.some((l) => l == null)) return null
      terms = manaSaleTerms(group, lines as ResolvedLine[], buyer)
      checkManaTerms({ buyer, chainId: group.key.chainId, sale: terms })
    } catch (e) {
      console.warn('[shell/shop] MANA is not offered for this item', e)
      return null
    }
    // Only at a venue whose execution evidence is verified: anywhere else a lost answer could never end.
    if (!verifiedVenue(group.key.chainId, terms.venue)) return null
    return { group, priced, terms, fingerprint: manaFingerprint(terms), price, balance: manaBalance, pays: manaBalance >= price }
  }

  /**
   * BUY with MANA picked: the item's one group, paid from the buyer's own MANA through the web Shop's executor
   * (runManaGroup), with the wallet bound to the sale the buyer was shown (./manaGuard) and the attempt kept at each
   * checkpoint (./manaPurchase). A first MANA purchase at the venue asks for its allowance first (Authorize MANA).
   * Whatever may have gone out is followed to its end and never sent again; what provably didn't may be tried again.
   */
  const settleMana = async (op: Operation, req: ShopBuyRequest, quote: Quote): Promise<void> => {
    op.unwatchWallet?.()
    const mana = quote.mana
    if (!mana) return
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
    // The shop-wide hold ends once the sale is kept as gone (or this ends): the operation's own lock keeps the purchase.
    const release = host.hold()
    try {
      await runMana(op, req, quote, mana, wallet, current, release)
    } finally {
      release()
      // Followed once the dialog says so: a watch that settles at once then updates it.
      if ((op.state as ShopOperationState) === 'pending') void reconcile(op)
    }
  }

  const runMana = async (op: Operation, req: ShopBuyRequest, quote: Quote, mana: ManaQuote, wallet: Eip1193Provider, current: () => boolean, release: () => void): Promise<void> => {
    const { part } = mana
    const { group } = part
    const chainId = group.key.chainId
    const again = (): void => restart(op, req, 'cancelled')
    /** Nothing of it went out: back to reviewing, so TRY AGAIN (a fresh read of the terms) may follow. */
    const refused = (message: string, retry?: () => void): void => {
      op.state = 'reviewing'
      publish(op)
      fail(op, message, retry)
    }
    op.items = part.items
    op.displays = part.displays
    op.chainId = chainId
    move(op, 'processing')
    if (ledger.failed(op.id) || ledger.saved(op.buyer ?? '', op.id)?.state !== 'processing') {
      refused(STORAGE_REFUSED, again)
      return
    }
    show(op, { step: 'processing', balance: null, count: quote.items.length }, { onClose: () => {} })
    // The sale as the chain and the server read it now: the one the buyer was shown, or a new confirmation.
    let review: Awaited<ReturnType<typeof reviewPart>>
    try {
      review = await reviewPart(part, quote)
    } catch (e) {
      console.warn('[shell/shop] could not review the MANA purchase', e)
      refused("We couldn't start this purchase. No MANA was spent.", again)
      return
    }
    if ('why' in review) {
      refused(review.why, review.retry === 'cancelled' ? again : undefined)
      return
    }
    const { resolved } = review
    const lines = group.lineIds.map((id) => resolved.get(id))
    let terms: ManaSaleTerms
    try {
      if (lines.some((l) => l == null)) throw new Error('A line of the sale could not be resolved')
      terms = manaSaleTerms(group, lines as ResolvedLine[], quote.buyer)
    } catch (e) {
      console.warn('[shell/shop] could not build the MANA sale', e)
      refused("This item can't be bought with MANA right now. No MANA was spent.", again)
      return
    }
    if (manaFingerprint(terms) !== mana.fingerprint) {
      refused('Its price or terms changed. Try again to see the new ones. No MANA was spent.', again)
      return
    }
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
      // Kept as gone: pending from here, closable, and followed by the watch, however long the relay and the executor take.
      onDispatch: () => {
        if (op.state === 'processing') move(op, 'pending')
        if (slot.holds(op) && !TERMINAL_STATES.includes(op.state)) showPending(op)
        release()
        void reconcile(op)
      }
    })
    if (!keepAttempt(structuredClone(attempt.record))) {
      refused(STORAGE_REFUSED, again)
      return
    }
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
      refused(e instanceof WalletRefusal ? `${e.message} No MANA was spent.` : "This item can't be bought with MANA right now. No MANA was spent.", again)
      return
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
        result = await svc.run({ ctx, store, guard: createSpendGuard(), requestApproval: (request) => authorize(op, request, ctx, terms, attempt) }, group, resolved)
      } finally {
        op.manaSending = false
      }
      unmirror()
    } catch (e) {
      unmirror()
      console.warn('[shell/shop] the MANA purchase stopped unexpectedly', e)
      if (attempt.dispatched()) {
        move(op, 'pending')
        showPending(op)
      } else refused("We couldn't start this purchase. No MANA was spent.", again)
      return
    }
    // Cancelled while it asked for the allowance: nothing of the sale went, and nothing more goes. Any evidence the sale
    // went (a dispatch, a hash, a confirmation) is followed below instead, whatever the dialog was asked.
    if (!attempt.open() && result.status === 'failed' && result.failure.kind !== 'reverted' && !attempt.dispatched()) {
      close(op, 'cancelled')
      return
    }
    // A sale that went (or that the executor says went): whatever it read (a receipt for the relay's hash, a revert, no
    // answer), only the chain's evidence for this sale's own signed execution settles it (./manaEvidence). A reverted
    // transaction doesn't retire the sale's nonce: the sale may still run, so it is followed like any other.
    // Settled meanwhile by the watch (the chain answered first): nothing left to do here.
    if (TERMINAL_STATES.includes(op.state)) return
    // A sale that went: pending, as its dispatch already showed it. The watch reads the chain; one look now too, with
    // whatever the relay answered since (its hash), rather than at the watch's next turn.
    if (attempt.dispatched() || result.status === 'confirmed' || result.status === 'pending') {
      if (op.state !== 'pending') {
        move(op, 'pending')
        if (slot.holds(op)) showPending(op)
      }
      void check(op as Operation & { buyer: string }, store).catch(() => false)
      return
    }
    const message = result.status === 'skipped' ? "This purchase couldn't run. No MANA was spent." : result.failure.message
    // Shown as pending at its dispatch, then the relay's late answer proved nothing went (a marked refusal): it ends
    // unbought, on the dialog still up for it, or quietly when that was closed (its item is free again).
    if (op.state === 'pending' && !slot.holds(op)) {
      settled(op, 'failed', {}, message, message)
      return
    }
    op.state = 'reviewing'
    publish(op)
    fail(op, message, result.status === 'skipped' ? again : () => restart(op, req, 'failed'))
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
   * click, through the guarded wallet and the purchase's relay transport; true once it is set. CANCEL (or Escape) closes
   * the purchase: false at once, and nothing more goes for it, so an approval confirmed later never starts the sale.
   */
  const authorize = (op: Operation, request: ApprovalRequest, ctx: RailContext, terms: ManaSaleTerms, attempt: ReturnType<typeof createManaAttempt>): Promise<boolean> =>
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
            ...(message ? { message } : {})
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
        show(op, { step: 'processing', balance: null, count: op.items.reduce((n, l) => n + l.quantity, 0) }, { onClose: () => {} })
        answer(true)
      }
      draw()
    })

  return { showPending, checkMana, readManaInTime, quoteMana, settleMana }
}
