// The CreditsManager an in-world purchase may spend through: a fact of the deployment, not of the
// server's answer. It is the Shop address book's manager for the chain, the one the web checkout spends
// through too, so the shell and the rails can't disagree. The book names the deployment's own manager when
// the build does (`VITE_SHOP_CREDITS_MANAGER` on the chain `VITE_SHOP_CHAIN_ID` names, the address the Shop
// server's CREDITS_MANAGER_CONTRACT holds); otherwise the chain's registry manager, which the server refuses
// to issue credits for, so nothing can be spent until one is set. A credit the server authorizes for any
// other manager is refused at signing (./walletGuard), before it reaches the wallet.

import { settlementChainId } from '../../../shop/checkout/chain/config'
import { chainContracts } from '../../../shop/checkout/chain/contracts'
import type { ChainId } from '@shop/model/chain'

export interface TrustedManager {
  chainId: number
  /** Lowercase. */
  address: string
  /** The EIP-712 domain the manager signs meta-transactions under. */
  name: string
  version: string
}

export function trustedCreditsManager(chainId: number = settlementChainId()): TrustedManager | null {
  const book = chainContracts(chainId as ChainId)?.creditsManager
  if (!book) return null
  return { chainId, address: book.address.toLowerCase(), name: book.name, version: book.version }
}
