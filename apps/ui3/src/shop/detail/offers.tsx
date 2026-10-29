// Detail pages and checkout share the node's PurchaseOffer source. The server decides price and
// eligibility; failed quotes keep the payment choices visible but unavailable, with a retry.

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import type { OfferFetcher } from "../cart/revalidate";
import { purchaseOfferSource } from "../api/offers";
import { useShopServices } from "../checkout/services";
import { assetKey, type AssetId } from "../model/asset";
import type { CartLine, CartListingRef, PurchaseOffer } from "../model/purchase";
import type { Session } from "../model/settlement";
import type { GuidanceRevalidation } from "../wallet/ManaGuidance";

export type OfferQuery = {
  /** The caller's key for the answer (a cart line id, a listing key). */
  key: string;
  asset: AssetId;
  listing: CartListingRef;
  quantity?: number;
};

/** Answers the server's offer per query key; null for a listing that no longer exists. */
export type OfferSource = (queries: OfferQuery[], session: Session, signal: AbortSignal) => Promise<Record<string, PurchaseOffer | null>>;

export class OfferRouteMissing extends Error {
  override name = "OfferRouteMissing";
}

export const missingOfferRoute: OfferSource = async () => {
  throw new OfferRouteMissing("No purchase-offer route is mounted on this node yet.");
};

const OfferSourceContext = createContext<OfferSource | null>(null);

export function ShopOfferProvider({ source, children }: { source: OfferSource; children: ReactNode }) {
  return <OfferSourceContext.Provider value={source}>{children}</OfferSourceContext.Provider>;
}

export function useOfferSource(): OfferSource {
  const override = useContext(OfferSourceContext);
  const { client } = useShopServices();
  return useMemo(() => override ?? purchaseOfferSource(client), [override, client]);
}

/** The same source as cart/revalidate's fetcher: one query per cart line, keyed by its id. */
export function cartOfferFetcher(source: OfferSource, session: Session): OfferFetcher {
  return (lines: CartLine[], signal: AbortSignal) =>
    source(
      lines.map((line) => ({ key: line.id, asset: line.asset, listing: line.listing, quantity: line.quantity })),
      session,
      signal,
    );
}

export type OffersStatus = "idle" | "loading" | "ready" | "routeMissing" | "failed";

export type OffersState = {
  status: OffersStatus;
  offers: Record<string, PurchaseOffer | null>;
  error: unknown;
  retry(): void;
};

export function offerQueryKey(asset: AssetId, listing: CartListingRef = {}): string {
  const via = listing.tradeId ?? listing.orderId ?? listing.rentalId ?? listing.name ?? listing.acquisition ?? "trade";
  return `${assetKey(asset)}|${via}`;
}

/**
 * The offers for `queries` under `session`, re-asked whenever the queries, the session or `revision`
 * (the wallet's account/network revision) change. A failed answer keeps nothing from before: prices are
 * money, and an old answer for a different session is not this one's.
 */
export function useOffers(queries: OfferQuery[], session: Session, revision = ""): OffersState {
  const source = useOfferSource();
  const [tick, setTick] = useState(0);
  const signature = useMemo(
    () => JSON.stringify(queries.map((q) => [q.key, assetKey(q.asset), q.listing, q.quantity ?? 1])),
    [queries],
  );
  const sessionKey = `${session.kind}:${session.kind === "web2" ? session.account : session.kind === "web3" ? session.wallet : "-"}`;
  const asked = `${signature}|${sessionKey}|${revision}|${tick}`;
  const [state, setState] = useState<Omit<OffersState, "retry"> & { asked: string | null }>({ status: "idle", offers: {}, error: null, asked: null });
  const queriesRef = useRef(queries);
  queriesRef.current = queries;

  useEffect(() => {
    const current = queriesRef.current;
    if (current.length === 0) {
      setState({ status: "idle", offers: {}, error: null, asked });
      return;
    }
    const controller = new AbortController();
    setState({ status: "loading", offers: {}, error: null, asked });
    source(current, session, controller.signal).then(
      (offers) => {
        if (!controller.signal.aborted) setState({ status: "ready", offers, error: null, asked });
      },
      (error) => {
        if (controller.signal.aborted) return;
        const missing = error instanceof OfferRouteMissing || (error as { name?: string })?.name === "OfferRouteMissing";
        setState({ status: missing ? "routeMissing" : "failed", offers: {}, error, asked });
      },
    );
    return () => controller.abort();
    // session is read through sessionKey; the object identity changes on every render of some hosts.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source, asked]);

  const retry = useCallback(() => setTick((n) => n + 1), []);
  // Until the effect has asked for THESE queries, an older answer is not theirs: report loading.
  if (state.asked !== asked) return { status: queries.length ? "loading" : "idle", offers: {}, error: null, retry };
  return { status: state.status, offers: state.offers, error: state.error, retry };
}

/** The MANA guidance's after-switch note for an offer's (re)quote. */
export function offersRevalidation(status: OffersStatus, retry: () => void): GuidanceRevalidation | null {
  switch (status) {
    case "loading":
      return { status: "checking", onRetry: retry };
    case "ready":
      return { status: "current", onRetry: retry };
    case "failed":
    case "routeMissing":
      return { status: "failed", onRetry: retry };
    default:
      return null;
  }
}
