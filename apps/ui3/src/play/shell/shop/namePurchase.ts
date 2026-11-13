// The web shell's side of BUY NAME (lib/shopProtocol shop.buyName). The HUD names the NAME; the shell reads
// everything that decides the purchase itself (the NAME's price in Credits, the buyer's balance, whether
// the NAME is free), shows the design's Buy Name dialog in the one dialog the item purchases use too
// (./purchase's DialogSlot), and on BUY NAME runs it. Its operation moves as the item purchases' do
// (`bevy-shell:shop` events, shop.status), as the NAME variant: the NAME, no items, and its mode.
//
// Only the simulation is built (`namesMode: 'simulation'`). It rehearses the flow on a testnet build with
// the real reads, and buys nothing: it never authorizes a credit, relays, asks the wallet for an allowance
// or a signature, or writes the payments ledger, so no Credits are spent and the NAME is neither
// registered nor reserved. Every dialog it shows says so, and its success offers a preview of the NAME
// over the avatar in the HUD, never a profile save. It runs only on a build that turns it on explicitly
// (VITE_SHOP_NAME_SIMULATION) for Amoy (VITE_SHOP_CHAIN_ID=80002), while the chains the page settles on
// are still Amoy and Sepolia (namesModeFor). Registering a NAME for real (`live`) needs a guarded rail of
// its own, and durable operations, and isn't here.

import type { ShopClient } from '@shop/api/shop'
import type { UsdBalance } from '@shop/credits/credits'
import { CHAIN } from '@shop/model/chain'
import { creditsCeil, type UsdCents } from '@shop/model/money'
import {
  parseBuyNameRequest,
  shopBusy,
  SHOP_EVENT,
  SHOP_PROTOCOL,
  TERMINAL_STATES,
  type ShopBuyNameRequest,
  type ShopNameOperation,
  type ShopNamesMode,
  type ShopOperationEvent,
  type ShopOperationState
} from '../../lib/shopProtocol'
import type { DialogSlot, ShellPurchaseHandlers, ShellPurchaseItem, ShellPurchaseView } from './purchase'

/** How long the simulation's Processing shows, as a purchase's would. */
export const NAME_SIMULATION_MS = 1500

/** What the build was built with: the simulation's own switch, and the chain it names (VITE_SHOP_CHAIN_ID). */
export interface NameBuild {
  simulation: boolean
  chainId: number | null
}

/** The chains this page settles on now (shop/checkout/chain/config), which a runtime override may change. */
export interface NameChains {
  settles: number
  ethereum: number
}

/**
 * How this build buys NAMEs, or null where it doesn't. The simulation needs the build to turn it on and to
 * name Amoy, both fixed when it is built, and the chains the page settles on now to be Amoy and Sepolia:
 * a runtime override of those can only turn it off.
 */
export function namesModeFor(build: NameBuild, chains: NameChains): ShopNamesMode | null {
  if (!build.simulation || build.chainId !== CHAIN.AMOY) return null
  return chains.settles === CHAIN.AMOY && chains.ethereum === CHAIN.SEPOLIA ? 'simulation' : null
}

/** This build's NameBuild: VITE_SHOP_NAME_SIMULATION set to `true`, and VITE_SHOP_CHAIN_ID as given (never a default). */
export function nameBuild(): NameBuild {
  const env = typeof import.meta !== 'undefined' ? import.meta.env : undefined
  const raw = (env?.VITE_SHOP_CHAIN_ID as string | undefined)?.trim()
  const chainId = raw && /^[1-9][0-9]*$/.test(raw) ? Number(raw) : null
  return { simulation: (env?.VITE_SHOP_NAME_SIMULATION as string | undefined)?.trim() === 'true', chainId: Number.isSafeInteger(chainId) ? chainId : null }
}

/** Whether a NAME can be registered: free, taken, or on the banned list. */
export type NameStanding = 'available' | 'taken' | 'banned'

/** What BUY NAME reads and waits on, injectable for tests. */
export interface NamePurchaseServices {
  /** The buyer's Credits balance; its whole `credits` are what the dialog shows and compares. */
  balance(account: string, client: ShopClient): Promise<UsdBalance>
  /** A NAME's price, the credits quote of its 100 MANA, in US cents; null while none can be given. */
  price(client: ShopClient): Promise<UsdCents | null>
  standing(name: string): Promise<NameStanding>
  sleep(ms: number): Promise<void>
  now(): number
  newId(): string
  /** The web Shop's credit packs, for BUY CREDITS. */
  creditsUrl(): string
}

