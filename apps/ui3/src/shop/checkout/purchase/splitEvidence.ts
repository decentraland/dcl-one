// What the chain says about ONE purchase paid with Credits and MANA together (a split), the web Shop's or /play's. Its terms are kept
// before its sale is signed (the buyer, the chain, the CreditsManager, the credit the node issued, the credit's exact
// value C, the MANA gap G and the external call as reviewed), and an execution counts as this purchase only when it
// carries exactly those terms: the buyer's meta-transaction to that manager, spending that credit, crediting C, taking G
// from the wallet, for that call. Exact sizing proves the amount, not which item a credit bought, so neither a successful
// receipt for a hash the relay answered nor the credit's SETTLED record is enough on its own: the transaction itself is
// read and matched. A revert is narrower still: it proves only that THAT invocation failed, so it counts as this purchase's
// only for the exact bytes this surface dispatched (`envelope`); the same terms signed again, or sent with another
// external-call salt, are another invocation, which a revert of one says nothing about.

import { decodeFunctionData, keccak256, type Hex } from 'viem'
import { CREDITS_MANAGER_ABI, OFFCHAIN_META_TX_ABI } from '@shop/checkout/chain/abi'
import { defaultSleep, type TxReceipt } from '../chain/rpc'
import { idToSalt } from './encoding'

/**
 * The two chain reads attribution needs: a transaction's target and input (and its sender, when the reader has it), and
 * its receipt (null when unknown).
 */
export interface SplitEvidenceReader {
  transaction(chainId: number, hash: string): Promise<SplitTransaction | null>
  receipt(chainId: number, hash: string): Promise<TxReceipt | null>
}

/**
 * A transaction as attribution reads it. `from` is only needed for a useCredits the buyer's wallet sent itself (the web
 * Shop's fallback when the relay proved it sent nothing); a relayed one names its buyer inside the call.
 */
export type SplitTransaction = { to: string | null; input: Hex; from?: string | null }

/** A split's sale as kept before it was signed: what an execution must carry to count as this purchase. */
export interface SplitSale {
  buyer: string
  chainId: number
  manager: string
  /** The credit the node issued for it (its id, whose salt the call spends). */
  creditId: string
  /** The credit's exact value C and the MANA gap G, in wei. */
  creditWei: string
  gapWei: string
  /** The external call as reviewed: its venue (`target`), its function (`selector`) and its data (price, coupon, buyer). */
  target: string
  selector: Hex
  callData: Hex
  /**
   * The identity of what was dispatched (`splitEnvelope` of its exact bytes), kept before they left: the relayed execution
   * as signed (the `data` the relay sends to the manager, `transactionData.params[1]`), or the useCredits call the
   * buyer's wallet was handed. Only a revert of these very bytes is this sale's; none kept, no revert is.
   */
  envelope?: Hex
  /** Its line in the hold the web Shop and /play share (shop/checkout/purchase/splitHold), and the attempt it wrote there. */
  hold?: { line: string; attempt: string }
}

/**
 * - `bought`: a mined, successful execution of exactly this purchase;
 * - `reverted`: the very bytes this purchase dispatched (`SplitSale.envelope`), mined and reverted: that invocation failed
 *   (the same execution may still run in another transaction: what the node's record of the credit says comes first).
 *   Only the in-world Shop acts on it (an own revert there waits on the node's record by name); the web Shop's
 *   `settleSplitHold` holds it as it holds `unknown`. The envelope is kept on both surfaces so that both can tell it;
 * - `other`: the buyer's useCredits spending this very credit, alone or in a batch, on other terms (another venue,
 *   function, call, share or gap): the credit went into another purchase;
 * - `unknown`: nothing that says either way: no transaction or receipt yet, a read that failed, a transaction whose
 *   wrapper or target this page can't read (its destination alone says nothing about where the credit went), a useCredits
 *   by another account or of other credits (it doesn't spend this one), or a revert of these terms in bytes other than
 *   the ones dispatched (or with none kept to compare).
 */
export type SplitOutcome = 'bought' | 'reverted' | 'other' | 'unknown'

const lower = (v: unknown): string => (typeof v === 'string' ? v.toLowerCase() : '')

/** The identity of dispatched bytes (`SplitSale.envelope`): their keccak256, the same on every surface. */
export function splitEnvelope(dispatched: Hex): Hex {
  return keccak256(dispatched)
}

