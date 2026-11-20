// What the chain says about ONE purchase paid with Credits and MANA together (a split), the web Shop's or /play's. Its terms are kept
// before its sale is signed (the buyer, the chain, the CreditsManager, the credit the node issued, the credit's exact
// value C, the MANA gap G and the external call as reviewed), and an execution counts as this purchase only when it
// carries exactly those terms: the buyer's meta-transaction to that manager, spending that credit, crediting C, taking G
// from the wallet, for that call. Exact sizing proves the amount, not which item a credit bought, so neither a successful
// receipt for a hash the relay answered nor the credit's SETTLED record is enough on its own: the transaction itself is
// read and matched.

import { decodeFunctionData, type Hex } from 'viem'
import { CREDITS_MANAGER_ABI, OFFCHAIN_META_TX_ABI } from '@shop/checkout/chain/abi'
import type { TxReceipt } from '../chain/rpc'
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
  /** Its line in the hold the web Shop and /play share (shop/checkout/purchase/splitHold), and the attempt it wrote there. */
  hold?: { line: string; attempt: string }
}

/**
 * - `bought`: a mined, successful execution of exactly this purchase;
 * - `reverted`: this purchase's own execution, mined and reverted (its credit unspent);
 * - `other`: a useCredits this page can read that is provably not this purchase: another sender, another credit, or this
 *   credit with other terms (another venue, function, call, share or gap);
 * - `unknown`: nothing that says either way: no transaction or receipt yet, a read that failed, or a transaction whose
 *   wrapper or target this page can't read (its destination alone says nothing about where the credit went).
 */
export type SplitOutcome = 'bought' | 'reverted' | 'other' | 'unknown'

const lower = (v: unknown): string => (typeof v === 'string' ? v.toLowerCase() : '')

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
    (s.hold === undefined ||
      (typeof s.hold === 'object' && s.hold !== null && typeof (s.hold as Record<string, unknown>).line === 'string' && typeof (s.hold as Record<string, unknown>).attempt === 'string'))
  )
}

/**
 * What `tx` says of the sale: `match` (the buyer's useCredits at the sale's manager carrying exactly its terms, relayed or
 * sent by the buyer's own wallet), `other` (a readable useCredits that provably isn't it) or `unreadable` (another target
 * or wrapper, calldata this page can't decode, or a direct call whose sender the reader didn't give).
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
  const creditWei = BigInt(sale.creditWei)
  const call = args.externalCall
  const [credit] = args.credits
  const same =
    lower(user) === lower(sale.buyer) &&
    args.credits.length === 1 &&
    credit !== undefined &&
    lower(credit.salt) === lower(idToSalt(sale.creditId)) &&
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

/** What the transaction at `hash` says about the sale (see SplitOutcome). A read that fails is `unknown`, never `other`. */
export async function splitExecution(evidence: SplitEvidenceReader, sale: SplitSale, hash: string): Promise<SplitOutcome> {
  let receipt: TxReceipt | null
  let tx: SplitTransaction | null
  try {
    ;[receipt, tx] = await Promise.all([evidence.receipt(sale.chainId, hash), evidence.transaction(sale.chainId, hash)])
  } catch {
    return 'unknown'
  }
  if (!tx) return 'unknown'
  const read = readSale(tx, sale)
  if (read === 'unreadable') return 'unknown'
  if (read === 'other') return 'other'
  if (!receipt) return 'unknown'
  return receipt.status === 'success' ? 'bought' : 'reverted'
}