/** The login that may buy NAMEs now: its account (lowercase), the login itself as signed, and the mode. */
export interface NameSession {
  buyer: string
  login: string
  mode: ShopNamesMode
}

export interface NamePurchaseContext {
  /** The same answer as capabilities' `namesMode`, with the login it is for; null where NAMEs aren't bought. */
  session(): NameSession | null
  /** Requests signed by that one login (`NameSession.login`), and by no later one. */
  client(login: string): ShopClient
  /** The one dialog over the app, shared with the item purchases. */
  slot: DialogSlot
  /** An item purchase is being opened or is running. */
  busy(): boolean
  post(event: ShopOperationEvent): void
  openExternal(url: string): void
  services: NamePurchaseServices
}

interface NameOperation {
  kind: 'name'
  id: string
  buyer: string
  /** The login it was opened under: only that one reads its terms, confirms it, or is shown how it ended. */
  login: string
  name: string
  mode: ShopNamesMode
  state: ShopOperationState
  next?: 'previewName'
  createdAt: number
  input?: ShopBuyNameRequest['input']
  /** BUY NAME was pressed: acted on once. */
  confirmed?: true
}

export interface NamePurchase {
  buy(params: unknown): Promise<ShopNameOperation>
  /** The buyer's NAME operations, as shop.status lists them. */
  list(buyer: string): Array<ShopNameOperation & { createdAt: number }>
  /** One of the buyer's NAME operations. */
  get(id: string, buyer: string): ShopNameOperation | null
}

const TAKEN = 'Someone else got this NAME first. Try another one.'
const BANNED = 'This NAME is not available. Try another one.'
const UNREADABLE = 'We couldn’t check this NAME right now.'
const NO_PRICE = 'We couldn’t get the price of a NAME right now.'
const NO_BALANCE = 'We couldn’t read your Credits balance right now.'
const SIGN_IN_CHANGED = 'Your sign-in changed meanwhile. Open the NAME again.'
const NOT_RUN = "This NAME wasn't bought. Nothing was charged."

