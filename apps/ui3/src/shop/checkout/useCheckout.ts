// from: shop@94584a5c:app/src/pages/Cart.tsx (checkout, confirmPurchase, chargeOrTopUp, the sign-in and
//       top-up resumes), app/src/hooks/useCartAvailability.ts
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { partitionCart, type CartGroup, type CartPartition, type LineRefusal } from "../cart/partition";
import { useCartRevalidation, type LineChange, type OfferFetcher } from "../cart/revalidate";
import { checkoutSummary, type CheckoutSummary } from "../cart/machine";
import { useCart } from "../cart/store";
import type { CartLine, PurchaseOffer, PurchaseState } from "../model/purchase";
import { PAYMENT_METHODS, type Eligibility, type PaymentMethod, type Session } from "../model/settlement";
import { outageText } from "../payment/copy";
import { presentPayment, presentUnpriced, type PaymentPresentation } from "../payment/present";
import { ruleFor } from "../payment/rule";
import { assetKindOf } from "../discovery/cardPrice";
import { useShopWallet, type ShopWalletState } from "../wallet/context";
import { cancelUsdIntents, type UsdBalance } from "../credits/credits";
import { useBalance, useManaBalance } from "../credits/hooks";
import { stashTopUpReturn } from "../credits/returns";
import { settlementChainId } from "./chain/config";
import { stashResumeIntent, takeResumeIntent } from "./lib/auth-return";
import { invalidateAfterPurchase } from "./lib/after-purchase";
import { setAuthorization, type ShopAuthorization } from "./lib/authorizations";
import { RESUME_CART_KEY, reviewCart, type CartReview, type ResolvedLine } from "./purchase/cart";
import { releaseAuthorizing, resumeCheckout, runGroup, watchGroup, type ApprovalRequest, type CheckoutDeps, type GroupResult } from "./purchase/executor";
import type { RailContext } from "./purchase/rail";
import { createSpendGuard } from "./purchase/spend-guard";
import { useShopServices } from "./services";
import { cartOfferFetcher, useOfferSource } from "../detail/offers";
import type { ManaWei, UsdCents } from "../model/money";
import { creditsCeil } from "../model/money";

export type ReviewState =
  | { kind: "empty" }
  | { kind: "outage"; reason: "no-quote-route" | "failed" }
  | { kind: "checking" }
  | { kind: "changed"; changes: LineChange[] }
  | { kind: "current" };

export type PurchasedLine = {
  lineId: string;
  name: string;
  thumbnail?: string;
  href?: string;
  quantity: number;
  method: PaymentMethod;
  keyId: string;
  creator?: string;
  category?: string;
  urn?: string;
  contractAddress?: string;
  itemId?: string;
  credits?: number;
};

export type PendingApproval = { request: ApprovalRequest; grant(): Promise<void>; resolve(granted: boolean): void };

export type CheckoutView = {
  wallet: ShopWalletState;
  session: Session;
  account: string | null;
  lines: CartLine[];
  offers: Record<string, PurchaseOffer | null>;
  views: Record<string, PaymentPresentation | null>;
  partition: CartPartition | null;
  refusedByLine: Record<string, LineRefusal>;
  review: ReviewState;
  changesByLine: Record<string, LineChange[]>;
  balance: UsdBalance | undefined;
  balanceError: boolean;
  manaBalance: bigint | null;
  checkout: PurchaseState | null;
  summary: CheckoutSummary | null;
  results: Record<string, GroupResult>;
  settlementIssues: Pick<CartReview, "unavailable" | "own" | "stale" | "unsupported" | "noPrice"> | null;
  topUp: { shortCents: UsdCents } | null;
  approval: PendingApproval | null;
  running: boolean;
  reviewChanges: LineChange[];
  quoteOutage: PaymentMethod[];
  blockers: string[];
  canConfirm: boolean;
  canResume: boolean;
  walletNeededKeys: string[];
  setMethod(lineId: string, method: PaymentMethod): void;
  setQuantity(lineId: string, quantity: number): void;
  remove(lineId: string): void;
  clear(): void;
  revalidate(): void;
  acceptChanges(): void;
  confirm(): Promise<void>;
  resume(): Promise<void>;
  discardCheckout(): Promise<void>;
  dismissTopUp(): void;
  stashForTopUp(): void;
};

const NO_OFFERS: OfferFetcher = async () => ({});

const IN_FLIGHT = "A purchase from this cart is still being confirmed.";
const RUNNING = "Purchase in progress…";

