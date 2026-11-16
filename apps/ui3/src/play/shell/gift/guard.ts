// The player's wallet as ONE gift may use it. Bound to that gift's intent (the login's account, the chain, the
// Decentraland collection the token belongs to, the recipient and the token the shell checked), it forwards only
// what sending exactly that token takes, and decodes it first:
//
//  - the account and the chain;
//  - the collection's meta-transaction, in its exact EIP-712 schema (the collection's domain name and version, the
//    collection, the chain as the domain's salt; MetaTransaction{nonce, from, functionSignature}) for the sender;
//  - whose call is `transferFrom(sender, recipient, tokenId)`, exactly those, and nothing else.
//
// Nothing is sent from the wallet itself: the gift is gasless only. Before the signature it reads the wallet's
// selected account and checks the login is still the one that started the gift. Anything else is refused before it
// reaches the wallet.

import { decodeFunctionData, parseAbi, type Hex } from 'viem'
import { chainSalt } from '../../../shop/market/lib/tx'
import type { ChainId } from '@shop/model/chain'
import type { Eip1193Provider } from '../../../data/auth/wallet'
import { assertWalletAccount, sameFields, sameKeys, WalletRefusal } from '../shop/walletGuard'

export const TRANSFER_FROM_ABI = parseAbi(['function transferFrom(address from, address to, uint256 tokenId)'])

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
  /** False once the login or the wallet changed since the gift started. */
  current(): boolean
}

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

const ONLY = 'Gift only sends the item you picked.'
const lower = (v: unknown): string => (typeof v === 'string' ? v.toLowerCase() : '')

function refuse(why: string): never {
  throw new WalletRefusal(why, 4100)
}

/** `transferFrom(sender, recipient, tokenId)` as the gift names it, and nothing else. */
function checkTransfer(data: unknown, intent: GiftIntent): void {
  if (typeof data !== 'string' || !/^0x[0-9a-fA-F]*$/.test(data)) refuse(ONLY)
  let decoded
  try {
    decoded = decodeFunctionData({ abi: TRANSFER_FROM_ABI, data: data as Hex })
  } catch {
    refuse(ONLY)
  }
  const [from, to, tokenId] = decoded.args
  // The exact length refuses trailing bytes the decoder would ignore.
  if (lower(from) !== intent.from || lower(to) !== intent.to || tokenId !== intent.tokenId || data.length !== 2 + 8 + 64 * 3) refuse(ONLY)
}

/**
 * The collection's meta-transaction for the sender, exactly as the sender builds it, and nothing more. Returns the
 * typed data as checked, for the wallet to sign that and not the text it was read from.
 */
function checkMetaTransaction(from: unknown, data: unknown, intent: GiftIntent): unknown {
  if (lower(from) !== intent.from) refuse('Gift signs only for the account you signed in with.')
  let typed: unknown
  try {
    typed = typeof data === 'string' ? JSON.parse(data) : data
  } catch {
    refuse('Gift could not read what it was asked to sign.')
  }
  if (!sameKeys(typed, ['types', 'domain', 'primaryType', 'message'])) refuse(ONLY)
  const { types, domain, primaryType, message } = typed as Record<string, Record<string, unknown>>
  const ok =
    sameKeys(types, ['EIP712Domain', 'MetaTransaction']) &&
    sameFields(types.EIP712Domain, DOMAIN_FIELDS) &&
    sameFields(types.MetaTransaction, META_TX_FIELDS) &&
    sameKeys(domain, ['name', 'version', 'verifyingContract', 'salt']) &&
    domain.name === intent.collection.name &&
    domain.version === intent.collection.version &&
    lower(domain.verifyingContract) === intent.collection.address &&
    lower(domain.salt) === chainSalt(intent.chainId as ChainId) &&
    (primaryType as unknown) === 'MetaTransaction' &&
    sameKeys(message, ['nonce', 'from', 'functionSignature']) &&
    /^\d{1,78}$/.test(String(message.nonce)) &&
    (intent.nonce === undefined || BigInt(String(message.nonce)) === intent.nonce) &&
    lower(message.from) === intent.from
  if (!ok) refuse(ONLY)
  checkTransfer(message.functionSignature, intent)
  return typed
}

export function guardedGiftProvider(provider: Eip1193Provider, intent: GiftIntent): Eip1193Provider {
  const ready = async (): Promise<void> => {
    if (!intent.current()) refuse('Your sign-in changed meanwhile. Open Gift again to send it.')
    await assertWalletAccount(provider, intent.from)
    if (!intent.current()) refuse('Your sign-in changed meanwhile. Open Gift again to send it.')
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
          if (args.length !== 2) refuse(ONLY)
          const checked = JSON.stringify(checkMetaTransaction(args[0], args[1], intent))
          await ready()
          return provider.request({ method, params: [args[0], checked] })
        }
        default:
          throw new WalletRefusal(`Gift doesn't use ${method}.`, 4200)
      }
    },
    ...(provider.on ? { on: provider.on.bind(provider) } : {}),
    ...(provider.removeListener ? { removeListener: provider.removeListener.bind(provider) } : {})
  }
}
