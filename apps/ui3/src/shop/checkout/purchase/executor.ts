// from: shop@94584a5c:app/src/pages/Cart.tsx (charge, chargeWithMana, withManaApprovals, the failure
//       accounting around partitionReservations), app/src/lib/buy.ts, app/src/lib/buy-gasless.ts
// Runs ONE confirmation group of a checkout through the foundation's per-group state machine
// (cart/machine: idle -> authorizing -> submitted -> pending -> confirmed | failed | expired), so every
// group completes, fails or stays pending on its own and a resume re-runs only the unfinished ones.
//
//  - A credits group authorizes ONE credit for the whole group (POST credit-authorizations/batch, with
//    the server's quoted cents per unit), then spends it in one useCredits: relayed by default, the
//    buyer's own transaction only for a self-custody wallet when the relay cannot run. A 402 is a
//    top-up, reported as such; nothing ever moves the group to MANA.
//  - A MANA group checks the MANA allowance its spender needs (a self-custody wallet is asked to
//    approve first, a managed wallet is never shown approval wording), then accepts or mints.
//  - `submit` is dispatched at the hand-off (the relayed signature, or right before the wallet is asked
//    to send). From then on the group is only watched; its credits are released only when nothing was
//    handed off, or its transaction mined and reverted. A relay that gave no usable answer, or a hash
//    with no receipt yet, leaves the group pending with its reservation kept.

import type { Hex } from "viem";

import { insufficientCredits, isShopApiError, creditsDisabled, tooManyLiveCredits } from "../../api/client";
import type { CreditBatchAuthorization } from "../../api/types";
import { prepareResume, resumePlan, type GroupEvent } from "../../cart/machine";
import type { CartGroup } from "../../cart/partition";
import type { CartStore } from "../../cart/store";
import type { GroupFailure, GroupProgress, PurchaseState } from "../../model/purchase";
import { authorizeUsdCreditGroup, cancelUsdIntents, fetchUserPurchases } from "../../credits/credits";
import { gaslessEnabled } from "../chain/config";
import { isGaslessUnavailable, isSignatureDeclined } from "../chain/metatx";
import { isPendingTxError, isRevertedTxError } from "../chain/wallet";
import { AuthorizationKind, getAuthorizationStatus, needsApprovalStep, ensureAuthorization, type ShopAuthorization } from "../lib/authorizations";
import { friendlyError, isRejection } from "../lib/errors";
import { isWrongNetworkError } from "../lib/network";
import { canPayGasItself } from "../lib/wallet-kind";
import { requireContracts } from "../chain/contracts";
import { checkoutLineFor, purchaseTargetFor, toUnits, type ResolvedLine } from "./cart";
import { buyManyWithCredits, groupPurchases, type AnyPurchase } from "./credits";
import { buyManyGasless, isSettlementPending, waitForSettlement } from "./gasless";
import { acceptPayingMana, manaSpenderFor, mintPayingMana, purchaseFor, type MintToBuy } from "./mana";
import type { RailContext } from "./rail";
import { createSpendGuard, type SpendGuard } from "./spend-guard";
import { isMetaTxPending, isMetaTxReverted } from "./tx-confirm";

export type ApprovalRequest = { auth: ShopAuthorization; index: number; total: number; requiredWei: bigint; keyId: string };

export type CheckoutDeps = {
  ctx: RailContext;
  store: CartStore;
  /** A self-custody wallet approves MANA spending explicitly; resolves true once the approval is granted. */
  requestApproval?: (request: ApprovalRequest) => Promise<boolean>;
  guard?: SpendGuard;
};

export type TopUpNeeded = { requiredCents: number; balanceCents: number };

export type GroupResult =
  | { status: "confirmed"; txHash: string | null }
  | { status: "pending"; txHash: string | null }
  | { status: "failed"; failure: GroupFailure; topUp?: TopUpNeeded; released: string[] }
  | { status: "skipped"; reason: string };

const FALLBACK = "We couldn't complete this part of your purchase. Nothing was charged for it.";

