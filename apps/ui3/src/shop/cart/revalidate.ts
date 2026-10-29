// from: shop@94584a5c:app/src/lib/cart-availability.ts, app/src/hooks/useCartAvailability.ts
// Live price/stock review before confirmation. Upstream judged availability from the live trade and
// stayed optimistic on errors; here every line is re-quoted by the server (the PurchaseOffer is the
// one authority for price, stock and payment eligibility) and compared with the offer the buyer was
// shown. Confirmation is allowed only when the review is `current`: a changed price, a sold or
// withdrawn listing, a method the rule no longer offers or a quote outage must be seen and accepted.
// A failed review blocks confirmation instead of assuming the old answer still holds.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { CartLine, PurchaseOffer } from "../model/purchase";
import { offerQuote } from "../model/purchase";
import type { MissingFact, PaymentMethod, Quote, Session } from "../model/settlement";
import { presentPayment } from "../payment/present";
import { lineUnits } from "./partition";

export type LineChange =
  | { lineId: string; kind: "unavailable" }
  | { lineId: string; kind: "priceChanged"; method: PaymentMethod; before: Quote; after: Quote }
  | { lineId: string; kind: "quoteUnavailable"; method: PaymentMethod; missing: MissingFact | null }
  | { lineId: string; kind: "methodWithdrawn"; method: PaymentMethod; allowed: PaymentMethod[] }
  | { lineId: string; kind: "insufficientStock"; available: number; wanted: number };

function sameQuote(a: Quote, b: Quote): boolean {
  if (a.method === "credits" && b.method === "credits") return a.quoted === b.quoted;
  if (a.method === "mana" && b.method === "mana") return a.amount === b.amount;
  return false;
}

/** What changed for one line between the offer the buyer saw and the server's answer now. */
export function diffLine(line: CartLine, next: PurchaseOffer | null, session: Session): LineChange[] {
  if (!next) return [{ lineId: line.id, kind: "unavailable" }];
  const changes: LineChange[] = [];
  const view = presentPayment(next, session, { report: () => {} });
  const method = line.method ?? view.primary;
  const option = view.options.find((o) => o.method === method);
  if (!option) {
    changes.push({ lineId: line.id, kind: "methodWithdrawn", method, allowed: view.options.map((o) => o.method) });
  } else if (!option.quote) {
    changes.push({
      lineId: line.id,
      kind: "quoteUnavailable",
      method,
      missing: option.state.kind === "unavailable" ? option.state.missing : null,
    });
  } else {
    const before = line.seen ? offerQuote(line.seen, method) : null;
    if (before && !sameQuote(before, option.quote)) {
      changes.push({ lineId: line.id, kind: "priceChanged", method, before, after: option.quote });
    }
  }
  const wanted = lineUnits(line);
  if (next.available !== null && wanted > next.available) {
    changes.push({ lineId: line.id, kind: "insufficientStock", available: next.available, wanted });
  }
  return changes;
}

export type OfferFetcher = (lines: CartLine[], signal: AbortSignal) => Promise<Record<string, PurchaseOffer | null>>;

export type RevalidationResult = { offers: Record<string, PurchaseOffer | null>; changes: LineChange[] };

export async function revalidateCart(
  lines: CartLine[],
  session: Session,
  fetchOffers: OfferFetcher,
  signal: AbortSignal,
): Promise<RevalidationResult> {
  const answered = await fetchOffers(lines, signal);
  const offers: Record<string, PurchaseOffer | null> = {};
  const changes: LineChange[] = [];
  for (const line of lines) {
    const offer = Object.prototype.hasOwnProperty.call(answered, line.id) ? (answered[line.id] ?? null) : null;
    offers[line.id] = offer;
    changes.push(...diffLine(line, offer, session));
  }
  return { offers, changes };
}

export type RevalidationStatus = "idle" | "checking" | "current" | "changed" | "failed";

export type CartRevalidation = {
  status: RevalidationStatus;
  offers: Record<string, PurchaseOffer | null>;
  changes: LineChange[];
  error: unknown;
  /** Re-quote every line now (after an account or network switch, or on the buyer's request). */
  revalidate(): void;
  /** The buyer saw the changes: record the new offers as seen, so the review turns current. */
  accept(): void;
  canConfirm: boolean;
};

/**
 * Re-quotes the cart while `enabled` (the review step is open) and whenever the lines, the session or
 * `revision` change: pass the wallet's account and chain as `revision` so a switch revalidates.
 */
export function useCartRevalidation(
  lines: CartLine[],
  session: Session,
  fetchOffers: OfferFetcher,
  opts: { enabled?: boolean; revision?: string | number; onSeen?: (lineId: string, offer: PurchaseOffer | null) => void } = {},
): CartRevalidation {
  const enabled = opts.enabled ?? true;
  const [state, setState] = useState<{ status: RevalidationStatus; result: RevalidationResult; error: unknown }>({
    status: "idle",
    result: { offers: {}, changes: [] },
    error: null,
  });
  const [tick, setTick] = useState(0);
  const latest = useRef({ lines, session, fetchOffers, onSeen: opts.onSeen });
  latest.current = { lines, session, fetchOffers, onSeen: opts.onSeen };

  const identity = useMemo(
    () => lines.map((l) => `${l.id}x${l.quantity}:${l.method ?? "-"}`).join(","),
    [lines],
  );
  const sessionKey = session.kind === "signedOut" ? "out" : `${session.kind}:${session.kind === "web2" ? session.account : session.wallet}`;

  useEffect(() => {
    if (!enabled || latest.current.lines.length === 0) {
      setState({ status: "idle", result: { offers: {}, changes: [] }, error: null });
      return;
    }
    const controller = new AbortController();
    setState((s) => ({ ...s, status: "checking", error: null }));
    const { lines: current, session: who, fetchOffers: fetcher } = latest.current;
    revalidateCart(current, who, fetcher, controller.signal).then(
      (result) => {
        if (controller.signal.aborted) return;
        setState({ status: result.changes.length === 0 ? "current" : "changed", result, error: null });
      },
      (error: unknown) => {
        if (controller.signal.aborted) return;
        setState((s) => ({ ...s, status: "failed", error }));
      },
    );
    return () => controller.abort();
  }, [enabled, identity, sessionKey, opts.revision, tick]);

  const revalidate = useCallback(() => setTick((n) => n + 1), []);
  const seenState = useRef(state);
  seenState.current = state;
  const accept = useCallback(() => {
    const s = seenState.current;
    if (s.status !== "changed" && s.status !== "current") return;
    for (const [lineId, offer] of Object.entries(s.result.offers)) latest.current.onSeen?.(lineId, offer);
    const blocking = s.result.changes.filter((c) => c.kind !== "priceChanged");
    setState({ ...s, status: blocking.length === 0 ? "current" : "changed", result: { ...s.result, changes: blocking } });
  }, []);

  return {
    status: state.status,
    offers: state.result.offers,
    changes: state.result.changes,
    error: state.error,
    revalidate,
    accept,
    canConfirm: state.status === "current",
  };
}
