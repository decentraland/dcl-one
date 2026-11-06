// The buyer's wallet as the in-world Shop's purchase may use it: only the requests a Credits purchase
// makes (the account, the chain, the meta-transaction signature and its direct-transaction fallback), only
// for the login's own account, on the settlement chain, to the Shop's CreditsManager. Anything else is
// refused before it reaches the wallet, so neither a bug nor a changed offer can make it sign for another
// account, another chain or another contract.

import type { Eip1193Provider } from '../../../data/auth/wallet'
import { settlementChainId } from '../../../shop/checkout/chain/config'
import { requireContracts } from '../../../shop/checkout/chain/contracts'
import { chainIdSalt } from '../../../shop/checkout/chain/metatx'

/** A refused request, in EIP-1193's terms: 4100 unauthorized, 4200 unsupported. */
export class WalletRefusal extends Error {
  override name = 'WalletRefusal'
  constructor(
    message: string,
    readonly code: 4100 | 4200
  ) {
    super(message)
  }
}

export interface WalletGuardOptions {
  chainId?: number
  /** The contracts it may sign for or send to; the settlement chain's CreditsManager by default. */
  targets?: string[]
}

const lower = (v: unknown): string => (typeof v === 'string' ? v.toLowerCase() : '')
const short = (a: string): string => `${a.slice(0, 6)}…${a.slice(-4)}`

export function guardedProvider(provider: Eip1193Provider, buyer: string, opts: WalletGuardOptions = {}): Eip1193Provider {
  const chainId = opts.chainId ?? settlementChainId()
  const self = buyer.toLowerCase()
  const targets = new Set(
    (opts.targets ?? [requireContracts(chainId as Parameters<typeof requireContracts>[0]).creditsManager?.address ?? '']).filter(Boolean).map((t) => t.toLowerCase())
  )
  const chainHex = `0x${chainId.toString(16)}`
  const mine = (accounts: unknown): string[] => {
    const list = Array.isArray(accounts) ? accounts.map(lower) : []
    if (list[0] !== self) {
      throw new WalletRefusal(`Your wallet is using another account. Switch it to ${short(self)}, the one you signed in with, to buy.`, 4100)
    }
    return [self]
  }
  return {
    async request({ method, params }) {
      const args = (Array.isArray(params) ? params : []) as unknown[]
      switch (method) {
        case 'eth_accounts':
        case 'eth_requestAccounts':
          return mine(await provider.request({ method, params }))
        case 'eth_chainId':
          return provider.request({ method, params })
        case 'wallet_switchEthereumChain':
        case 'wallet_addEthereumChain': {
          const asked = lower((args[0] as { chainId?: unknown } | undefined)?.chainId)
          if (asked !== chainHex) throw new WalletRefusal('The Shop only switches your wallet to its own network.', 4100)
          return provider.request({ method, params })
        }
        case 'eth_signTypedData_v4': {
          const [from, data] = args
          if (lower(from) !== self) throw new WalletRefusal('The Shop signs only for the account you signed in with.', 4100)
          let typed: { primaryType?: unknown; domain?: { verifyingContract?: unknown; salt?: unknown; chainId?: unknown }; message?: { from?: unknown } }
          try {
            typed = typeof data === 'string' ? JSON.parse(data) : (data as typeof typed)
          } catch {
            throw new WalletRefusal('The Shop could not read what it was asked to sign.', 4100)
          }
          const domain = typed?.domain ?? {}
          const onChain = lower(domain.salt) === lower(chainIdSalt(chainId as Parameters<typeof chainIdSalt>[0])) || Number(domain.chainId) === chainId
          if (typed?.primaryType !== 'MetaTransaction' || !targets.has(lower(domain.verifyingContract)) || !onChain || lower(typed.message?.from) !== self) {
            throw new WalletRefusal('The Shop only signs its own purchases.', 4100)
          }
          return provider.request({ method, params })
        }
        case 'eth_sendTransaction': {
          const tx = (args[0] ?? {}) as { from?: unknown; to?: unknown; value?: unknown }
          const value = typeof tx.value === 'string' || typeof tx.value === 'number' || typeof tx.value === 'bigint' ? BigInt(tx.value) : 0n
          if (lower(tx.from) !== self || !targets.has(lower(tx.to)) || value !== 0n) {
            throw new WalletRefusal('The Shop only sends its own purchases.', 4100)
          }
          return provider.request({ method, params })
        }
        default:
          throw new WalletRefusal(`The Shop doesn't use ${method}.`, 4200)
      }
    }
  }
}