function dispatchSafe(store: CartStore, keyId: string, event: GroupEvent): void {
  try {
    store.dispatch(keyId, event);
  } catch (e) {
    console.warn("[shop/checkout] ignored an out-of-order group event", keyId, event.type, e);
  }
}

function progressOf(store: CartStore, keyId: string): GroupProgress | null {
  return store.getSnapshot().checkout?.groups.find((g) => g.keyId === keyId) ?? null;
}

function failureFor(e: unknown, kind?: GroupFailure["kind"]): GroupFailure {
  if (kind) return { kind, message: friendlyError(e, FALLBACK, { sale: true }) };
  if (isRejection(e) || isSignatureDeclined(e)) return { kind: "rejected", message: friendlyError(e, FALLBACK) };
  if (isShopApiError(e)) return { kind: "refused", message: friendlyError(e, FALLBACK, { sale: true }) };
  return { kind: "error", message: friendlyError(e, FALLBACK, { sale: true }) };
}

async function release(deps: CheckoutDeps, salts: string[]): Promise<string[]> {
  const guard = deps.guard;
  const free = guard ? salts.filter((s) => !guard.mayBeConsumed(s) && !guard.isInFlight(s)) : salts;
  if (free.length === 0) return [];
  try {
    await cancelUsdIntents(free, deps.ctx.client);
    return free;
  } catch (e) {
    console.warn("[shop/checkout] could not release reserved credits; they return when they expire", e);
    return [];
  }
}

function linesOf(group: CartGroup, resolved: Map<string, ResolvedLine>): ResolvedLine[] | null {
  const lines = group.lineIds.map((id) => resolved.get(id));
  return lines.every((l): l is ResolvedLine => !!l) ? lines : null;
}

/** Credits group: one credit, one useCredits. */
async function runCreditsGroup(deps: CheckoutDeps, group: CartGroup, lines: ResolvedLine[]): Promise<GroupResult> {
  const { ctx, store } = deps;
  const guard = (deps.guard ??= createSpendGuard());
  const keyId = group.keyId;
  const units = toUnits(lines);
  let auth: CreditBatchAuthorization;
  try {
    auth = await authorizeUsdCreditGroup(units.map(checkoutLineFor), "website", ctx.client);
  } catch (e) {
    const short = insufficientCredits(e);
    const message = short
      ? "Your Credits balance does not cover this group. Top up to continue."
      : creditsDisabled(e)
        ? "Paying with Credits is unavailable right now. Try again in a moment."
        : tooManyLiveCredits(e)
          ? "You have too many purchases in progress. Complete or cancel one first."
          : undefined;
    const failure = message ? { kind: "refused" as const, message } : failureFor(e);
    dispatchSafe(store, keyId, { type: "failed", failure });
    return { status: "failed", failure, released: [], ...(short ? { topUp: { requiredCents: short.requiredCents, balanceCents: short.balanceCents } } : {}) };
  }
  const creditId = auth.credit.id;
  dispatchSafe(store, keyId, { type: "authorized", creditIds: [creditId] });
  const credit = { ...auth.credit };
  const purchases: AnyPurchase[] = units.map((u) => purchaseFor(purchaseTargetFor(u), [credit], auth.maxCreditedValue));
  if (groupPurchases(purchases).length !== 1) {
    const failure: GroupFailure = { kind: "error", message: "These items no longer settle together. Review your cart again." };
    dispatchSafe(store, keyId, { type: "failed", failure });
    return { status: "failed", failure, released: await release(deps, [creditId]) };
  }

  let handedOff = false;
  let txHash: Hex | null = null;
  const handoff = () => {
    if (handedOff) return;
    handedOff = true;
    guard.submitStarted(creditId);
    dispatchSafe(store, keyId, { type: "submit" });
  };
  const broadcast = (hash: Hex) => {
    txHash = hash;
    guard.broadcast(creditId, hash);
    dispatchSafe(store, keyId, { type: "hash", txHash: hash });
    dispatchSafe(store, keyId, { type: "observed" });
  };
  const chainId = group.key.chainId;
  try {
    let settled = false;
    if (ctx.gasless ?? gaslessEnabled()) {
      try {
        await buyManyGasless(ctx, { purchases, onSigned: handoff, onBroadcast: ({ txHash: h }) => broadcast(h) });
        await waitForSettlement(ctx.reader, chainId, txHash!, ctx.wait);
        settled = true;
      } catch (e) {
        const canFallBack = isGaslessUnavailable(e) && e.reason !== "relayer-unreachable" && canPayGasItself(ctx.walletKind) && txHash === null;
        if (!canFallBack) throw e;
        if (handedOff) {
          // The relay parsed our request and refused it: provably not broadcast. The direct rail is a new hand-off.
          handedOff = false;
          guard.submitFinished(creditId);
          dispatchSafe(store, keyId, { type: "failed", failure: { kind: "rejected", message: "The relay refused this purchase." } });
          dispatchSafe(store, keyId, { type: "authorize" });
          dispatchSafe(store, keyId, { type: "authorized", creditIds: [creditId] });
        }
      }
    }
    if (!settled) {
      await buyManyWithCredits(ctx, { purchases, onHandoff: handoff, onBroadcast: ({ txHash: h }) => broadcast(h) });
    }
    guard.submitFinished(creditId);
    dispatchSafe(store, keyId, { type: "confirmed", ...(txHash ? { txHash } : {}) });
    return { status: "confirmed", txHash };
  } catch (e) {
    guard.submitFinished(creditId);
    if (isRevertedTxError(e) && txHash) {
      guard.reverted(txHash);
      const failure: GroupFailure = { kind: "reverted", message: "The transaction failed on-chain and nothing was bought. The Credits held for it are released once the authorization expires and that is confirmed on chain." };
      dispatchSafe(store, keyId, { type: "failed", failure });
      return { status: "failed", failure, released: await release(deps, [creditId]) };
    }
    if (isSettlementPending(e) || isPendingTxError(e) || isMetaTxPending(e)) return { status: "pending", txHash };
    if (isGaslessUnavailable(e) && e.reason === "relayer-unreachable" && handedOff) {
      guard.unobservable(creditId);
      return { status: "pending", txHash: null };
    }
    if (txHash) return { status: "pending", txHash };
    const failure = handedOff ? { ...failureFor(e), kind: "rejected" as const } : failureFor(e);
    if (isWrongNetworkError(e)) failure.message = friendlyError(e, FALLBACK);
    dispatchSafe(store, keyId, { type: "failed", failure });
    return { status: "failed", failure, released: await release(deps, [creditId]) };
  }
}

