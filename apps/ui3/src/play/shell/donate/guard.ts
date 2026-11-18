// The player's wallet as ONE confirmed tip may use it. Bound to that tip's intent (the login's account, the
// chain, the MANA token, the creator and the amount the player confirmed in the shell's own panel), it forwards
// only what sending exactly that MANA takes, and decodes it first:
//
//  - the account and the chain;
//  - the MANA token's meta-transaction, in its exact EIP-712 schema (the token's domain name and version, the
//    token, the chain as the domain's salt; MetaTransaction{nonce, from, functionSignature}) for the tipper;
//  - whose call is `transfer(creator, amount)`, exactly those, and nothing else.
//
// Nothing is sent from the wallet itself: a transaction beside the relayed one could pay twice. Before the
// signature it reads the wallet's selected account and checks the login is still the one that confirmed. Anything
// else is refused before it reaches the wallet.

import { decodeFunctionData, type Hex } from 'viem'
import { TRANSFER_ABI } from '@explorer-core/features/donate/tip'
import type { Eip1193Provider } from '../../../data/auth/wallet'
import { guardedMetaTxProvider, lower, refuse } from '../metaTxGuard'

export interface DonationIntent {
  /** The tipper: the login's account, lowercase. */
  from: string
  chainId: number
  /** The MANA token on `chainId`, lowercase, with its EIP-712 domain. */
  token: { address: string; name: string; version: string }
  /** The creator the shell resolved, lowercase. */
  to: string
  /** The confirmed amount, in wei. */
  wei: bigint
  /** The meta-transaction nonce it must sign, when pinned (a retry of a tip shown as failed signs that one's). */
  nonce?: bigint
  /** False once the login or the wallet changed since the player confirmed. */
  current(): boolean
}

const ONLY = 'Donate only sends the tip you confirmed.'

/** `transfer(creator, amount)` as confirmed, and nothing else. */
function checkTransfer(data: unknown, intent: DonationIntent): void {
  if (typeof data !== 'string' || !/^0x[0-9a-fA-F]*$/.test(data)) refuse(ONLY)
  let decoded
  try {
    decoded = decodeFunctionData({ abi: TRANSFER_ABI, data: data as Hex })
  } catch {
    refuse(ONLY)
  }
  const [to, amount] = decoded.args
  // The exact length refuses trailing bytes the decoder would ignore.
  if (lower(to) !== intent.to || amount !== intent.wei || data.length !== 2 + 8 + 64 * 2) refuse(ONLY)
}

export function guardedDonationProvider(provider: Eip1193Provider, intent: DonationIntent): Eip1193Provider {
  return guardedMetaTxProvider(provider, { from: intent.from, chainId: intent.chainId, contract: intent.token, nonce: intent.nonce, current: intent.current }, {
    feature: 'Donate',
    only: 'Donate only signs the tip you confirmed.',
    changed: 'Your sign-in changed meanwhile. Open Donate again to tip.',
    checkCall: (data) => checkTransfer(data, intent)
  })
}
