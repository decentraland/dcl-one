// The player's wallet as ONE confirmed gift may use it (../metaTxGuard): the Decentraland collection the token
// belongs to, on the build's chain, for the login's account, whose call is `transferFrom(sender, recipient, tokenId)`,
// exactly those, and nothing else. The intent is current only while the gift's dialog is up after its SEND GIFT, so
// a signature for a gift closed meanwhile is dropped.

import { decodeFunctionData, type Hex } from 'viem'
import { ERC721_ABI } from '@shop/market/lib/contracts'
import type { Eip1193Provider } from '../../../data/auth/wallet'
import { guardedMetaTxProvider, lower, refuse } from '../metaTxGuard'

export interface GiftIntent {
  /** The sender: the login's account, lowercase. */
  from: string
  chainId: number
  /** The token's collection on `chainId`, lowercase, with its EIP-712 domain. */
  collection: { address: string; name: string; version: string }
  /** The recipient, lowercase. */
  to: string
  tokenId: bigint
  /** The meta-transaction nonce it must sign, when pinned (TRY AGAIN after a refused gift signs that one's). */
  nonce?: bigint
  /** False once the login or the wallet changed, or the gift's dialog was closed, since the player confirmed. */
  current(): boolean
}

const ONLY = 'Gift only sends the item you confirmed.'

/** `transferFrom(sender, recipient, tokenId)` as the gift names it, and nothing else. */
function checkTransfer(data: unknown, intent: GiftIntent): void {
  if (typeof data !== 'string' || !/^0x[0-9a-fA-F]*$/.test(data)) refuse(ONLY)
  let decoded
  try {
    decoded = decodeFunctionData({ abi: ERC721_ABI, data: data as Hex })
  } catch {
    refuse(ONLY)
  }
  if (decoded.functionName !== 'transferFrom') refuse(ONLY)
  const [from, to, tokenId] = decoded.args as readonly [string, string, bigint]
  // The exact length refuses trailing bytes the decoder would ignore.
  if (lower(from) !== intent.from || lower(to) !== intent.to || tokenId !== intent.tokenId || data.length !== 2 + 8 + 64 * 3) refuse(ONLY)
}

export function guardedGiftProvider(provider: Eip1193Provider, intent: GiftIntent): Eip1193Provider {
  return guardedMetaTxProvider(provider, { from: intent.from, chainId: intent.chainId, contract: intent.collection, nonce: intent.nonce, current: intent.current }, {
    feature: 'Gift',
    only: ONLY,
    checkCall: (data) => checkTransfer(data, intent)
  })
}
