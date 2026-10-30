import { useMemo, type ReactNode } from "react";
import { MemoryRouter, Route, Routes } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import type { AuthIdentity } from "../../data/auth/identity";
import { createCartStore, type CartStorage, type CartStore } from "../cart/store";
import type { OfferFetcher } from "../cart/revalidate";
import type { CartListingRef, PurchaseOffer, PurchaseState } from "../model/purchase";
import type { Session } from "../model/settlement";
import { BOOK, SIGNED_OUT, web2, web3 } from "../payment/fixtures";
import { ShopWalletProvider } from "../wallet/context";
import type { ShopWalletHost } from "../wallet/session";
import { batchAuthorization, createFakeChain, createFakeRelay, createFakeShop, createFakeWallet, creatorTrade, resaleTrade, type FakeHandlers } from "./fakes";
import type { SettlementResolvers } from "./purchase/cart";
import { ShopServicesProvider, type ShopServices } from "./services";

export type DemoLine = {
  offer: (session: Session) => PurchaseOffer;
  listing?: CartListingRef;
  name: string;
  thumbnail?: string;
  category?: string;
  rarity?: string;
  method?: "credits" | "mana";
  quantity?: number;
  unseen?: boolean;
};

export type DemoScenario = {
  session: "signedOut" | "web2" | "web3";
  lines?: DemoLine[];
  offers?: "live" | "outage" | "failing" | "repriced" | "sold";
  balanceCents?: number;
  held?: { cents: number; credits: number; releasesAtSeconds: number | null; purchases?: unknown[] };
  authorize?: "ok" | "insufficient" | "disabled";
  relay?: "ok" | "refuse" | "down";
  confirm?: "success" | "reverted" | "none";
  walletChainId?: number;
  walletAccount?: string;
  noSigner?: boolean;
  checkout?: (lines: { id: string }[], cart: CartStore) => PurchaseState | null;
  path?: string;
  handlers?: FakeHandlers;
  resolvers?: Partial<SettlementResolvers>;
};

function memoryStorage(): CartStorage {
  const data = new Map<string, string>();
  return { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v), removeItem: (k) => void data.delete(k) };
}

export function demoIdentity(signer: string): AuthIdentity {
  return {
    signer,
    ephemeral: { address: "0x0000000000000000000000000000000000000001", privateKey: `0x${"11".repeat(32)}` },
    expiration: new Date(Date.now() + 86_400_000).toISOString(),
    authChain: [],
  };
}

function sessionOf(kind: DemoScenario["session"]): Session {
  return kind === "web3" ? web3() : kind === "web2" ? web2() : SIGNED_OUT;
}

function bump(offer: PurchaseOffer): PurchaseOffer {
  return {
    ...offer,
    quotes: offer.quotes.map((q) => (q.method === "credits" ? { ...q, quoted: (q.quoted + 20) as typeof q.quoted } : { ...q, amount: (q.amount + 10n ** 17n) as typeof q.amount })),
  };
}

