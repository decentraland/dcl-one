// from: shop@94584a5c:app/src/components/BuyModal/BuyModal.tsx (confirmCombined: the credit sized to the
//       balance, the MANA gap in one useCredits, releaseReservation behind the spend guard),
//       app/src/lib/buy-mana.ts (buyWithCreditsAndMana, buyMintWithCreditsAndMana)
// Upstream's credits+MANA rail, kept INSIDE the MANA option of an eligible primary wearable or emote for a
// web3 wallet (SHOP.md): the whole credits the balance holds first, then MANA for the remainder, in ONE
// CreditsManager.useCredits. The MANA allowance for the manager is approved first (explicitly, for a
// self-custody wallet, and for exactly the MANA shown, never an unlimited one), so no credit is held while
// the buyer decides; then ONE credit is authorized at the line's whole price for its Credits part, the
// MANA the contract would pull is checked against the amount shown, and the call is relayed (or sent by
// the wallet when the relay cannot run). A credit is released only when nothing may have consumed it; a
// broadcast that has not confirmed is pending, never re-sent.

import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";

import Button from "../../../atoms/Button";
import Modal from "../../../components/Modal";
import type { ShopClient } from "@shop/api/shop";
import { authorizeUsdCredit, cancelUsdIntents, fetchUserPurchases, type PurchaseRecord } from "@shop/credits/credits";
import { invalidateAfterPurchase } from "../../checkout/lib/after-purchase";
import { isSignatureDeclined } from "../../checkout/chain/metatx";
import { isPendingTxError, isRevertedTxError } from "../../checkout/chain/wallet";
import AuthorizeStep from "../../checkout/components/AuthorizeStep";
import { exactAllowanceReady, exactGrantLock, getManaSpendingAuthorization, GrantNotRecordedError, isAuthorizationPending, needsApprovalStep, setAuthorization, type ShopAuthorization } from "../../checkout/lib/authorizations";
import { friendlyError, isRejection } from "../../checkout/lib/errors";
import { webLock } from "../../checkout/lib/web-lock";
import { isSettlementPending } from "../../checkout/purchase/gasless";
import { purchaseTargetFor, resolveLine, tradeManaPrice } from "../../checkout/purchase/cart";
import { buyMintWithCreditsAndMana, buyWithCreditsAndMana, manaSpenderFor, splitTerms, type PurchaseTarget } from "../../checkout/purchase/mana";
import type { RailContext } from "../../checkout/purchase/rail";
import { createSpendGuard } from "@shop/checkout/purchase/spend-guard";
import { isMetaTxPending } from "../../checkout/purchase/tx-confirm";
import { useShopServices } from "../../checkout/services";
import { cartLineId, type CartAddInput } from "../../cart/store";
import { formatMana } from "@shop/model/money";
import type { CartLine, PurchaseOffer } from "@shop/model/purchase";
import type { MixedPayment } from "@shop/payment/present";
import { useShopWallet } from "../../wallet/context";
import "./legacyorderbuy.css";

export type CombinedBuyProps = {
  line: CartAddInput;
  offer: PurchaseOffer;
  mixed: MixedPayment;
  onClose(): void;
  onDone?(txHash: string | null): void;
  portal?: boolean;
};

type CombinedPhase =
  | { kind: "review" }
  | { kind: "approve"; auth: ShopAuthorization }
  | { kind: "working" }
  | { kind: "success"; txHash: string | null }
  | { kind: "pending" }
  | { kind: "error"; message: string };

const FALLBACK = "We couldn't complete the purchase. Nothing was charged.";
const REVERTED = "The purchase failed on-chain, so nothing was bought. The Credits it held come back on their own.";

