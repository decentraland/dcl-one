// What the chain says about ONE in-world purchase paid with Credits and MANA together (a split). Its terms are kept
// before its sale is signed (the buyer, the chain, the CreditsManager, the credit the node issued, the credit's exact
// value C, the MANA gap G and the external call as reviewed), and an execution counts as this purchase only when it
// carries exactly those terms: the buyer's meta-transaction to that manager, spending that credit, crediting C, taking G
// from the wallet, for that call. Exact sizing proves the amount, not which item a credit bought, so neither a successful
// receipt for a hash the relay answered nor the credit's SETTLED record is enough on its own: the transaction itself is
// read and matched.

import { decodeFunctionData, type Hex } from 'viem'
import { CREDITS_MANAGER_ABI, OFFCHAIN_META_TX_ABI } from '@shop/checkout/chain/abi'
import { idToSalt } from '../../../shop/checkout/purchase/encoding'
import type { ManaEvidence } from './manaEvidence'

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
  /** The external call's data, as reviewed (its price, coupon and buyer). */
  callData: Hex
}

/**
 * - `bought`: a mined, successful execution of exactly this purchase;
 * - `reverted`: this purchase's own execution, mined and reverted (its credit unspent);
 * - `other`: a transaction that isn't this purchase (another call, another credit, another manager or sender);
 * - `unknown`: nothing read yet (no receipt, no transaction, or a read that failed).
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
    hex(s.callData)
  )
}

/** Whether `tx` is the buyer's relayed useCredits at the sale's manager, carrying exactly the sale's terms. */
export function carriesSale(tx: { to: string | null; input: Hex }, sale: SplitSale): boolean {
  if (lower(tx.to) !== lower(sale.manager)) return false
  try {
    const meta = decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data: tx.input })
    if (meta.functionName !== 'executeMetaTransaction' || lower(meta.args[0]) !== lower(sale.buyer)) return false
    const call = decodeFunctionData({ abi: CREDITS_MANAGER_ABI, data: meta.args[1] as Hex })
    if (call.functionName !== 'useCredits') return false
    const [args] = call.args
    const creditWei = BigInt(sale.creditWei)
    return (
      args.credits.length === 1 &&
      lower(args.credits[0].salt) === lower(idToSalt(sale.creditId)) &&
      args.credits[0].value === creditWei &&
      args.maxCreditedValue === creditWei &&
      args.maxUncreditedValue === BigInt(sale.gapWei) &&
      args.customExternalCallSignature === '0x' &&
      lower(args.externalCall.data) === lower(sale.callData)
    )
  } catch {
    return false
  }
}

/** What the transaction at `hash` says about the sale (see SplitOutcome). A read that fails is `unknown`, never `other`. */
export async function splitExecution(evidence: Pick<ManaEvidence, 'receipt' | 'transaction'>, sale: SplitSale, hash: string): Promise<SplitOutcome> {
  let receipt: Awaited<ReturnType<ManaEvidence['receipt']>>
  let tx: Awaited<ReturnType<ManaEvidence['transaction']>>
  try {
    ;[receipt, tx] = await Promise.all([evidence.receipt(sale.chainId, hash), evidence.transaction(sale.chainId, hash)])
  } catch {
    return 'unknown'
  }
  if (!tx) return 'unknown'
  if (!carriesSale(tx, sale)) return 'other'
  if (!receipt) return 'unknown'
  return receipt.status === 'success' ? 'bought' : 'reverted'
}