export function createDemo(s: DemoScenario) {
  const session = sessionOf(s.session);
  const storage = memoryStorage();
  const cart = createCartStore({ storage: () => storage });
  if (session.kind !== "signedOut") cart.scopeTo(BOOK.buyer);
  const lines = (s.lines ?? []).map((l) => {
    const offer = l.offer(session);
    const line = cart.add({
      asset: offer.asset,
      listing: l.listing ?? {},
      display: { name: l.name, thumbnail: l.thumbnail, category: l.category, rarity: l.rarity, href: `/shop/items` },
      quantity: l.quantity,
      available: offer.available,
      seen: l.unseen ? null : offer,
    });
    if (l.method) cart.setMethod(line.id, l.method);
    return { line, spec: l };
  });
  const seeded = s.checkout?.(lines.map((l) => l.line), cart) ?? null;
  if (seeded) cart.replaceCheckout(seeded);

  const chain = createFakeChain();
  let clock = 0;
  const relay = createFakeRelay({ mode: s.relay ?? "ok", chain, confirm: s.confirm ?? "success" });
  const balanceCents = s.balanceCents ?? 100_000;
  const shop = createFakeShop({
    userCredits: () => ({ credits: [], totalCredits: 0, totals: { expiring: 0, nonExpiring: 0 }, usd: { balanceCents, credits: Math.floor(balanceCents / 10), ...(s.held ? { held: s.held } : {}) } }),
    authorizeCreditBatch: ({ body }) => {
      if (s.authorize === "insufficient") return { status: 402, body: { error: "Insufficient USD credits", balanceCents, requiredCents: balanceCents + 250 } };
      if (s.authorize === "disabled") return { status: 404, body: { error: "USD credits are not enabled" } };
      return batchAuthorization((body as { items: { usdPriceCents: number }[] }).items);
    },
    cancelCreditAuthorizations: ({ body }) => ({ released: (body as { salts: string[] }).salts.length, retired: 0, spendableAtSeconds: null }),
    reportCreditSubmission: () => ({ recorded: 1 }),
    userPurchases: () => ({ purchases: [], total: 0 }),
    packs: () => ({ packs: [
      { id: "pack_5", usd: 5.99, credits: 40, order: 1 },
      { id: "pack_10", usd: 11.99, credits: 100, recommended: true, order: 2 },
      { id: "pack_25", usd: 29.99, credits: 260, order: 3 },
      { id: "pack_50", usd: 59.99, credits: 540, order: 4 },
    ] }),
    createPackOrder: ({ body }) => ({ orderId: `order-${(body as { packId: string }).packId}`, url: "https://checkout.stripe.com/c/pay/demo" }),
    ...s.handlers,
  });
  const wallet = createFakeWallet({ chainId: s.walletChainId ?? 137, account: s.walletAccount ?? BOOK.buyer });

  const offersMode = s.offers ?? "live";
  const fetchOffers: OfferFetcher | null =
    offersMode === "outage"
      ? null
      : async (asked) => {
          if (offersMode === "failing") throw new Error("quote route unavailable");
          const out: Record<string, PurchaseOffer | null> = {};
          for (const line of asked) {
            const entry = lines.find((l) => l.line.id === line.id);
            if (!entry) continue;
            const offer = entry.spec.offer(session);
            out[line.id] = offersMode === "repriced" ? bump(offer) : offersMode === "sold" && lines[0]?.line.id === line.id ? null : offer;
          }
          return out;
        };

  const resolvers: SettlementResolvers = {
    trade: async (line) => (line.listing.tradeId === "trade-resale-1" ? resaleTrade() : line.listing.tradeId ? creatorTrade({ id: line.listing.tradeId }) : null),
    store: async () => ({ priceWei: (2_300n * 10n ** 15n).toString(), available: 10 }),
    coupon: async () => undefined,
    ...s.resolvers,
  };

  const services: Partial<ShopServices> = {
    client: shop.client,
    reader: chain,
    cart,
    fetchOffers,
    resolvers,
    relayerUrl: "https://node.test/transactions-api/v1",
    fetchImpl: relay.fetchImpl,
    gasless: true,
    wait: {
      timeoutMs: 40,
      pollMs: 10,
      now: () => clock,
      sleep: async (ms: number) => {
        clock += ms;
      },
    },
    openExternal: () => {},
  };

  const host: ShopWalletHost = {
    identity: session.kind === "signedOut" ? null : demoIdentity(BOOK.buyer),
    walletKind: session.kind === "web3" ? "web3" : session.kind === "web2" ? "managed" : null,
    provider: s.noSigner ? null : () => wallet,
    onSignIn: () => {},
    onConnectWallet: () => {},
    onSwitchAccount: () => {},
  };

  return { session, cart, lines: lines.map((l) => l.line), shop, chain, relay, wallet, services, host };
}

export type Demo = ReturnType<typeof createDemo>;

export function DemoShell({
  demo,
  path = "/shop/cart",
  pattern,
  state,
  children,
  routes,
}: {
  demo: Demo;
  path?: string;
  pattern?: string;
  state?: unknown;
  children: ReactNode;
  routes?: Record<string, ReactNode>;
}) {
  const [pathname, search = ""] = path.split("?");
  const qc = useMemo(() => new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } } }), []);
  return (
    <QueryClientProvider client={qc}>
      <ShopServicesProvider value={demo.services}>
        <ShopWalletProvider host={demo.host} cart={demo.cart}>
          <MemoryRouter initialEntries={[{ pathname, search: search ? `?${search}` : "", state }]}>
            <Routes>
              <Route path={pattern ?? pathname} element={children} />
              {Object.entries(routes ?? {}).map(([p, el]) => (
                <Route key={p} path={p} element={el} />
              ))}
            </Routes>
          </MemoryRouter>
        </ShopWalletProvider>
      </ShopServicesProvider>
    </QueryClientProvider>
  );
}