// A split sent and not confirmed yet, per account and item, in this browser: every tab reads it and it outlives a restart,
// since a sale whose outcome is unknown is exactly when the buyer opens the item again elsewhere. One tab at a time runs a
// split of an item (a Web Lock per account and item): that run reads the record, writes it before the sale may leave, and
// changes or clears only its own attempt's. A record whose credit is known stays until the sale's receipt, or the node's
// record of that credit, says what became of it: time alone never lets it go. It is there however the split was started
// (Buy now's dialog or the page's) and even when its screen is gone by then (an account switched away remounts the dialog).
const SENT_KEY = "dcl-shop-buynow-split:v1:";
/** How long a record from before credits were kept with it holds; and how long Check again waits on a fresh one. */
export const SENT_HOLD_MS = 15 * 60 * 1000;
const NOT_RECORDED = "Your browser couldn't keep a record of this purchase, so it wasn't sent. Nothing was charged.";
const ELSEWHERE = "This item is being bought in another window. Nothing was charged here.";
const APPROVING_ELSEWHERE = "A purchase with Credits and MANA, or its MANA approval, is in progress in another window. Try again in a moment. Nothing was charged here.";
const UNGUARDED = "This browser can't make sure a purchase with Credits and MANA is sent only once, so it can't be paid this way here.";
/** The node's purchase history, read this many lines at a time (its most) while looking for one credit. */
const HISTORY_PAGE = 200;

/** The split could not be recorded before leaving, so it was not sent. */
class SplitNotRecorded extends Error {}

/** The storage every tab of this browser shares; none on a page without it, or where the browser blocks it. */
function holdStorage(): Storage | null {
  try {
    return typeof localStorage !== "undefined" ? localStorage : null;
  } catch {
    return null;
  }
}

/**
 * `attempt`: the run that wrote it, the only one that changes or clears it. `creditId`: the credit the split reserved. When
 * Check again finds the purchase reverted it retires that reservation, as the live path does; the Credits then come back
 * with the expiry sweep. `creditExpiresAt` (unix seconds): when that credit stops being spendable. Its receipt settles a
 * split sent with a hash; one with no receipt yet, or no hash to read (an answer lost), is settled once that credit has
 * expired by the node's record of it: spent completes it, released lets the item go, anything else keeps it held.
 */
export type SentSplit = {
  at: number;
  attempt: string | null;
  txHash: string | null;
  chainId: number | null;
  creditId: string | null;
  creditExpiresAt: number | null;
};

const sentKey = (account: string | null, lineId: string) => `${SENT_KEY}${account?.toLowerCase() ?? "anonymous"}:${lineId}`;
/** The Web Lock a split of this item by this account runs and settles under, in every tab. */
const sentLock = (account: string, lineId: string) => `dcl-shop-buynow-split:${account.toLowerCase()}:${lineId}`;

/** The record as stored, held or not. A bare number is the first format's (a timestamp, written before the hash was kept). */
function readSentRecord(account: string | null, lineId: string): SentSplit | null {
  try {
    const raw = holdStorage()?.getItem(sentKey(account, lineId)) ?? null;
    const parsed = raw ? (JSON.parse(raw) as unknown) : null;
    if (typeof parsed === "number") return { at: parsed, attempt: null, txHash: null, chainId: null, creditId: null, creditExpiresAt: null };
    const sent = parsed as Partial<SentSplit> | null;
    if (!sent || typeof sent.at !== "number") return null;
    return {
      at: sent.at,
      attempt: typeof sent.attempt === "string" ? sent.attempt : null,
      txHash: typeof sent.txHash === "string" ? sent.txHash : null,
      chainId: typeof sent.chainId === "number" ? sent.chainId : null,
      creditId: typeof sent.creditId === "string" ? sent.creditId : null,
      creditExpiresAt: typeof sent.creditExpiresAt === "number" ? sent.creditExpiresAt : null,
    };
  } catch {
    return null;
  }
}

