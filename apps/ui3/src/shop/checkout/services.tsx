// Shared Shop dependencies. Hosts may override services; detail pages and checkout otherwise use
// the same node quote source. Explicit fetchOffers: null keeps the cart in a price-check outage.

import { QueryClient, QueryClientContext, QueryClientProvider } from "@tanstack/react-query";
import { createContext, useContext, useMemo, useState, type ReactNode } from "react";

import { shopClient, type ShopClient } from "../api/shop";
import type { ShopSigner } from "../api/signer";
import { type OfferFetcher } from "../cart/revalidate";
import { shopCart, type CartStore } from "../cart/store";
import { shopChainReader, type ChainReader } from "./chain/rpc";
import { liveResolvers, type SettlementResolvers } from "./purchase/cart";
import type { RailContext } from "./purchase/rail";

export type ShopServices = {
  client: ShopClient;
  reader: ChainReader;
  cart: CartStore;
  /** Override cart quoting; undefined uses the shared live offer source, null explicitly disables it. */
  fetchOffers: OfferFetcher | null | undefined;
  resolvers: SettlementResolvers;
  relayerUrl?: string;
  fetchImpl?: typeof fetch;
  gasless?: boolean;
  wait?: RailContext["wait"];
  /** Leaves the page for Stripe or a card provider. The Explorer opens its own browser instead. */
  openExternal: (url: string) => void;
  /** Signs the credits balance stream; without one the balance polls while credits are held. */
  signer?: ShopSigner | null;
  /** A page on setting up a web3 wallet, for the MANA guidance. */
  walletSetupUrl?: string;
};

const Ctx = createContext<Partial<ShopServices>>({});

function defaults(partial: Partial<ShopServices>): ShopServices {
  const client = partial.client ?? shopClient();
  return {
    client,
    reader: partial.reader ?? shopChainReader(),
    cart: partial.cart ?? shopCart,
    fetchOffers: partial.fetchOffers,
    resolvers: partial.resolvers ?? liveResolvers(client),
    relayerUrl: partial.relayerUrl,
    fetchImpl: partial.fetchImpl,
    gasless: partial.gasless,
    wait: partial.wait,
    openExternal:
      partial.openExternal ??
      ((url: string) => {
        window.location.assign(url);
      }),
    signer: partial.signer ?? null,
    walletSetupUrl: partial.walletSetupUrl ?? "https://decentraland.org/help/wallets/",
  };
}

/** Provide some or all services to the Shopping and Credits pages below. */
export function ShopServicesProvider({ value, children }: { value: Partial<ShopServices>; children: ReactNode }) {
  const parent = useContext(Ctx);
  const merged = useMemo(() => ({ ...parent, ...value }), [parent, value]);
  return <Ctx.Provider value={merged}>{children}</Ctx.Provider>;
}

export function useShopServices(): ShopServices {
  const partial = useContext(Ctx);
  return useMemo(() => defaults(partial), [partial]);
}

/** The pages use react-query; a host without a QueryClientProvider gets a private client here. */
export function EnsureQueryClient({ children }: { children: ReactNode }) {
  const existing = useContext(QueryClientContext);
  const [own] = useState(() => (existing ? null : new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } } })));
  if (existing || !own) return <>{children}</>;
  return <QueryClientProvider client={own}>{children}</QueryClientProvider>;
}
