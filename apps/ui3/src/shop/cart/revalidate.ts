// from: shop@94584a5c:app/src/lib/cart-availability.ts, app/src/hooks/useCartAvailability.ts
// Live price/stock review before confirmation. Upstream judged availability from the live trade and
// stayed optimistic on errors; here every line is re-quoted by the server (the PurchaseOffer is the
// one authority for price, stock and payment eligibility) and compared with the offer the buyer was
// shown. Confirmation is allowed only when the review is `current`: a changed price, a sold or
// withdrawn listing, a method the rule no longer offers or a quote outage must be seen and accepted.
// A failed review blocks confirmation instead of assuming the old answer still holds.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { CartLine, PurchaseOffer } from "@shop/model/purchase";
import { offerQuote } from "@shop/model/purchase";
import type { MissingFact, PaymentMethod, Quote, Session } from "@shop/model/settlement";
import { presentPayment } from "@shop/payment/present";
import { lineUnits } from "@shop/cart/partition";

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

/** Each line's changes from the offer it holds as seen to `offers` (a line with no offer there is unavailable). */
export function diffOffers(lines: readonly CartLine[], offers: Record<string, PurchaseOffer | null>, session: Session): LineChange[] {
  return lines.flatMap((line) => diffLine(line, offers[line.id] ?? null, session));
}

export async function revalidateCart(
  lines: CartLine[],
  session: Session,
  fetchOffers: OfferFetcher,
  signal: AbortSignal,
): Promise<RevalidationResult> {
  const answered = await fetchOffers(lines, signal);
  const offers: Record<string, PurchaseOffer | null> = {};
  for (const line of lines) {
    offers[line.id] = Object.prototype.hasOwnProperty.call(answered, line.id) ? (answered[line.id] ?? null) : null;
  }
  return { offers, changes: diffOffers(lines, offers, session) };
}

export type RevalidationStatus = "idle" | "checking" | "current" | "changed" | "failed";

/** How old an offer the buyer was shown can be and still stand on screen while the server prices the line again. */
export const SEEN_RECENT_MS = 20_000;

/** The line holds an offer the buyer was shown moments ago (the page's), so its payment options can show before the check ends. */
export function seenRecently(line: CartLine, now: number = Date.now()): boolean {
  return !!line.seen && Math.abs(now - line.seen.quotedAt) <= SEEN_RECENT_MS;
}

/** What a price check is for: each line, its quantity and its method. A check answers only the lines it was asked. */
export function revalidationIdentity(lines: readonly CartLine[]): string {
  return lines.map((l) => `${l.id}x${l.quantity}:${l.method ?? "-"}`).join(",");
}

export type CartRevalidation = {
  status: RevalidationStatus;
  /**
   * While `checking`: every line shows an offer the buyer saw moments ago, under the session and revision the hook
   * started with. It can stand on screen until the check ends; confirmation still waits for the check.
   */
  seenRecently: boolean;
  offers: Record<string, PurchaseOffer | null>;
  changes: LineChange[];
  error: unknown;
  /** The lines (and their quantities and methods) the last finished check priced, as `revalidationIdentity`. */
  pricedFor: string | null;
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
  // `firstCheck`: the running check is the first under the session and revision the seen offers were quoted under, so
  // the offers the lines hold may stand on screen until it ends (`seenRecently` below).
  const [state, setState] = useState<{ status: RevalidationStatus; firstCheck: boolean; result: RevalidationResult; error: unknown; pricedFor: string | null }>({
    status: "idle",
    firstCheck: false,
    result: { offers: {}, changes: [] },
    error: null,
    pricedFor: null,
  });
  const [tick, setTick] = useState(0);
  const latest = useRef({ lines, session, fetchOffers, onSeen: opts.onSeen });
  latest.current = { lines, session, fetchOffers, onSeen: opts.onSeen };

  const identity = useMemo(() => revalidationIdentity(lines), [lines]);
  const sessionKey = session.kind === "signedOut" ? "out" : `${session.kind}:${session.kind === "web2" ? session.account : session.wallet}`;
  // The session and revision the seen offers were quoted under: a switch, or a re-check the buyer asked for, waits for the server.
  const firstAsked = useRef(`${sessionKey}|${opts.revision ?? ""}`);

  useEffect(() => {
    if (!enabled || latest.current.lines.length === 0) {
      setState({ status: "idle", firstCheck: false, result: { offers: {}, changes: [] }, error: null, pricedFor: null });
      return;
    }
    const controller = new AbortController();
    const { lines: current, session: who, fetchOffers: fetcher } = latest.current;
    const firstCheck = tick === 0 && firstAsked.current === `${sessionKey}|${opts.revision ?? ""}`;
    setState((s) => ({ ...s, status: "checking", firstCheck, error: null }));
    const asked = revalidationIdentity(current);
    revalidateCart(current, who, fetcher, controller.signal).then(
      (answer) => {
        if (controller.signal.aborted) return;
        // The new offers are compared with the ones on screen now, not with those the check started from: a line handed
        // a fresher offer meanwhile (Buy now holds its line in an effect that runs after this one) showed that offer.
        // The lines, quantities and methods are the same, or this check would have been aborted for a new one.
        const shown = new Map(latest.current.lines.map((l) => [l.id, l]));
        const result = { offers: answer.offers, changes: diffOffers(current.map((l) => shown.get(l.id) ?? l), answer.offers, who) };
        setState({ status: result.changes.length === 0 ? "current" : "changed", firstCheck: false, result, error: null, pricedFor: asked });
      },
      (error: unknown) => {
        if (controller.signal.aborted) return;
        setState((s) => ({ ...s, status: "failed", firstCheck: false, error }));
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

  // Read from the lines as they are now, not as the check found them: a line the page hands its fresh offer after the
  // check started (Buy now holds its line in its own effect, which runs after this hook's) still counts.
  const recent = state.status === "checking" && state.firstCheck && lines.length > 0 && lines.every((l) => seenRecently(l));
  return {
    status: state.status,
    seenRecently: recent,
    offers: state.result.offers,
    changes: state.result.changes,
    error: state.error,
    pricedFor: state.pricedFor,
    revalidate,
    accept,
    canConfirm: state.status === "current",
  };
}