const held = (sent: SentSplit, now: number) => now >= sent.at && now - sent.at < SENT_HOLD_MS;
/** The record still holds the item: one whose credit is known until it is settled; one from before, for the hold. */
const keeps = (sent: SentSplit, now: number) => sent.creditId !== null || held(sent, now);
/** The split's credit, spent or not, can no longer be spent: only then can the node's record of it settle the split. */
const creditOver = (sent: SentSplit, now: number) => sent.creditExpiresAt === null || now >= sent.creditExpiresAt * 1000;
/** When Check again next has something to do for a split with no hash: its hold and its credit both over. */
const releasableAt = (sent: SentSplit) => Math.max(sent.at + SENT_HOLD_MS, (sent.creditExpiresAt ?? 0) * 1000);

/** The split this account sent for this line and has not settled: kept as `keeps` says. */
export function readSentSplit(account: string | null, lineId: string, now: number = Date.now()): SentSplit | null {
  const sent = readSentRecord(account, lineId);
  return sent && keeps(sent, now) ? sent : null;
}

/** Writes `sent` unless another attempt's record still holds the item; whether it was kept (written and read back). */
function writeSentSplit(account: string | null, lineId: string, sent: SentSplit): boolean {
  try {
    const storage = holdStorage();
    if (!storage) return false;
    const current = readSentRecord(account, lineId);
    if (current && current.attempt !== sent.attempt && keeps(current, Date.now())) return false;
    const key = sentKey(account, lineId);
    const value = JSON.stringify(sent);
    storage.setItem(key, value);
    return storage.getItem(key) === value;
  } catch {
    return false;
  }
}

/** Clears the record only while it is `attempt`'s: an older attempt's outcome never wipes a newer one's record. */
function clearSentSplit(account: string | null, lineId: string, attempt: string | null): void {
  try {
    if (readSentRecord(account, lineId)?.attempt !== attempt) return;
    holdStorage()?.removeItem(sentKey(account, lineId));
  } catch {
    // Nothing was stored.
  }
}