export function createNamePurchase(ctx: NamePurchaseContext): NamePurchase {
  const svc = ctx.services
  const ops = new Map<string, NameOperation>()

  const snapshot = (op: NameOperation): ShopNameOperation => ({
    v: SHOP_PROTOCOL,
    operation: op.id,
    buyer: op.buyer,
    state: op.state,
    items: [],
    name: op.name,
    mode: op.mode,
    ...(op.next ? { next: op.next } : {})
  })
  const publish = (op: NameOperation): void => ctx.post({ type: SHOP_EVENT, ...snapshot(op) })
  /** A state the operation moves to; a settled one is final. */
  const move = (op: NameOperation, state: ShopOperationState): void => {
    if (TERMINAL_STATES.includes(op.state)) return
    op.state = state
    publish(op)
  }
  const show = (op: NameOperation, view: ShellPurchaseView, handlers: ShellPurchaseHandlers): void => ctx.slot.show(op, view, handlers, { pointer: op.input === 'pointer' })
  const hide = (op: NameOperation): void => ctx.slot.hide(op)
  const close = (op: NameOperation, state: ShopOperationState): void => {
    hide(op)
    move(op, state)
  }
  /** Still the login, account and mode it was opened under. */
  const current = (op: NameOperation): boolean => {
    const now = ctx.session()
    return now != null && now.buyer === op.buyer && now.login === op.login && now.mode === op.mode
  }
  /**
   * Its dialog is still up for it and it is still being reviewed. One that lost its dialog while reviewed
   * (cancelled, or the dialog taken by another purchase) ends cancelled, never left open in shop.status.
   */
  const reviewing = (op: NameOperation): boolean => {
    if (ctx.slot.holds(op) && op.state === 'reviewing') return true
    if (op.state === 'reviewing') move(op, 'cancelled')
    return false
  }
  const simulated = (op: NameOperation): { simulation?: true } => (op.mode === 'simulation' ? { simulation: true } : {})
  /** Why it ended unbought; TRY AGAIN reads the terms again when it may help. */
  const fail = (op: NameOperation, message: string, retry?: () => void): void => {
    show(op, { step: 'failed', message, retry: retry != null, ...simulated(op) }, { onClose: () => close(op, 'failed'), ...(retry ? { onRetry: retry } : {}) })
  }

  /** Reads the terms and shows them: Buy Name, or Insufficient Funds when the balance is short. */
  const review = async (op: NameOperation): Promise<void> => {
    show(op, { step: 'loading', subject: 'name', ...simulated(op) }, { onClose: () => close(op, 'cancelled') })
    const client = ctx.client(op.login)
    const [standing, price, balance] = await Promise.allSettled([svc.standing(op.name), svc.price(client), svc.balance(op.buyer, client)])
    // Cancelled, or its dialog taken, while it read: a late answer reopens nothing.
    if (!reviewing(op)) return
    if (!current(op)) return fail(op, SIGN_IN_CHANGED)
    const again = (): void => {
      if (reviewing(op)) void review(op)
    }
    if (standing.status === 'rejected') return fail(op, UNREADABLE, again)
    if (standing.value === 'taken') return fail(op, TAKEN)
    if (standing.value === 'banned') return fail(op, BANNED)
    // The quote is in US cents; the dialog and the balance are in whole Credits.
    const cents = price.status === 'fulfilled' ? price.value : null
    const credits = cents != null && Number.isFinite(cents) && cents > 0 ? creditsCeil(cents) : null
    if (credits == null || !Number.isSafeInteger(credits)) return fail(op, NO_PRICE, again)
    const have = balance.status === 'fulfilled' ? balance.value.credits : null
    if (have == null || !Number.isSafeInteger(have) || have < 0) return fail(op, NO_BALANCE, again)
    const item: ShellPurchaseItem = { kind: 'name', name: op.name, creator: '', rarity: '', credits }
    if (credits > have) {
      show(op, { step: 'insufficient', items: [item], balance: have, need: credits - have, ...simulated(op) }, {
        onClose: () => close(op, 'cancelled'),
        onBuyCredits: () => {
          ctx.openExternal(svc.creditsUrl())
          close(op, 'cancelled')
        }
      })
      return
    }
    show(op, { step: 'confirm', items: [item], balance: have, ...simulated(op) }, {
      onClose: () => close(op, 'cancelled'),
      onConfirm: () => void confirm(op, item, have)
    })
  }

  /** BUY NAME: taken once, for the login that was shown the terms, before anything is awaited. */
  const confirm = async (op: NameOperation, item: ShellPurchaseItem, balance: number): Promise<void> => {
    if (op.confirmed || !reviewing(op)) return
    op.confirmed = true
    if (!current(op)) return fail(op, SIGN_IN_CHANGED)
    // Only the simulation is built (see the header): nothing else may run here.
    if (op.mode !== 'simulation') return fail(op, NOT_RUN)
    move(op, 'processing')
    show(op, { step: 'processing', balance, subject: 'name', simulation: true }, { onClose: () => {} })
    try {
      await svc.sleep(NAME_SIMULATION_MS)
    } catch {
      // The page is going: its Processing doesn't stay up with no way out.
      move(op, 'failed')
      hide(op)
      return
    }
    // Signed out, into another account, or the sign-in renewed meanwhile: its end isn't that session's.
    if (!current(op)) {
      move(op, 'failed')
      if (ctx.slot.holds(op)) show(op, { step: 'failed', message: SIGN_IN_CHANGED, retry: false, simulation: true }, { onClose: () => hide(op) })
      return
    }
    move(op, 'succeeded')
    if (!ctx.slot.holds(op)) return
    show(op, { step: 'success', items: [item], simulation: true }, {
      onClose: () => hide(op),
      onPreview: () => {
        hide(op)
        // Once per operation, however often it is pressed, and only for the session that rehearsed it.
        if (op.next || !current(op)) return
        op.next = 'previewName'
        publish(op)
      }
    })
  }

  return {
    async buy(params) {
      const req = parseBuyNameRequest(params)
      const session = ctx.session()
      // Refused where capabilities says nothing of NAMEs, or something else than the HUD was told.
      if (!session) throw new Error('NAMEs are not bought here')
      if (req.mode !== session.mode) throw new Error(`NAMEs are bought here in ${session.mode} mode`)
      if (ctx.busy() || ctx.slot.open()) throw shopBusy()
      const op: NameOperation = { kind: 'name', id: svc.newId(), buyer: session.buyer, login: session.login, name: req.name, mode: session.mode, state: 'reviewing', createdAt: svc.now(), input: req.input }
      ops.set(op.id, op)
      publish(op)
      // Its loading step shows before this returns.
      void review(op)
      return snapshot(op)
    },
    list: (buyer) => [...ops.values()].filter((o) => o.buyer === buyer).map((o) => ({ ...snapshot(o), createdAt: o.createdAt })),
    get: (id, buyer) => {
      const op = ops.get(id)
      return op && op.buyer === buyer ? snapshot(op) : null
    }
  }
}