/** The MANA each spender of a group must be allowed to pull (the group total, per spender). */
function spendersOf(lines: ResolvedLine[]): { spender: string; chainId: number; requiredWei: bigint }[] {
  const bySpender = new Map<string, { spender: string; chainId: number; requiredWei: bigint }>();
  for (const unit of toUnits(lines)) {
    const { spender, chainId } = manaSpenderFor("mana", purchaseTargetFor(unit));
    const amount = unit.quote.method === "mana" ? unit.quote.amount : 0n;
    const key = `${chainId}:${spender}`;
    const entry = bySpender.get(key) ?? { spender, chainId, requiredWei: 0n };
    entry.requiredWei += amount;
    bySpender.set(key, entry);
  }
  return [...bySpender.values()];
}

/** MANA group: allowance (explicit for a self-custody wallet), then accept or mint. */
async function runManaGroup(deps: CheckoutDeps, group: CartGroup, lines: ResolvedLine[]): Promise<GroupResult> {
  const { ctx, store } = deps;
  const keyId = group.keyId;
  const spenders = spendersOf(lines);
  try {
    const pending: { auth: ShopAuthorization; requiredWei: bigint }[] = [];
    for (const { spender, chainId, requiredWei } of spenders) {
      const auth: ShopAuthorization = { kind: AuthorizationKind.Allowance, contractAddress: requireContracts(chainId).mana.address, spenderAddress: spender, chainId };
      const authorized = await getAuthorizationStatus(auth, ctx.buyer, requiredWei, ctx.reader).catch(() => true);
      if (!authorized) pending.push({ auth, requiredWei });
    }
    for (const [i, { auth, requiredWei }] of pending.entries()) {
      if (needsApprovalStep(ctx.walletKind, false)) {
        const granted = deps.requestApproval ? await deps.requestApproval({ auth, index: i + 1, total: pending.length, requiredWei, keyId }) : false;
        if (!granted) {
          const failure: GroupFailure = { kind: "rejected", message: "MANA spending was not approved, so nothing was bought." };
          dispatchSafe(store, keyId, { type: "failed", failure });
          return { status: "failed", failure, released: [] };
        }
      } else {
        await ensureAuthorization({ auth, provider: ctx.provider, account: ctx.buyer, walletKind: ctx.walletKind, reader: ctx.reader, relayerUrl: ctx.relayerUrl, fetchImpl: ctx.fetchImpl, requiredWei });
      }
    }
  } catch (e) {
    const failure = failureFor(e);
    dispatchSafe(store, keyId, { type: "failed", failure });
    return { status: "failed", failure, released: [] };
  }

  let handedOff = false;
  let txHash: Hex | null = null;
  const onSigned = () => {
    if (handedOff) return;
    handedOff = true;
    dispatchSafe(store, keyId, { type: "submit" });
  };
  const onBroadcast = (hash: Hex) => {
    txHash = hash;
    dispatchSafe(store, keyId, { type: "hash", txHash: hash });
    dispatchSafe(store, keyId, { type: "observed" });
  };
  const units = toUnits(lines);
  const required = spenders.reduce((sum, s) => sum + s.requiredWei, 0n);
  try {
    let hash: Hex;
    if (group.key.call.kind === "storeBuy") {
      const mints: MintToBuy[] = units.map((u) => {
        const t = purchaseTargetFor(u);
        if (t.kind !== "store") throw new Error("A store group holds only mints");
        return t.mint;
      });
      hash = await mintPayingMana(ctx, { mints, chainId: group.key.chainId, requiredManaWei: required, onSigned, onBroadcast });
    } else {
      const targets = units.map(purchaseTargetFor);
      const trades = targets.flatMap((t) => (t.kind === "trade" ? [t.trade] : []));
      const coupons = targets.flatMap((t) => (t.kind === "trade" && t.coupon ? [t.coupon] : []));
      hash = await acceptPayingMana(ctx, { trades, coupons: coupons.length ? coupons : undefined, requiredManaWei: required, onSigned, onBroadcast });
    }
    if (!txHash) onBroadcast(hash);
    dispatchSafe(store, keyId, { type: "confirmed", txHash: hash });
    return { status: "confirmed", txHash: hash };
  } catch (e) {
    if ((isRevertedTxError(e) || isMetaTxReverted(e)) && txHash) {
      const failure: GroupFailure = { kind: "reverted", message: "The transaction failed on-chain. Nothing was bought and no MANA was spent." };
      dispatchSafe(store, keyId, { type: "failed", failure });
      return { status: "failed", failure, released: [] };
    }
    if (isMetaTxPending(e) || isPendingTxError(e) || txHash) return { status: "pending", txHash };
    if (isGaslessUnavailable(e) && e.reason === "relayer-unreachable" && handedOff) return { status: "pending", txHash: null };
    const failure = handedOff ? { ...failureFor(e), kind: "rejected" as const } : failureFor(e);
    dispatchSafe(store, keyId, { type: "failed", failure });
    return { status: "failed", failure, released: [] };
  }
}