/** Whether a structurally valid shape for a split sale (a ledger record read back, say). */
export function isSplitSale(v: unknown): v is SplitSale {
  if (!v || typeof v !== 'object') return false
  const s = v as Record<string, unknown>
  const hex = (x: unknown): boolean => typeof x === 'string' && /^0x[0-9a-fA-F]*$/.test(x)
  const digits = (x: unknown): boolean => typeof x === 'string' && /^\d+$/.test(x)
  return (
    hex(s.buyer) &&
    typeof s.chainId === 'number' &&
    Number.isSafeInteger(s.chainId) &&
    hex(s.manager) &&
    typeof s.creditId === 'string' &&
    s.creditId.length > 0 &&
    digits(s.creditWei) &&
    digits(s.gapWei) &&
    hex(s.target) &&
    hex(s.selector) &&
    hex(s.callData) &&
    (s.envelope === undefined || (typeof s.envelope === 'string' && /^0x[0-9a-fA-F]{64}$/.test(s.envelope))) &&
    (s.hold === undefined ||
      (typeof s.hold === 'object' && s.hold !== null && typeof (s.hold as Record<string, unknown>).line === 'string' && typeof (s.hold as Record<string, unknown>).attempt === 'string'))
  )
}

/**
 * What `tx` says of the sale: `match` (the buyer's useCredits at the sale's manager carrying exactly its terms, relayed or
 * sent by the buyer's own wallet), `other` (the buyer's useCredits spending this very credit, alone or in a batch, on
 * other terms: the credit went into something else) or `unreadable`. That covers another target or wrapper, calldata
 * this page can't decode, a direct call whose sender the reader didn't give, and a useCredits by another account or of
 * other credits: such a call says nothing of where this credit went (a record that points at it is wrong, not proof).
 */
export function readSale(tx: SplitTransaction, sale: SplitSale): 'match' | 'other' | 'unreadable' {
  if (lower(tx.to) !== lower(sale.manager)) return 'unreadable'
  let user: string
  let inner: Hex
  try {
    const meta = decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data: tx.input })
    if (meta.functionName !== 'executeMetaTransaction') return 'unreadable'
    user = meta.args[0] as string
    inner = meta.args[1] as Hex
  } catch {
    // Not relayed: the call itself, sent by the wallet that pays, which is the transaction's sender.
    if (!tx.from) return 'unreadable'
    user = tx.from
    inner = tx.input
  }
  let args
  try {
    const call = decodeFunctionData({ abi: CREDITS_MANAGER_ABI, data: inner })
    if (call.functionName !== 'useCredits') return 'unreadable'
    ;[args] = call.args
  } catch {
    return 'unreadable'
  }
  const salt = lower(idToSalt(sale.creditId))
  if (lower(user) !== lower(sale.buyer) || !args.credits.some((c) => lower(c.salt) === salt)) return 'unreadable'
  const creditWei = BigInt(sale.creditWei)
  const call = args.externalCall
  const [credit] = args.credits
  const same =
    args.credits.length === 1 &&
    credit !== undefined &&
    credit.value === creditWei &&
    args.maxCreditedValue === creditWei &&
    args.maxUncreditedValue === BigInt(sale.gapWei) &&
    args.customExternalCallSignature === '0x' &&
    lower(call.target) === lower(sale.target) &&
    lower(call.selector) === lower(sale.selector) &&
    lower(call.data) === lower(sale.callData)
  return same ? 'match' : 'other'
}

/** Whether `tx` is the buyer's relayed useCredits at the sale's manager, carrying exactly the sale's terms. */
export function carriesSale(tx: SplitTransaction, sale: SplitSale): boolean {
  return readSale(tx, sale) === 'match'
}

/** How long the one second look at a transaction waits, when its receipt is read and the transaction itself isn't yet. */
export const SPLIT_TX_RETRY_MS = 1_000

/**
 * What the transaction at `hash` says about the sale (see SplitOutcome). A read that fails is `unknown`, never `other`. A
 * node that serves a receipt a moment before the transaction it is for gets one more look, `SPLIT_TX_RETRY_MS` later
 * (`sleep` stands in for the wait in tests), so a mined purchase isn't left confirming for that alone.
 */
export async function splitExecution(
  evidence: SplitEvidenceReader,
  sale: SplitSale,
  hash: string,
  opts: { sleep?: (ms: number) => Promise<void> } = {}
): Promise<SplitOutcome> {
  let receipt: TxReceipt | null
  let tx: SplitTransaction | null
  try {
    ;[receipt, tx] = await Promise.all([evidence.receipt(sale.chainId, hash), evidence.transaction(sale.chainId, hash)])
  } catch {
    return 'unknown'
  }
  if (!tx && receipt) {
    await (opts.sleep ?? defaultSleep)(SPLIT_TX_RETRY_MS)
    tx = await evidence.transaction(sale.chainId, hash).catch(() => null)
  }
  if (!tx) return 'unknown'
  const read = readSale(tx, sale)
  if (read === 'unreadable') return 'unknown'
  if (read === 'other') return 'other'
  if (!receipt) return 'unknown'
  // A success carrying the terms is the purchase, whatever bytes carried it; a revert is only that invocation's.
  if (receipt.status === 'success') return 'bought'
  return sale.envelope !== undefined && lower(splitEnvelope(tx.input)) === lower(sale.envelope) ? 'reverted' : 'unknown'
}