export function lineRule(line: CartLine, session: Session): Eligibility {
  const kind = line.listing.rentalId ? "rental" : assetKindOf(line.display.category ?? "");
  const sale = line.asset.token.kind === "item" ? "primaryMint" : kind === "name" && line.listing.name ? "primaryNameRegistration" : "secondary";
  return ruleFor({ asset: line.asset, kind, sale, bundle: false, settlement: [] }, session);
}

function inProgress(state: PurchaseState | null): boolean {
  return !!state && state.groups.some((g) => g.status !== "confirmed");
}

export function useCheckout(opts: { onComplete?(purchased: PurchasedLine[], results: Record<string, GroupResult>, groups: CartGroup[]): void; autoConfirm?: boolean } = {}): CheckoutView {
  const services = useShopServices();
  const wallet = useShopWallet();
  const qc = useQueryClient();
  const { session, account } = wallet;
  const snapshot = useCart((s) => s, services.cart);
  const lines = snapshot.lines;
  const checkout = snapshot.checkout;

  const balanceQuery = useBalance(account);
  const manaQuery = useManaBalance(session.kind === "web3" ? account : null, settlementChainId());
  const balance = balanceQuery.data;
  const manaBalance = manaQuery.data ?? null;

  const source = useOfferSource();
  const fetchOffers = useMemo(() => services.fetchOffers === undefined ? cartOfferFetcher(source, session) : services.fetchOffers, [services.fetchOffers, source, session]);
  const revalidation = useCartRevalidation(lines, session, fetchOffers ?? NO_OFFERS, {
    enabled: !!fetchOffers && lines.length > 0,
    revision: wallet.revision,
    onSeen: (lineId, offer) => services.cart.markSeen(lineId, offer),
  });

  const offers = useMemo(() => {
    const out: Record<string, PurchaseOffer | null> = {};
    for (const line of lines) {
      const fresh = Object.prototype.hasOwnProperty.call(revalidation.offers, line.id) ? revalidation.offers[line.id] : undefined;
      out[line.id] = fresh !== undefined ? fresh : line.seen;
    }
    return out;
  }, [lines, revalidation.offers]);

  const unpriced = !fetchOffers || revalidation.status === "failed";
  const views = useMemo(() => {
    const out: Record<string, PaymentPresentation | null> = {};
    for (const line of lines) {
      const offer = offers[line.id];
      out[line.id] = offer
        ? presentPayment(offer, session, { creditsBalance: balance?.balanceCents ?? null, manaBalance: manaBalance as ManaWei | null, report: () => {} })
        : unpriced
          ? presentUnpriced(lineRule(line, session))
          : null;
    }
    return out;
  }, [lines, offers, session, balance?.balanceCents, manaBalance, unpriced]);

  const partition = useMemo(() => {
    if (lines.length === 0) return null;
    return partitionCart(
      lines.map((line) => ({ line, offer: offers[line.id] ?? null })),
      session,
      { creditsBalance: balance?.balanceCents ?? null, manaBalance: manaBalance as ManaWei | null, report: () => {} },
    );
  }, [lines, offers, session, balance?.balanceCents, manaBalance]);

  const refusedByLine = useMemo(() => Object.fromEntries((partition?.refused ?? []).map((r) => [r.lineId, r])), [partition]);

  const review: ReviewState = useMemo(() => {
    if (lines.length === 0) return { kind: "empty" };
    if (!fetchOffers) return { kind: "outage", reason: "no-quote-route" };
    switch (revalidation.status) {
      case "idle":
      case "checking":
        return { kind: "checking" };
      case "failed":
        return { kind: "outage", reason: "failed" };
      case "changed":
        return { kind: "changed", changes: revalidation.changes };
      case "current":
        return { kind: "current" };
    }
  }, [lines.length, fetchOffers, revalidation.status, revalidation.changes]);

  const reviewChanges = useMemo(() => revalidation.changes.filter((c) => c.kind !== "quoteUnavailable"), [revalidation.changes]);
  const quoteOutage = useMemo(() => {
    const methods = new Set<PaymentMethod>();
    for (const c of revalidation.changes) if (c.kind === "quoteUnavailable") methods.add(c.method);
    for (const r of partition?.refused ?? []) if (r.code === "quoteUnavailable") methods.add(r.method ?? "credits");
    return PAYMENT_METHODS.filter((m) => methods.has(m));
  }, [revalidation.changes, partition]);

  const changesByLine = useMemo(() => {
    const out: Record<string, LineChange[]> = {};
    for (const c of revalidation.changes) (out[c.lineId] ??= []).push(c);
    return out;
  }, [revalidation.changes]);

  const [results, setResults] = useState<Record<string, GroupResult>>({});
  const [running, setRunning] = useState(false);
  const [topUp, setTopUp] = useState<{ shortCents: UsdCents } | null>(null);
  const [approval, setApproval] = useState<PendingApproval | null>(null);
  const [settlementIssues, setSettlementIssues] = useState<CheckoutView["settlementIssues"]>(null);
  const guard = useRef(createSpendGuard());

  const provider = useMemo(() => wallet.host?.provider?.() ?? null, [wallet.host]);
  const walletKind = wallet.host?.walletKind ?? null;

  const walletNeededKeys = useMemo(
    () => (session.kind === "web3" ? [] : (partition?.groups ?? []).filter((g) => g.key.method === "mana").map((g) => g.keyId)),
    [partition, session.kind],
  );

  const blockers = useMemo(() => {
    const out: string[] = [];
    if (lines.length === 0) return out;
    if (session.kind === "signedOut") out.push("Sign in to check out.");
    if (wallet.accountMismatch) out.push("Your wallet switched to another account. Sign in again with it, or switch your wallet back.");
    if (review.kind === "outage") out.push("Prices can't be checked right now, so nothing can be bought. Try again in a moment.");
    if (review.kind === "checking") out.push("Checking prices and availability…");
    if (reviewChanges.some((c) => c.kind === "priceChanged")) out.push("Prices changed since you added these items. Accept the new prices to continue.");
    if (reviewChanges.some((c) => c.kind !== "priceChanged")) out.push("Some items changed since you added them. See the note on each item.");
    for (const method of quoteOutage) out.push(outageText(method, null));
    const refused = partition?.refused.filter((r) => r.code !== "quoteUnavailable" && r.code !== "offerUnavailable") ?? [];
    if (refused.some((r) => r.code === "methodNotAllowed" && r.allowed.length > 0)) out.push("Some items can't be paid the way they're set. Choose how to pay for them.");
    if (refused.some((r) => r.code !== "methodNotAllowed" || r.allowed.length === 0)) out.push("Some items can't be bought as they are. See the note on each item.");
    if (partition?.creditsShort) out.push("Your Credits balance doesn't cover the Credits part of this cart.");
    if (session.kind !== "signedOut" && !provider) {
      out.push(session.kind === "web3" ? "Connect your wallet again to sign the purchase." : "Your sign-in can't confirm purchases right now. Sign in again to continue.");
    }
    if (inProgress(checkout) && checkout!.groups.some((g) => g.status === "submitted" || g.status === "pending")) {
      out.push(IN_FLIGHT);
    }
    if (running) out.push(RUNNING);
    return out;
  }, [lines.length, session.kind, wallet.accountMismatch, review.kind, reviewChanges, quoteOutage, partition, provider, checkout, running]);

  const payableGroups = useMemo(() => (partition?.groups ?? []).filter((g) => !walletNeededKeys.includes(g.keyId)), [partition, walletNeededKeys]);
  const canConfirm = blockers.length === 0 && payableGroups.length > 0;
  const canResume =
    review.kind === "current" && !wallet.accountMismatch && blockers.every((b) => b === IN_FLIGHT || b === RUNNING) && payableGroups.length > 0;

  const railContext = useCallback((): RailContext | null => {
    if (!account || !provider) return null;
    return {
      buyer: account,
      provider,
      reader: services.reader,
      client: services.client,
      walletKind,
      relayerUrl: services.relayerUrl,
      fetchImpl: services.fetchImpl,
      gasless: services.gasless,
      wait: services.wait,
    };
  }, [account, provider, services, walletKind]);

  const requestApproval = useCallback(
    (request: ApprovalRequest) =>
      new Promise<boolean>((resolve) => {
        const ctx = railContext();
        setApproval({
          request,
          grant: async () => {
            if (!ctx) throw new Error("Connect your wallet again to approve.");
            await setAuthorization({
              auth: request.auth as ShopAuthorization,
              active: true,
              provider: ctx.provider,
              account: ctx.buyer,
              walletKind: ctx.walletKind,
              reader: ctx.reader,
              relayerUrl: ctx.relayerUrl,
              fetchImpl: ctx.fetchImpl,
            });
          },
          resolve: (granted) => {
            setApproval(null);
            resolve(granted);
          },
        });
      }),
    [railContext],
  );

  const deps = useCallback((): CheckoutDeps | null => {
    const ctx = railContext();
    return ctx ? { ctx, store: services.cart, requestApproval, guard: guard.current } : null;
  }, [railContext, services.cart, requestApproval]);

  const settle = useCallback(
    async (groups: CartGroup[]): Promise<Map<string, ResolvedLine> | null> => {
      const ids = new Set(groups.flatMap((g) => g.lineIds));
      const priced = lines.filter((l) => ids.has(l.id)).map((line) => ({ line, offer: offers[line.id] ?? null }));
      const res = await reviewCart(priced, session, account!, services.resolvers);
      const issues = { unavailable: res.unavailable, own: res.own, stale: res.stale, unsupported: res.unsupported, noPrice: res.noPrice };
      setSettlementIssues(Object.values(issues).some((v) => v.length > 0) ? issues : null);
      if (res.stale.length > 0 || res.unavailable.length > 0) revalidation.revalidate();
      return new Map(res.buyable.map((r) => [r.line.id, r]));
    },
    [lines, offers, session, account, services.resolvers, revalidation],
  );

  const finish = useCallback(
    (groups: CartGroup[], next: Record<string, GroupResult>, resolved: Map<string, ResolvedLine>) => {
      const confirmed = groups.filter((g) => next[g.keyId]?.status === "confirmed");
      if (confirmed.length > 0) invalidateAfterPurchase(qc);
      const purchased: PurchasedLine[] = confirmed.flatMap((g) =>
        g.lineIds.map((id, i) => {
          const r = resolved.get(id);
          const line = r?.line ?? lines.find((l) => l.id === id)!;
          const quantity = r?.quantity ?? line.quantity;
          const quote = r?.quote ?? g.quotes[i];
          const credits = g.key.method === "credits" && quote?.method === "credits" ? creditsCeil(quote.quoted) * quantity : undefined;
          const { creator, category, urn } = line.display;
          const itemId = line.asset.token.kind === "item" ? line.asset.token.itemId : undefined;
          return { lineId: id, name: line.display.name, thumbnail: line.display.thumbnail, href: line.display.href, quantity, method: g.key.method, keyId: g.keyId, creator, category, urn, contractAddress: line.asset.contract, itemId, credits };
        }),
      );
      services.cart.removeLines(purchased.map((p) => p.lineId));
      const state = services.cart.getSnapshot().checkout;
      const allDone = !!state && state.groups.every((g) => g.status === "confirmed");
      if (allDone) {
        services.cart.replaceCheckout(null);
        opts.onComplete?.(purchased, next, groups);
      }
    },
    [qc, lines, services.cart, opts],
  );

  const confirm = useCallback(async () => {
    if (!account) {
      stashResumeIntent({ type: "cart-checkout" });
      wallet.host?.onSignIn?.();
      return;
    }
    if (!canConfirm || !partition) return;
    const d = deps();
    if (!d) return;
    setRunning(true);
    setTopUp(null);
    try {
      const resolved = await settle(payableGroups);
      if (!resolved) return;
      const runnable = payableGroups.filter((g) => g.lineIds.every((id) => resolved.has(id)));
      if (runnable.length === 0) return;
      let existing = services.cart.getSnapshot().checkout;
      if (existing && inProgress(existing)) existing = await releaseAuthorizing(d, existing);
      if (!existing || !inProgress(existing)) services.cart.startCheckout(runnable);
      else {
        const keep = existing.groups.filter((g) => g.status === "confirmed" || g.status === "submitted" || g.status === "pending");
        const fresh = runnable.filter((g) => !keep.some((k) => k.keyId === g.keyId));
        const started = services.cart.startCheckout(fresh);
        services.cart.replaceCheckout({ ...started, groups: [...keep, ...started.groups] });
      }
      const next: Record<string, GroupResult> = {};
      for (const group of runnable) {
        const result = await runGroup(d, group, resolved);
        next[group.keyId] = result;
        setResults((prev) => ({ ...prev, [group.keyId]: result }));
        if (result.status === "failed" && result.topUp) {
          const short = Math.max(0, result.topUp.requiredCents - result.topUp.balanceCents);
          setTopUp({ shortCents: short as UsdCents });
        }
      }
      finish(runnable, next, resolved);
    } finally {
      setRunning(false);
    }
  }, [account, canConfirm, partition, deps, settle, payableGroups, services.cart, finish, wallet.host]);

  const resume = useCallback(async () => {
    const state = services.cart.getSnapshot().checkout;
    const d = deps();
    if (!state || !d) return;
    setRunning(true);
    try {
      if (!canResume) {
        const out: Record<string, GroupResult> = {};
        for (const g of state.groups) if (g.status === "submitted" || g.status === "pending") out[g.keyId] = await watchGroup(d, g);
        setResults((prev) => ({ ...prev, ...out }));
        finish(partition?.groups ?? [], out, new Map());
        return;
      }
      const unfinished = state.groups.filter((g) => g.status !== "confirmed" && g.status !== "submitted" && g.status !== "pending");
      const groups = (partition?.groups ?? []).filter((g) => unfinished.some((u) => u.keyId === g.keyId));
      const resolved = groups.length ? ((await settle(groups)) ?? new Map<string, ResolvedLine>()) : new Map<string, ResolvedLine>();
      const out = await resumeCheckout(d, state, groups, resolved);
      setResults((prev) => ({ ...prev, ...out }));
      finish(partition?.groups ?? [], out, resolved);
    } finally {
      setRunning(false);
    }
  }, [services.cart, deps, partition, settle, finish, canResume]);

  const discardCheckout = useCallback(async () => {
    const state = services.cart.getSnapshot().checkout;
    if (!state || state.groups.some((g) => g.status === "submitted" || g.status === "pending")) return;
    const salts = state.groups.filter((g) => g.status === "authorizing").flatMap((g) => g.creditIds);
    if (salts.length) await cancelUsdIntents(salts, services.client).catch(() => 0);
    services.cart.replaceCheckout(null);
    setResults({});
  }, [services.cart, services.client]);

  const watched = useRef(new Set<string>());
  useEffect(() => {
    const d = deps();
    if (!checkout || !d) return;
    for (const g of checkout.groups) {
      if ((g.status !== "submitted" && g.status !== "pending") || watched.current.has(`${g.keyId}:${g.attempts}`)) continue;
      watched.current.add(`${g.keyId}:${g.attempts}`);
      void watchGroup(d, g).then((r) => {
        setResults((prev) => ({ ...prev, [g.keyId]: r }));
        if (r.status === "confirmed") {
          services.cart.removeLines(g.lineIds);
          invalidateAfterPurchase(qc);
        }
      });
    }
  }, [checkout, deps, services.cart, qc]);

  const [autoConfirm, setAutoConfirm] = useState(!!opts.autoConfirm);
  useEffect(() => {
    if (!account) return;
    let flagged = false;
    try {
      flagged = sessionStorage.getItem(RESUME_CART_KEY) !== null;
      if (flagged) sessionStorage.removeItem(RESUME_CART_KEY);
    } catch {
      flagged = false;
    }
    if (takeResumeIntent("cart-checkout") || flagged) setAutoConfirm(true);
  }, [account]);
  useEffect(() => {
    if (autoConfirm && canConfirm && !running) {
      setAutoConfirm(false);
      void confirm();
    }
  }, [autoConfirm, canConfirm, running, confirm]);

  return {
    wallet,
    session,
    account,
    lines,
    offers,
    views,
    partition,
    refusedByLine,
    review,
    changesByLine,
    reviewChanges,
    quoteOutage,
    balance,
    balanceError: balanceQuery.isError,
    manaBalance,
    checkout,
    summary: checkout ? checkoutSummary(checkout) : null,
    results,
    settlementIssues,
    topUp: topUp ?? (partition?.creditsShort ? { shortCents: partition.creditsShort } : null),
    approval,
    running,
    blockers,
    canConfirm,
    canResume,
    walletNeededKeys,
    setMethod: (lineId, method) => services.cart.setMethod(lineId, method),
    setQuantity: (lineId, quantity) => {
      const available = offers[lineId]?.available ?? null;
      services.cart.setQuantity(lineId, quantity, available);
    },
    remove: (lineId) => services.cart.remove(lineId),
    clear: () => services.cart.clear(),
    revalidate: revalidation.revalidate,
    acceptChanges: revalidation.accept,
    confirm,
    resume,
    discardCheckout,
    dismissTopUp: () => setTopUp(null),
    stashForTopUp: () => {
      stashTopUpReturn("/shop/cart");
      try {
        sessionStorage.setItem(RESUME_CART_KEY, "1");
      } catch {
      }
    },
  };
}