/** Run one idle/failed/expired group. `resolved` maps cart line ids to their settlement material. */
export async function runGroup(deps: CheckoutDeps, group: CartGroup, resolved: Map<string, ResolvedLine>): Promise<GroupResult> {
  const progress = progressOf(deps.store, group.keyId);
  if (!progress) return { status: "skipped", reason: "not in this checkout" };
  if (!["idle", "failed", "expired"].includes(progress.status)) return { status: "skipped", reason: progress.status };
  const lines = linesOf(group, resolved);
  if (!lines) return { status: "skipped", reason: "a line could not be resolved for settlement" };
  deps.store.dispatch(group.keyId, { type: "authorize" });
  return group.key.method === "credits" ? runCreditsGroup(deps, group, lines) : runManaGroup(deps, group, lines);
}

/**
 * Follow a group that may have been broadcast. With a hash: its receipt (a late receipt confirms it).
 * Without one (the relay gave no usable answer): the credit's own record, since the server reconciles
 * the chain's CreditUsed events; a MANA group without a hash stays pending, honestly unknown.
 */
export async function watchGroup(deps: CheckoutDeps, progress: GroupProgress): Promise<GroupResult> {
  const { ctx, store } = deps;
  if (progress.status !== "submitted" && progress.status !== "pending") return { status: "skipped", reason: progress.status };
  if (progress.txHash) {
    try {
      await waitForSettlement(ctx.reader, progress.key.chainId, progress.txHash, ctx.wait);
      dispatchSafe(store, progress.keyId, { type: "confirmed", txHash: progress.txHash });
      return { status: "confirmed", txHash: progress.txHash };
    } catch (e) {
      if (isRevertedTxError(e)) {
        const failure: GroupFailure = { kind: "reverted", message: "The transaction failed on-chain. Nothing was bought." };
        dispatchSafe(store, progress.keyId, { type: "failed", failure });
        const released = progress.key.method === "credits" ? await release(deps, progress.creditIds) : [];
        return { status: "failed", failure, released };
      }
      return { status: "pending", txHash: progress.txHash };
    }
  }
  if (progress.key.method !== "credits" || progress.creditIds.length === 0) return { status: "pending", txHash: null };
  try {
    const { items } = await fetchUserPurchases(ctx.buyer, { all: true, first: 50 }, ctx.client);
    const mine = items.filter((p) => progress.creditIds.includes(p.id));
    const settled = mine.find((p) => p.status === "SETTLED");
    if (settled) {
      if (settled.txHash) dispatchSafe(store, progress.keyId, { type: "hash", txHash: settled.txHash });
      dispatchSafe(store, progress.keyId, { type: "confirmed", ...(settled.txHash ? { txHash: settled.txHash } : {}) });
      return { status: "confirmed", txHash: settled.txHash };
    }
    const expired = mine.length > 0 && mine.every((p) => p.status === "EXPIRED" && !p.submittedTxHash);
    if (expired && progress.status === "submitted") {
      const failure: GroupFailure = { kind: "rejected", message: "This part of your purchase never went out. Its Credits were returned." };
      dispatchSafe(store, progress.keyId, { type: "failed", failure });
      return { status: "failed", failure, released: [] };
    }
  } catch {
    // Unreadable history is not an answer.
  }
  return { status: "pending", txHash: null };
}

