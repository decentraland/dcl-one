// What the in-world Shop takes from the page it runs in, as `@shop-host` (each host maps the alias to its own
// module, as `@hud-host`): the web Shop's client and reads, its rollout switches, the Ethereum network and
// chain reader of its LAND and NAME flows, the banned-NAMEs list and the credits mark. The /play host's is
// apps/ui3/src/play/shopHost.ts, which checks itself against this type.

import type { ComponentType } from 'react'
import type { ShopClient } from '@shop/api/shop'
import type { Opts } from '@shop/discovery/api'
import type { HostShopFlags, RemoteFlagsAnswer } from '@shop/discovery/lib/rolloutFlags'
import type { ChainReader } from '@shop/land/read'
import type { ChainId } from '@shop/model/chain'

export interface ShopHostContract {
  /** The page's Shop API client. */
  shopClient: () => ShopClient
  /** The options the Shop's reads (discovery/api, detail/api) take: the client and the creator-sales switch. */
  shopReads: (signal?: AbortSignal) => Opts
  /** A switch the page declares, over the remote one (null: neither says). */
  hostOr: (key: keyof HostShopFlags, remote: boolean | null) => boolean | null
  /** The remote rollout flags, read for the app named. */
  readRemoteFlags: (signal?: AbortSignal, app?: string) => Promise<RemoteFlagsAnswer>
  /** The flags the page holds, primed from an answer read here. */
  primeRemoteFlags: (flags: Record<string, boolean> | null, variants?: Record<string, string>) => void
  /** The Ethereum network the LAND and NAME flows run on. */
  landChainId: () => ChainId
  /** A contract reader on a chain. */
  rpcReader: (chainId: number) => ChainReader
  /** The NAMEs the lists server bans. */
  readBannedNames: (signal?: AbortSignal) => Promise<string[]>
  /** The credits mark. */
  CreditMark: ComponentType<{ size?: number; className?: string }>
}
