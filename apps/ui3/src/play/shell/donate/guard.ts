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

import { decodeFunctionData, parseAbi, type Hex } from 'viem'
import { chainSalt } from '../../../shop/market/lib/tx'
import type { ChainId } from '@shop/model/chain'
import type { Eip1193Provider } from '../../../data/auth/wallet'
import { assertWalletAccount, sameFields, sameKeys, WalletRefusal } from '../shop/walletGuard'

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

/** The MANA token's transfer, the one call a tip makes. */
export const TRANSFER_ABI = parseAbi(['function transfer(address to, uint256 amount) returns (bool)'])
const DOMAIN_FIELDS = [
  { name: 'name', type: 'string' },
  { name: 'version', type: 'string' },
  { name: 'verifyingContract', type: 'address' },
  { name: 'salt', type: 'bytes32' }
] as const
const META_TX_FIELDS = [
  { name: 'nonce', type: 'uint256' },
  { name: 'from', type: 'address' },
  { name: 'functionSignature', type: 'bytes' }
] as const

const lower = (v: unknown): string => (typeof v === 'string' ? v.toLowerCase() : '')

function refuse(why: string): never {
  throw new WalletRefusal(why, 4100)
}

/** `transfer(creator, amount)` as confirmed, and nothing else. */
function checkTransfer(data: unknown, intent: DonationIntent): void {
  if (typeof data !== 'string' || !/^0x[0-9a-fA-F]*$/.test(data)) refuse('Donate only sends the tip you confirmed.')
  let decoded
  try {
    decoded = decodeFunctionData({ abi: TRANSFER_ABI, data: data as Hex })
  } catch {
    refuse('Donate only sends the tip you confirmed.')
  }
  const [to, amount] = decoded.args
  // The exact length refuses trailing bytes the decoder would ignore.
  if (lower(to) !== intent.to || amount !== intent.wei || data.length !== 2 + 8 + 64 * 2) refuse('Donate only sends the tip you confirmed.')
}

/**
 * The MANA token's meta-transaction for the tipper, exactly as the sender builds it, and nothing more.
 * Returns the typed data as checked, for the wallet to sign that and not the text it was read from.
 */
function checkMetaTransaction(from: unknown, data: unknown, intent: DonationIntent): unknown {
  if (lower(from) !== intent.from) refuse('Donate signs only for the account you signed in with.')
  let typed: unknown
  try {
    typed = typeof data === 'string' ? JSON.parse(data) : data
  } catch {
    refuse('Donate could not read what it was asked to sign.')
  }
  if (!sameKeys(typed, ['types', 'domain', 'primaryType', 'message'])) refuse('Donate only signs the tip you confirmed.')
  const { types, domain, primaryType, message } = typed as Record<string, Record<string, unknown>>
  const ok =
    sameKeys(types, ['EIP712Domain', 'MetaTransaction']) &&
    sameFields(types.EIP712Domain, DOMAIN_FIELDS) &&
    sameFields(types.MetaTransaction, META_TX_FIELDS) &&
    sameKeys(domain, ['name', 'version', 'verifyingContract', 'salt']) &&
    domain.name === intent.token.name &&
    domain.version === intent.token.version &&
    lower(domain.verifyingContract) === intent.token.address &&
    lower(domain.salt) === chainSalt(intent.chainId as ChainId) &&
    (primaryType as unknown) === 'MetaTransaction' &&
    sameKeys(message, ['nonce', 'from', 'functionSignature']) &&
    /^\d{1,78}$/.test(String(message.nonce)) &&
    (intent.nonce === undefined || BigInt(String(message.nonce)) === intent.nonce) &&
    lower(message.from) === intent.from
  if (!ok) refuse('Donate only signs the tip you confirmed.')
  checkTransfer(message.functionSignature, intent)
  return typed
}

export function guardedDonationProvider(provider: Eip1193Provider, intent: DonationIntent): Eip1193Provider {
  const ready = async (): Promise<void> => {
    if (!intent.current()) refuse('Your sign-in changed meanwhile. Open Donate again to tip.')
    await assertWalletAccount(provider, intent.from)
    if (!intent.current()) refuse('Your sign-in changed meanwhile. Open Donate again to tip.')
  }
  return {
    async request({ method, params }) {
      const args = (Array.isArray(params) ? params : []) as unknown[]
      switch (method) {
        case 'eth_accounts':
        case 'eth_requestAccounts':
          await assertWalletAccount(provider, intent.from, method === 'eth_requestAccounts')
          return [intent.from]
        case 'eth_chainId':
          return provider.request({ method, params })
        case 'eth_signTypedData_v4': {
          if (args.length !== 2) refuse('Donate only signs the tip you confirmed.')
          const checked = JSON.stringify(checkMetaTransaction(args[0], args[1], intent))
          await ready()
          return provider.request({ method, params: [args[0], checked] })
        }
        default:
          throw new WalletRefusal(`Donate doesn't use ${method}.`, 4200)
      }
    },
    ...(provider.on ? { on: provider.on.bind(provider) } : {}),
    ...(provider.removeListener ? { removeListener: provider.removeListener.bind(provider) } : {})
  }
}