/**
 * Groups left `authorizing` (a reload or a closed tab between the credit and its hand-off) hold a reserved
 * credit nothing was sent with: cancel those credits and put the groups back to idle, so no path that
 * starts them again authorizes a second credit next to the stranded one.
 */
export async function releaseAuthorizing(deps: CheckoutDeps, state: PurchaseState): Promise<PurchaseState> {
  const plan = resumePlan(state);
  if (plan.release.length) await release(deps, plan.release);
  const next = prepareResume(state);
  deps.store.replaceCheckout(next);
  return next;
}

/**
 * Resume a checkout after an interruption (a declined prompt, a reload, a closed tab): credits
 * authorized for groups nothing was handed off for are released first, broadcast groups are watched,
 * and only unfinished groups run again. `groups` are the current partition's groups for those lines.
 */
export async function resumeCheckout(
  deps: CheckoutDeps,
  state: PurchaseState,
  groups: CartGroup[],
  resolved: Map<string, ResolvedLine>,
): Promise<Record<string, GroupResult>> {
  const plan = resumePlan(state);
  const results: Record<string, GroupResult> = {};
  await releaseAuthorizing(deps, state);
  for (const watching of plan.watch) results[watching.keyId] = await watchGroup(deps, watching);
  for (const run of plan.run) {
    const group = groups.find((g) => g.keyId === run.keyId);
    results[run.keyId] = group ? await runGroup(deps, group, resolved) : { status: "skipped", reason: "the cart changed; review it again" };
  }
  for (const done of plan.done) results[done.keyId] = { status: "confirmed", txHash: done.txHash };
  return results;
}
