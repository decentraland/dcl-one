// The in-world Shop's host (`@shop-host`, typed by library/ui/apps/shop/hostContract.ts): what it takes from
// this page, the web Shop's own client, reads, switches, LAND/NAME chain and credits mark. It stays in ui3
// with the web Shop modules it names, until the /play HUD leaves ui3.

import type { ShopHostContract } from '@dcl/one-ui/apps/shop/hostContract'
import type { ShopWalletKind } from '@dcl/one-ui/apps/shop/shopProtocol'
import { shopClient as webShopClient } from '../shop/api/host'
import WebCreditMark from '../shop/credits/components/CreditMark'
import { shopReads as webShopReads } from '../shop/discovery/reads'
import { hostOr as webHostOr } from '../shop/discovery/flags'
import { primeRemoteFlags as webPrimeRemoteFlags, readRemoteFlags as webReadRemoteFlags } from '../shop/discovery/lib/remoteFlags'
import { landChainId as webLandChainId } from '../shop/land/chain'
import { rpcReader as webRpcReader } from '../shop/land/evm'
import { readBannedNames as webReadBannedNames } from '../shop/names/reads'
import type { WalletKind } from '../shop/wallet/session'

/** This page's host, checked against the contract. */
const shopHost: ShopHostContract = {
  shopClient: webShopClient,
  shopReads: webShopReads,
  hostOr: webHostOr,
  readRemoteFlags: webReadRemoteFlags,
  primeRemoteFlags: webPrimeRemoteFlags,
  landChainId: webLandChainId,
  rpcReader: (chainId) => webRpcReader(chainId),
  readBannedNames: webReadBannedNames,
  CreditMark: WebCreditMark
}

// What the in-world Shop imports: the contract's members, typed as the contract types them, so it compiles
// against the contract and not against this page's wider implementations.
export const { shopClient, shopReads, hostOr, readRemoteFlags, primeRemoteFlags, landChainId, rpcReader, readBannedNames, CreditMark } = shopHost

// The in-world Shop names the web Shop's wallet kinds itself (shopProtocol): this fails to compile when they part.
type Same<A, B> = [A] extends [B] ? ([B] extends [A] ? true : never) : never
const walletKindsMatch: Same<ShopWalletKind, WalletKind> = true
void walletKindsMatch
