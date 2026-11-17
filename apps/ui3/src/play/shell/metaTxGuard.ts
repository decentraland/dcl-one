// The player's wallet as ONE gasless meta-transaction may use it: the shell's Donate and Gift each bind it to their
// own intent. It forwards only:
//
//  - the account and the chain;
//  - the contract's meta-transaction, in its exact EIP-712 schema (the contract's domain name and version, the
//    contract, the chain as the domain's salt; MetaTransaction{nonce, from, functionSignature}) for the signer,
//    pinned to one nonce when the intent names it;
//  - whose call the intent's `checkCall` accepts, and nothing else.
//
// Nothing is sent from the wallet itself: a transaction beside the relayed one could land twice. Before the
// signature it reads the wallet's selected account and checks the login is still the one that started; after it,
// that the operation is still wanted (a signature for one closed meanwhile is dropped, never relayed). Anything else
// is refused before it reaches the wallet.

import { chainSalt } from '../../shop/market/lib/tx'
import type { ChainId } from '@shop/model/chain'
import type { Eip1193Provider } from '../../data/auth/wallet'
import { assertWalletAccount, sameFields, sameKeys, WalletRefusal } from './shop/walletGuard'

export interface MetaTxIntent {
  /** The signer: the login's account, lowercase. */
  from: string
  chainId: number
  /** The contract on `chainId`, lowercase, with its EIP-712 domain. */
  contract: { address: string; name: string; version: string }
  /** The meta-transaction nonce it must sign, when pinned (a retry that must replace, never add to, a signed one). */
  nonce?: bigint
  /** False once the login or the wallet changed, or the operation was given up, since it started. */
  current(): boolean
}

export interface MetaTxGuard {
  /** What the refusals name: "Donate", "Gift". */
  feature: string
  /** What the guard refuses everything else with ("Donate only sends the tip you confirmed."). */
  only: string
  /** Throws (through `refuse`) unless `functionSignature` is exactly the call the intent allows. */
  checkCall(functionSignature: unknown, refuse: (why: string) => never): void
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

export const lower = (v: unknown): string => (typeof v === 'string' ? v.toLowerCase() : '')

export function refuse(why: string): never {
  throw new WalletRefusal(why, 4100)
}

/**
 * The contract's meta-transaction for the signer, exactly as the Shop's sender builds it, and nothing more. Returns
 * the typed data as checked, for the wallet to sign that and not the text it was read from.
 */
function checkMetaTransaction(from: unknown, data: unknown, intent: MetaTxIntent, guard: MetaTxGuard): unknown {
  if (lower(from) !== intent.from) refuse(`${guard.feature} signs only for the account you signed in with.`)
  let typed: unknown
  try {
    typed = typeof data === 'string' ? JSON.parse(data) : data
  } catch {
    refuse(`${guard.feature} could not read what it was asked to sign.`)
  }
  if (!sameKeys(typed, ['types', 'domain', 'primaryType', 'message'])) refuse(guard.only)
  const { types, domain, primaryType, message } = typed as Record<string, Record<string, unknown>>
  const ok =
    sameKeys(types, ['EIP712Domain', 'MetaTransaction']) &&
    sameFields(types.EIP712Domain, DOMAIN_FIELDS) &&
    sameFields(types.MetaTransaction, META_TX_FIELDS) &&
    sameKeys(domain, ['name', 'version', 'verifyingContract', 'salt']) &&
    domain.name === intent.contract.name &&
    domain.version === intent.contract.version &&
    lower(domain.verifyingContract) === intent.contract.address &&
    lower(domain.salt) === chainSalt(intent.chainId as ChainId) &&
    (primaryType as unknown) === 'MetaTransaction' &&
    sameKeys(message, ['nonce', 'from', 'functionSignature']) &&
    /^\d{1,78}$/.test(String(message.nonce)) &&
    (intent.nonce === undefined || BigInt(String(message.nonce)) === intent.nonce) &&
    lower(message.from) === intent.from
  if (!ok) refuse(guard.only)
  guard.checkCall(message.functionSignature, refuse)
  return typed
}

export function guardedMetaTxProvider(provider: Eip1193Provider, intent: MetaTxIntent, guard: MetaTxGuard): Eip1193Provider {
  const changed = `Your sign-in changed meanwhile. Open ${guard.feature} again.`
  const ready = async (): Promise<void> => {
    if (!intent.current()) refuse(changed)
    await assertWalletAccount(provider, intent.from)
    if (!intent.current()) refuse(changed)
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
          if (args.length !== 2) refuse(guard.only)
          const checked = JSON.stringify(checkMetaTransaction(args[0], args[1], intent, guard))
          await ready()
          const signature = await provider.request({ method, params: [args[0], checked] })
          // Given up while the wallet was asking: the signature is dropped before anything can relay it.
          if (!intent.current()) refuse(changed)
          return signature
        }
        default:
          throw new WalletRefusal(`${guard.feature} doesn't use ${method}.`, 4200)
      }
    },
    ...(provider.on ? { on: provider.on.bind(provider) } : {}),
    ...(provider.removeListener ? { removeListener: provider.removeListener.bind(provider) } : {})
  }
}
