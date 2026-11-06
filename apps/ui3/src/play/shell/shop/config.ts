// The CreditsManager an in-world purchase may spend through: a fact of the deployment, not of the
// server's answer. `VITE_SHOP_CREDITS_MANAGER` names it (the same address the Shop server's
// CREDITS_MANAGER_CONTRACT holds, a manager this deployment controls); without it, the chain's registry
// manager, which the server refuses to issue credits for, so nothing can be spent until one is set. A
// credit the server authorizes for any other manager is refused at signing (./walletGuard), before it
// reaches the wallet.

import { settlementChainId } from '../../../shop/checkout/chain/config'
import { chainContracts } from '../../../shop/checkout/chain/contracts'
import type { ChainId } from '../../../shop/model/chain'

export interface TrustedManager {
  chainId: number
  /** Lowercase. */
  address: string
  /** The EIP-712 domain the manager signs meta-transactions under. */
  name: string
  version: string
}

const ADDRESS = /^0x[0-9a-fA-F]{40}$/

function configured(): string | null {
  const value = (import.meta.env?.VITE_SHOP_CREDITS_MANAGER as string | undefined)?.trim()
  return value && ADDRESS.test(value) ? value.toLowerCase() : null
}

export function trustedCreditsManager(chainId: number = settlementChainId(), address: string | null = configured()): TrustedManager | null {
  const book = chainContracts(chainId as ChainId)?.creditsManager
  if (!book) return null
  return { chainId, address: address ?? book.address.toLowerCase(), name: book.name, version: book.version }
}