/** A new run's own id. */
function newAttempt(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/** The node's record of one credit (listed by its lines, `<salt>:<position>`), read page by page until found or the end. */
async function findCreditRecord(account: string, creditId: string, client: ShopClient): Promise<PurchaseRecord | null> {
  const salt = creditId.toLowerCase();
  for (let skip = 0; ; ) {
    const { items, total } = await fetchUserPurchases(account, { all: true, first: HISTORY_PAGE, skip }, client);
    const found = items.find((p) => p.id.split(":")[0]!.toLowerCase() === salt);
    if (found) return found;
    skip += items.length;
    if (items.length === 0 || skip >= total) return null;
  }
}

/** What became of a sent split, as far as can be read now: null while nothing says. */
type Settled = { kind: "spent"; txHash: string | null } | { kind: "reverted" } | { kind: "released" };

/** What paying with Credits and MANA is doing, and what the screen drawing it can do next. */
type CombinedBuyRun = {
  phase: CombinedPhase;
  busy: boolean;
  /** The split the buyer confirmed with `start`, which the purchase pays: what an approval asks for, not a re-poll. */
  agreed: MixedPayment | null;
  /** Why it cannot start: no wallet to send it, or a wallet on another account than the session's. */
  blocked: string | null;
  start(): Promise<void>;
  /** The MANA approval a self-custody wallet grants before the purchase. */
  grant(): Promise<void>;
  /** Granted: the purchase goes on, on the terms it started with, unless the screen that started it is gone. */
  approved(): void;
  declined(): void;
  /** Back to the start after a failure, so the buyer can choose again. */
  reset(): void;
  /** A split for this account and line was sent and is not known to be confirmed yet (this run's, an earlier one, or another tab's). */
  remembered: boolean;
  /**
   * Check again has something to do: a transaction to read; or, for a split with no hash to read (an answer lost), its
   * hold and its credit both over, so the node's record of that credit can tell spent from released; or a record from
   * before credits were kept with it whose hold has run out.
   */
  checkable: boolean;
  checking: boolean;
  /**
   * Reads the sent split's receipt: confirmed completes it, reverted fails it (retiring its credit). With no receipt yet, or
   * none to read, the node's record of its credit decides once that credit can no longer be spent: spent completes it,
   * released lets the item go, anything else keeps it held. Only the attempt it read is cleared, under the item's lock.
   */
  checkAgain(): Promise<void>;
};

type Terms = { offer: PurchaseOffer; mixed: MixedPayment };

/**
 * The credits+MANA purchase itself, for whichever screen draws it: this file's dialog, or Buy now's (BuyModal), which
 * shows it on upstream's own screens. The terms are the ones `start` saw: an approval in between never changes them.
 */
export function useCombinedBuy({ line, offer, mixed, onDone }: { line: CartAddInput; offer: PurchaseOffer | null; mixed: MixedPayment | null; onDone?(txHash: string | null): void }): CombinedBuyRun {
  const wallet = useShopWallet();
  const services = useShopServices();
  const qc = useQueryClient();
  // A finished purchase spent MANA and Credits: the balances, the item and the buyer's lists are read again, as the
  // checkout does after its own purchases.
  const finished = (txHash: string | null) => {
    invalidateAfterPurchase(qc);
    onDone?.(txHash);
  };
  const [phase, setPhase] = useState<CombinedPhase>({ kind: "review" });
  const guard = useRef(createSpendGuard());
  // The terms `start` agreed: kept in a ref for the purchase that runs on, and in state for the screen that shows them.
  const terms = useRef<Terms | null>(null);
  const [agreed, setAgreed] = useState<MixedPayment | null>(null);
  // An approval granted after its screen closed must not go on to buy: nothing would be left to show it.
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const lineId = cartLineId(line.asset, line.listing);
  const [sent, setSent] = useState(() => readSentSplit(wallet.account, lineId));
  useEffect(() => setSent(readSentSplit(wallet.account, lineId)), [wallet.account, lineId]);
  // Drawn again the moment the hold runs out, so a record with nothing to read can then be let go by Check again.
  const [, setExpiredTick] = useState(0);
  useEffect(() => {
    if (!sent) return;
    const left = releasableAt(sent) - Date.now();
    if (left <= 0) return;
    const t = setTimeout(() => setExpiredTick((n) => n + 1), left + 1);
    return () => clearTimeout(t);
  }, [sent]);
  const [checking, setChecking] = useState(false);
  const provider = wallet.session.kind === "web3" ? (wallet.host?.provider?.() ?? null) : null;
  const busy = phase.kind === "working";
  const blocked = !provider || !wallet.account ? "Connect your wallet to pay with Credits and MANA." : wallet.accountMismatch ? "Your wallet is on another account than the one you're signed in with." : null;

  function context(): RailContext | null {
    if (!provider || !wallet.account) return null;
    return {
      buyer: wallet.account,
      provider,
      reader: services.reader,
      client: services.client,
      walletKind: wallet.host?.walletKind ?? null,
      relayerUrl: services.relayerUrl,
      fetchImpl: services.fetchImpl,
      gasless: services.gasless,
      wait: services.wait,
      locks: services.locks,
    };
  }

  /** The MANA allowance the sale's CreditsManager pulls the gap through, which an exact grant sets. */
  function spendingAuth(target: PurchaseTarget): ShopAuthorization {
    const { spender, chainId } = manaSpenderFor("combined", target);
    return getManaSpendingAuthorization(chainId, spender);
  }

  async function resolveTarget(ctx: RailContext, offer: PurchaseOffer): Promise<PurchaseTarget> {
    const cartLine: CartLine = {
      id: "combined",
      asset: line.asset,
      quantity: 1,
      method: "mana",
      listing: line.listing ?? {},
      display: line.display,
      provenance: line.provenance ?? {},
      seen: offer,
      addedAt: 0,
    };
    const outcome = await resolveLine(cartLine, offer, wallet.session, ctx.buyer, services.resolvers);
    if (outcome.status !== "buyable") {
      throw new Error(outcome.status === "own" ? "This is your own listing." : outcome.status === "stale" ? "The price changed. Check it again before buying." : "This item is no longer available.");
    }
    return purchaseTargetFor(outcome.resolved);
  }

  async function start() {
    const ctx = context();
    if (!ctx || blocked || !offer || !mixed || busy) return;
    // Sent from another tab since this screen opened: shown confirming, never a second one beside it.
    const earlier = readSentSplit(ctx.buyer, lineId);
    if (earlier) {
      setSent(earlier);
      setPhase({ kind: "review" });
      return;
    }
    // Without Web Locks nothing proves the approval or the sale goes out once: nothing is asked of the wallet.
    if (!services.locks) {
      setPhase({ kind: "error", message: UNGUARDED });
      return;
    }
    const agreedTerms: Terms = { offer, mixed };
    terms.current = agreedTerms;
    setAgreed(mixed);
    setPhase({ kind: "working" });
    try {
      const target = await resolveTarget(ctx, agreedTerms.offer);
      const auth = spendingAuth(target);
      // Ready only with no recorded approval that may still run: an allowance another grant covers does not retire one.
      const allowed = await exactAllowanceReady(auth, ctx.buyer, agreedTerms.mixed.manaWei, ctx.reader);
      if (!allowed && needsApprovalStep(ctx.walletKind, false)) {
        setPhase({ kind: "approve", auth });
        return;
      }
      await pay(ctx, agreedTerms, target);
    } catch (e) {
      setPhase({ kind: "error", message: friendlyError(e, FALLBACK, { sale: true }) });
    }
  }

  async function pay(ctx: RailContext, agreedTerms: Terms, known?: PurchaseTarget) {
    setPhase({ kind: "working" });
    if (!services.locks) {
      setPhase({ kind: "error", message: UNGUARDED });
      return;
    }
    let target: PurchaseTarget;
    try {
      target = known ?? (await resolveTarget(ctx, agreedTerms.offer));
    } catch (e) {
      setPhase({ kind: "error", message: friendlyError(e, FALLBACK, { sale: true }) });
      return;
    }
    // The exact grant's lock, then the item's, in every window in that order: the sale reconciles and spends the manager's
    // allowance with no approval starting beside it, and holds the item's record from its first read to its last write,
    // so two windows never both find it free and both send.
    const grantRelease = await webLock(services.locks, exactGrantLock(spendingAuth(target), ctx.buyer));
    if (!grantRelease) {
      setPhase({ kind: "error", message: APPROVING_ELSEWHERE });
      return;
    }
    try {
      const release = await webLock(services.locks, sentLock(ctx.buyer, lineId));
      if (!release) {
        setPhase({ kind: "error", message: ELSEWHERE });
        return;
      }
      try {
        await payHeld({ ...ctx, grantLockHeld: true }, agreedTerms, target);
      } finally {
        release();
      }
    } finally {
      grantRelease();
    }
  }

  async function payHeld(ctx: RailContext, { mixed }: Terms, target: PurchaseTarget) {
    // Read under the lock: a split another tab sent since this screen opened is shown confirming, and nothing is authorized.
    const earlier = readSentSplit(ctx.buyer, lineId);
    if (earlier) {
      setSent(earlier);
      setPhase({ kind: "review" });
      return;
    }
    const attempt = newAttempt();
    let creditId: string | null = null;
    let creditExpiresAt: number | null = null;
    let sentHash: string | null = null;
    let sentChain: number | null = null;
    // Set once the sale itself goes to the relay or the wallet: before it, a failure is the MANA approval's, and no sale left.
    let dispatched = false;
    try {
      sentChain = manaSpenderFor("combined", target).chainId;
      // The MANA the sale moves: the mint's price, or the listing's after its coupon. A USD-pegged listing's is only fixed
      // at settlement, so it has no split, and nothing is authorized for one.
      const price = target.kind === "trade" ? tradeManaPrice(target.trade, target.coupon) : BigInt(target.mint.item.priceWei);
      if (price === null) throw new Error("This listing is priced in USD, so it can't be paid with Credits and MANA together.");
      const item = line.asset.token.kind === "item" ? { contractAddress: line.asset.contract, itemId: line.asset.token.itemId } : undefined;
      // The whole price is what the node checks against its quote; the credit it signs is for the Credits part alone.
      const authorized = await authorizeUsdCredit(
        mixed.priceCents,
        {
          ...(target.kind === "trade" ? { tradeId: target.trade.id } : {}),
          ...(item ? { item } : {}),
          creditsCents: mixed.creditsCents,
          // The session the quote was asked with, signed: the node authorizes a part only for a web3 wallet.
          session: { walletKind: wallet.session.kind === "web3" ? "web3" : "web2" },
        },
        ctx.client,
      );
      creditId = authorized.credit.id;
      creditExpiresAt = authorized.credit.expiresAt;
      // The credit must be the node's share for these terms, and the MANA left is what the buyer confirmed at most: anything
      // else stops here, before anything is sent, and the credit is released below.
      const { gapWei: gap } = splitTerms({
        priceWei: price,
        quotedCents: mixed.priceCents,
        partCents: mixed.creditsCents,
        credit: authorized.credit,
        confirmedWei: mixed.manaWei,
      });
      const id = creditId;
      guard.current.submitStarted(id);
      const gapArgs = {
        credits: [{ ...authorized.credit }],
        manaGapWei: gap,
        onBroadcast: ({ txHash }: { txHash: string }) => {
          sentHash = txHash;
          guard.current.broadcast(id, txHash);
        },
        onReverted: ({ txHash }: { txHash: string | null }) => {
          if (txHash) guard.current.reverted(txHash);
        },
        onUnobservable: () => guard.current.unobservable(id),
        // Recorded before it may leave, for every tab and a restart: a split the browser cannot record is not sent.
        onDispatch: () => {
          if (!writeSentSplit(ctx.buyer, lineId, { at: Date.now(), attempt, txHash: null, chainId: sentChain, creditId: id, creditExpiresAt })) {
            throw new SplitNotRecorded(NOT_RECORDED);
          }
          dispatched = true;
        },
      };
      const hash =
        target.kind === "trade"
          ? await buyWithCreditsAndMana(ctx, { trade: target.trade, coupon: target.coupon, ...gapArgs })
          : await buyMintWithCreditsAndMana(ctx, { mint: target.mint, ...gapArgs });
      guard.current.submitFinished(id);
      clearSentSplit(ctx.buyer, lineId, attempt);
      setSent(null);
      setPhase({ kind: "success", txHash: hash });
      finished(hash);
    } catch (e) {
      if (creditId) guard.current.submitFinished(creditId);
      const stillPending = isSettlementPending(e) || isPendingTxError(e) || isMetaTxPending(e);
      // A sale handed off that may have consumed its credit (a broadcast not seen reverting, an answer the relay or the
      // wallet lost) is still confirming: held for this account and item, never offered again until it is found out.
      if (dispatched && (stillPending || (creditId !== null && guard.current.mayBeConsumed(creditId)))) {
        // Recorded for the account that sent it, whatever the wallet shows by now, with its hash when there is one. Should
        // this write fail, the record made before it left stays: held, with no hash, until the node settles its credit.
        const record: SentSplit = { at: Date.now(), attempt, txHash: sentHash, chainId: sentChain, creditId, creditExpiresAt };
        writeSentSplit(ctx.buyer, lineId, record);
        setSent(record);
        setPhase({ kind: "pending" });
        return;
      }
      if (creditId && !guard.current.mayBeConsumed(creditId)) await cancelUsdIntents([creditId], ctx.client).catch(() => 0);
      // Nothing of this run is left confirming: its own record goes.
      if (dispatched) clearSentSplit(ctx.buyer, lineId, attempt);
      const message =
        e instanceof SplitNotRecorded
          ? NOT_RECORDED
          : e instanceof GrantNotRecordedError
            ? `${e.message} Nothing was bought.`
            : isAuthorizationPending(e) || (stillPending && !dispatched)
              ? "Your MANA approval is still confirming, so nothing was bought. Try again in a moment."
              : isRejection(e) || isSignatureDeclined(e)
                ? "You cancelled the request in your wallet. Nothing was charged."
                : isRevertedTxError(e)
                  ? REVERTED
                  : friendlyError(e, FALLBACK, { sale: true });
      setPhase({ kind: "error", message });
    }
  }

  /**
   * What became of a recorded split, from its receipt, else from the node's record of its credit once that credit can no
   * longer be spent: null while nothing says (still reserved, not listed, a failed read), however long ago it was sent.
   */
  async function settle(record: SentSplit, account: string | null): Promise<Settled | null> {
    if (record.txHash && record.chainId !== null) {
      // A receipt read that fails is no answer, as no receipt is none: either way the credit's record is asked next.
      const receipt = await services.reader.receipt(record.chainId, record.txHash).catch(() => null);
      if (receipt) return receipt.status === "reverted" ? { kind: "reverted" } : { kind: "spent", txHash: record.txHash };
    }
    if (record.creditId) {
      if (!creditOver(record, Date.now()) || !account) return null;
      try {
        const credit = await findCreditRecord(account, record.creditId, services.client);
        if (credit?.status === "SETTLED") return { kind: "spent", txHash: credit.txHash ?? record.txHash };
        if (credit?.status === "EXPIRED") return { kind: "released" };
      } catch {
        // Not an answer: still held.
      }
      return null;
    }
    // A record from before credits were kept with it: let go once its hold has run out.
    return held(record, Date.now()) ? null : { kind: "released" };
  }

  return {
    phase,
    busy,
    agreed,
    blocked,
    start,
    grant: async () => {
      const ctx = context();
      if (phase.kind !== "approve") return;
      if (!ctx) throw new Error("Connect your wallet again to approve.");
      const agreedTerms = terms.current;
      if (!agreedTerms) throw new Error("Start the purchase again to approve.");
      // Exactly the MANA the buyer agreed to pay, never an unlimited allowance to the CreditsManager.
      await setAuthorization({
        auth: phase.auth,
        active: true,
        amount: agreedTerms.mixed.manaWei,
        provider: ctx.provider,
        account: ctx.buyer,
        walletKind: ctx.walletKind,
        reader: ctx.reader,
        relayerUrl: ctx.relayerUrl,
        fetchImpl: ctx.fetchImpl,
        locks: ctx.locks,
      });
    },
    approved: () => {
      const ctx = context();
      if (mounted.current && ctx && terms.current) void pay(ctx, terms.current);
    },
    declined: () => setPhase({ kind: "error", message: "MANA spending was not approved, so nothing was bought." }),
    reset: () => {
      if (!busy) setPhase({ kind: "review" });
    },
    remembered: sent !== null,
    // A receipt to read; or a hold that has run out (Check again then lets the item go), which for a split with no hash
    // also waits for its credit to expire, so the node's record of it can tell spent from released.
    checkable:
      !!sent &&
      ((!!sent.txHash && sent.chainId !== null) ||
        (!held(sent, Date.now()) && (!!sent.txHash || !sent.creditId || creditOver(sent, Date.now())))),
    checking,
    checkAgain: async () => {
      const account = wallet.account;
      // Read even past the hold: a purchase that mined in the meantime is completed, never offered a second time.
      const record = readSentRecord(account, lineId);
      if (!record) {
        setSent(null);
        if (phase.kind === "pending") setPhase({ kind: "review" });
        return;
      }
      setChecking(true);
      let settled: Settled | null;
      try {
        settled = await settle(record, account);
      } finally {
        setChecking(false);
      }
      if (!settled || !account) return;
      // Applied under the item's lock, and only to the attempt that was read: another tab may have settled it meanwhile
      // and sent a new split, whose record an outcome of this one never wipes.
      const release = await webLock(services.locks, sentLock(account, lineId));
      if (!release) return;
      try {
        const current = readSentRecord(account, lineId);
        const same = current !== null && current.attempt === record.attempt && (record.attempt !== null || current.at === record.at);
        if (!same) {
          const shown = current && keeps(current, Date.now()) ? current : null;
          setSent(shown);
          if (!shown && phase.kind === "pending") setPhase({ kind: "review" });
          return;
        }
        clearSentSplit(account, lineId, record.attempt);
      } finally {
        release();
      }
      setSent(null);
      if (settled.kind === "reverted") {
        // The only purchase this credit went into reverted, so nothing consumed it: retire it now, as the live path does.
        // Retired credits come back with the expiry sweep, not at once, which is what the copy says.
        if (record.creditId) await cancelUsdIntents([record.creditId], services.client).catch(() => 0);
        setPhase({ kind: "error", message: REVERTED });
      } else if (settled.kind === "spent") {
        setPhase({ kind: "success", txHash: settled.txHash });
        finished(settled.txHash);
      } else if (phase.kind === "pending") {
        setPhase({ kind: "review" });
      }
    },
  };
}

export default function CombinedBuy({ line, offer, mixed, onClose, onDone, portal }: CombinedBuyProps) {
  const { phase, busy, agreed, blocked, start, grant, approved, declined, remembered, checkable, checking, checkAgain } = useCombinedBuy({ line, offer, mixed, onDone });
  const title = `Buy ${line.display.name}`;
  const credits = `${mixed.credits} ${mixed.credits === 1 ? "Credit" : "Credits"}`;
  const mana = `${formatMana(mixed.manaWei, { group: true })} MANA`;

  if (phase.kind === "approve") {
    return <AuthorizeStep auth={phase.auth} requiredWei={(agreed ?? mixed).manaWei} portal={portal} grant={grant} onAuthorized={approved} onCancel={declined} />;
  }

  return (
    <Modal onClose={busy ? undefined : onClose} closeOnBackdrop={!busy} closeOnEsc={!busy} showClose={!busy} width={460} portal={portal} ariaLabel={title}>
      <div className="shop-orderbuy" data-testid="combined-buy">
        <h2>{title}</h2>
        {phase.kind === "success" ? (
          <>
            <p role="status">Purchase complete. {line.display.name} is on its way to your wallet.</p>
            <Button onClick={onClose}>Done</Button>
          </>
        ) : phase.kind === "pending" || (remembered && phase.kind === "review") ? (
          // Still confirming, from this run or one sent earlier: never a second one beside it.
          <>
            <p role="status">The purchase was sent and is still confirming. It was not sent twice; check your purchases in a moment.</p>
            <div className="shop-orderbuy__actions">
              <Button onClick={() => void checkAgain()} disabled={checking || !checkable} data-testid="combined-check-again">
                Check again
              </Button>
              <Button variant="ghost" onClick={onClose}>
                Close
              </Button>
            </div>
          </>
        ) : (
          <>
            <dl className="shop-orderbuy__terms">
              <div>
                <dt>From your Credits</dt>
                <dd data-testid="combined-credits">{credits}</dd>
              </div>
              <div>
                <dt>From your wallet</dt>
                <dd data-testid="combined-mana">{mana}</dd>
              </div>
            </dl>
            <p className="shop-orderbuy__note">One transaction pays both. Your Credits are used first; MANA covers the rest.</p>
            {blocked ? <p className="shop-orderbuy__note" role="status">{blocked}</p> : null}
            {busy ? (
              <p className="shop-orderbuy__note" role="status" aria-live="polite">
                Confirm in your wallet…
              </p>
            ) : null}
            {phase.kind === "error" ? (
              <p className="shop-orderbuy__error" role="alert">
                {phase.message}
              </p>
            ) : null}
            <div className="shop-orderbuy__actions">
              <Button onClick={() => void start()} disabled={!!blocked || busy} data-testid="combined-confirm">
                {busy ? "Working…" : `Pay ${credits} + ${mana}`}
              </Button>
              <Button variant="ghost" onClick={onClose} disabled={busy}>
                Cancel
              </Button>
            </div>
          </>
        )}
      </div>
    </Modal>
  );
}
