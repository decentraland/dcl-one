// The in-world Shop's host (`@shop-host`, typed by library/ui/apps/shop/hostContract.ts): what it takes from
// this page, the web Shop's own client, reads, switches, LAND/NAME chain and credits mark. It stays in ui3
// with the web Shop modules it names, until the /play HUD leaves ui3.

import type { ShopHostContract } from '@dcl/one-ui/apps/shop/hostContract'
import { shopClient } from '../shop/api/host'
import CreditMark from '../shop/credits/components/CreditMark'
import { shopReads } from '../shop/discovery/reads'
import { hostOr } from '../shop/discovery/flags'
import { primeRemoteFlags, readRemoteFlags } from '../shop/discovery/lib/remoteFlags'
import { landChainId } from '../shop/land/chain'
import { rpcReader } from '../shop/land/evm'
import { readBannedNames } from '../shop/names/reads'

export { CreditMark, hostOr, landChainId, primeRemoteFlags, readBannedNames, readRemoteFlags, rpcReader, shopClient, shopReads }

/** Fails to compile when this host stops meeting the contract. */
export const shopHost: ShopHostContract = {
  shopClient,
  shopReads,
  hostOr,
  readRemoteFlags,
  primeRemoteFlags,
  landChainId,
  rpcReader: (chainId) => rpcReader(chainId),
  readBannedNames,
  CreditMark
}
