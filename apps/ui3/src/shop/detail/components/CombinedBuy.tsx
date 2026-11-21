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
// broadcast that has not confirmed is pending, never re-sent. One split runs at a time per account, on every surface
// (splitsInFlight): another item's split whose credit the chain hasn't proven expired, a hold of this item, or a record
// this browser can't read stops a new one: before the approval, under the approval's lock, and once the sale holds the
// grant's lock (where this item's own hold is the item's claim to read). The approval takes that lock at its click and
// hands it to its sale, so no other window's purchase starts between them.

import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";

import Button from "../../../atoms/Button";
import Modal from "../../../components/Modal";
import { authorizeUsdCredit, cancelUsdIntents } from "@shop/credits/credits";
import { invalidateAfterPurchase } from "../../checkout/lib/after-purchase";
import { isSignatureDeclined } from "../../checkout/chain/metatx";
import { isPendingTxError, isRevertedTxError } from "../../checkout/chain/wallet";
import AuthorizeStep from "../../checkout/components/AuthorizeStep";
import {
  exactAllowanceReady,
  ExactGrantUnavailableError,
  exactGrantLock,
  getManaSpendingAuthorization,
  GrantCancelledError,
  GrantInProgressElsewhereError,
  GrantNotInEffectError,
  GrantNotRecordedError,
  GrantUncheckedError,
  isAuthorizationPending,
  needsApprovalStep,
  setAuthorization,
  type ShopAuthorization,
} from "../../checkout/lib/authorizations";
import { friendlyError, isRejection } from "../../checkout/lib/errors";
import { webLock } from "../../checkout/lib/web-lock";
import { isSettlementPending } from "../../checkout/purchase/gasless";
import { purchaseTargetFor, resolveLine, tradeManaPrice } from "../../checkout/purchase/cart";
import { buyMintWithCreditsAndMana, buyWithCreditsAndMana, manaSpenderFor, SaleCancelledError, splitTerms, type PurchaseTarget, type SplitDispatch } from "../../checkout/purchase/mana";
import type { RailContext } from "../../checkout/purchase/rail";
import {
  claimSplitHold,
  clearSettledSplitHold,
  clearSplitHold,
  readSplitHold,
  readSplitHoldRecord,
  settleSplitHold,
  splitHoldCheckable,
  splitHoldLine,
  splitHoldReleasableAt,
  splitCreditOverOnChain,
  splitsInFlight,
  splitInFlightMessage,
  splitToName,
  readAccountSplitHolds,
  readerFinalizedTime,
  writeSplitHold,
  type SplitHold,
  noteSplitHoldExpiry,
} from "../../checkout/purchase/splitHold";
import { splitEnvelope, type SplitSale } from "../../checkout/purchase/splitEvidence";
import { createSpendGuard } from "@shop/checkout/purchase/spend-guard";
import { isMetaTxPending } from "../../checkout/purchase/tx-confirm";
import { useShopServices } from "../../checkout/services";
import type { CartAddInput } from "../../cart/store";
import { parseAssetKey } from "@shop/model/asset";
import { Link } from "react-router";
import { itemRoute, tokenRoute } from "../../discovery/lib/routes";
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
  | { kind: "error"; message: string; held?: HeldItem };

/** Another item whose purchase holds this one back, as its record names it, and its page, where it can be checked again. */
export type HeldItem = { name: string; href: string };

const FALLBACK = "We couldn't complete the purchase. Nothing was charged.";
// Reached only before the sale leaves (a sale that left is held, whatever its receipt says): the approval's own revert.
const APPROVAL_REVERTED = "The MANA approval failed on-chain, so nothing was bought. The Credits it held come back on their own.";

// A split sent and not settled is held per account and item in every tab and on every surface (checkout/purchase/
// splitHold.ts): this dialog claims the item's lock, reads the hold inside it, writes it before the sale may leave, and
// clears only its own attempt's. It is there however the split was started (Buy now's dialog or the page's) and even when
// its screen is gone by then (an account switched away remounts the dialog).
const NOT_RECORDED = "Your browser couldn't keep a record of this purchase, so it wasn't sent. Nothing was charged.";
const ELSEWHERE = "This item is being bought in another window. Nothing was charged here.";
const APPROVING_ELSEWHERE = "A purchase with Credits and MANA, or its MANA approval, is in progress in another window. Try again in a moment. Nothing was charged here.";
const UNGUARDED = "This browser can't make sure a purchase with Credits and MANA is sent only once, so it can't be paid this way here.";
const CHECKING_ELSEWHERE = "This purchase is being paid or checked in another window. Check again in a moment.";
const UNCHECKABLE = "This browser can't check this purchase safely here. Open it in a browser with Web Locks, or check your purchases.";
const SPENT_ELSEWHERE = "The Credits this purchase held were spent on another purchase, so nothing was bought here.";
const EXPIRING = "This purchase's Credits expired unspent. The item is let go once that is confirmed, in a few minutes: check again then.";
const UNCLEARED = "This browser couldn't update its record of this purchase. Check again in a moment.";
const UNKEPT = "This browser couldn't save this purchase's transaction. Once you close this, checking it can take until its Credits expire.";

// One split at a time per account, on every surface (checkout/purchase/splitHold.ts, splitsInFlight): an exact approval
// sets the manager's allowance to one gap, so another split's approval would take an earlier one's MANA, and a split that
// finds the allowance covered could spend another's.
const SPLIT_ITEM_HELD = "A purchase of this item with Credits and MANA is still confirming. Nothing was charged here.";
const SPLITS_UNREADABLE = "This browser can't read your other purchases with Credits and MANA right now, so nothing was charged here.";

/** The exact grant's lock an approval took and hands to its sale: how to let it go, its name, and the terms it was for. */
type HeldGrant = { release: () => void; name: string; terms: Terms };

/** What holds a split back: why, and the other item whose purchase does, when its record names it. */
type SplitBlocked = { message: string; held?: HeldItem };

/** The page of the item `line` (an asset key) names; null for a key that names none. */
function itemPage(line: string): string | null {
  const asset = parseAssetKey(line);
  if (!asset) return null;
  return asset.token.kind === "item" ? itemRoute(asset.contract, asset.token.itemId) : tokenRoute(asset.contract, asset.token.tokenId);
}

/** Another item's split, held: named, with its page, when its record says which item it is; said generically otherwise. */
function inFlightBlock({ line, hold }: { line: string; hold: SplitHold }): SplitBlocked {
  const name = hold.itemName;
  if (!name) return { message: splitInFlightMessage(null) };
  const href = itemPage(line);
  return { message: splitInFlightMessage(name), ...(href ? { held: { name, href } } : {}) };
}

/** `message` with "Nothing was bought." said before its closing "Try again…", if it has one, or after it. */
function nothingBought(message: string): string {
  const retry = /^(.*?)\s*(Try again[^.]*\.)$/.exec(message);
  return retry ? `${retry[1]} Nothing was bought. ${retry[2]}` : `${message} Nothing was bought.`;
}

/** The split could not be recorded before leaving, so it was not sent. */
class SplitNotRecorded extends Error {}

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
  /** Why the last Check again did nothing: another window holds the item, or this browser cannot take its lock. */
  notice: string | null;
  /**
   * This run's own outcome when it sent nothing and its record would not clear: the item stays held, said for what it is
   * (not "sent"), until Check again can clear the record. Only this dialog knows it; reopened, the node's record decides.
   */
  unsent: string | null;
  /**
   * Reads the sent split's transaction (purchase/splitHold.ts, settleSplitHold): its terms confirmed complete it. Anything
   * else, its own bytes reverted included, leaves it to the node's record of its credit: spent by this split completes it,
   * by another purchase or expired unspent lets the item go, anything else keeps it held. No credit is retired here. Only
   * the attempt it read is cleared, under the item's lock.
   */
  checkAgain(): Promise<void>;
};

type Terms = { offer: PurchaseOffer; mixed: MixedPayment; buyer: string };

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
  // The grant's lock this dialog's approval took at the click, its name and the terms it was for: handed to the sale
  // (`approved`, then `pay`), which keeps it through its hand-off; let go if nothing takes it.
  const handedGrant = useRef<HeldGrant | null>(null);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      handedGrant.current?.release();
      handedGrant.current = null;
    };
  }, []);
  // The signed-in account and the wallet as last drawn: a step that waited reads them again, never what its click saw.
  const walletNow = useRef(wallet);
  useEffect(() => {
    walletNow.current = wallet;
  });
  /**
   * Whether the purchase agreed on as `agreed` is still this dialog's: open, with those very terms, for the account it was
   * agreed by, with the wallet still on it (a session or wallet account changed meanwhile ends it).
   */
  const stillAgreed = (agreed: Terms) => () =>
    mounted.current && terms.current === agreed && !walletNow.current.accountMismatch && walletNow.current.account?.toLowerCase() === agreed.buyer.toLowerCase();
  // The hold's key: the item, whatever listing this dialog buys it through (another surface may use another).
  const lineId = splitHoldLine(line.asset);
  const [sent, setSent] = useState(() => readSplitHold(wallet.account, lineId));
  useEffect(() => setSent(readSplitHold(wallet.account, lineId)), [wallet.account, lineId]);
  // Drawn again the moment the hold runs out, so a record with nothing to read can then be let go by Check again.
  const [, setExpiredTick] = useState(0);
  useEffect(() => {
    if (!sent) return;
    const left = splitHoldReleasableAt(sent) - Date.now();
    if (left <= 0) return;
    const t = setTimeout(() => setExpiredTick((n) => n + 1), left + 1);
    return () => clearTimeout(t);
  }, [sent]);
  const [checking, setChecking] = useState(false);
  // Set while a Check again runs, at once (the state that disables its button only lands on the next render).
  const checkingNow = useRef(false);
  // Credits of other splits proven unable to spend anything any more, by the chain's own time: once proven, for good.
  const creditsOver = useRef(new Set<string>());
  // The chain's finalized time, each read bounded (readerFinalizedTime): one that runs late proves nothing.
  const finalizedTime = readerFinalizedTime(services.reader, services.finalizedTimeMs);

  /**
   * Proves, by the chain's finalized time (splitCreditOverOnChain), which of `account`'s held splits of other items can't
   * spend anything any more, for splitBlock to read without waiting. This item's own is never let go by such a proof, so it
   * isn't asked. Only those the local clock says are past their expiry are asked, all at once, each read bounded; it runs
   * before any lock is taken, so no lock waits on the chain.
   */
  async function proveCreditsOver(account: string): Promise<void> {
    let holds: { line: string; hold: SplitHold }[];
    try {
      holds = readAccountSplitHolds(account);
    } catch {
      return;
    }
    const unproven = holds.flatMap(({ line, hold }) => (line !== lineId && hold.creditId !== null && !creditsOver.current.has(hold.creditId) ? [{ hold, creditId: hold.creditId }] : []));
    await Promise.all(
      unproven.map(async ({ hold, creditId }) => {
        if (await splitCreditOverOnChain(hold, finalizedTime)) creditsOver.current.add(creditId);
      }),
    );
  }

  /**
   * Why a split of this item by `account` can't start now, read at once (so the approval's `live` check, under its lock,
   * can ask it): another item's split that may still spend the allowance (its credit not proven over by the chain), any
   * hold of this item while it is kept (`sameItem`; a credit proven over doesn't let it go, only its purchase found out),
   * or purchases this browser can't read. Another item's is named, with its page, when its record names it. Null: nothing
   * holds it.
   */
  function splitBlock(account: string, sameItem: boolean): SplitBlocked | null {
    let live: { line: string; hold: SplitHold }[];
    try {
      live = splitsInFlight(account, Date.now(), (l, hold) => l !== lineId && hold.creditId !== null && creditsOver.current.has(hold.creditId));
    } catch {
      return { message: SPLITS_UNREADABLE };
    }
    if (sameItem && live.some((held) => held.line === lineId)) return { message: SPLIT_ITEM_HELD };
    const other = splitToName(live.filter((held) => held.line !== lineId));
    return other ? inFlightBlock(other) : null;
  }
  const [notice, setNotice] = useState<string | null>(null);
  // A run that proved it sent nothing, whose record the storage kept: its attempt, and the outcome shown once it clears.
  const [unsent, setUnsent] = useState<{ attempt: string; message: string } | null>(null);
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
    const earlier = readSplitHold(ctx.buyer, lineId);
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
    // Before anything is asked (no approval left behind for a purchase that can't go ahead): another item's split that may
    // still spend the allowance, or purchases this browser can't read. This item's own hold is shown above, as confirming.
    // Each is read again where it counts: under the approval's lock, and once the sale holds it.
    setPhase({ kind: "working" });
    await proveCreditsOver(ctx.buyer);
    const blockedNow = splitBlock(ctx.buyer, false);
    if (blockedNow) {
      setPhase({ kind: "error", ...blockedNow });
      return;
    }
    const agreedTerms: Terms = { offer, mixed, buyer: ctx.buyer };
    terms.current = agreedTerms;
    setAgreed(mixed);
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

  /**
   * The sale, under the exact grant's lock: `held` when this dialog's own approval took it at the approve click and hands it
   * over (held since, so no other window's purchase starts between that approval and this sale), else taken here, once.
   * After an approval, the listing is read again under that lock: still buyable on the terms agreed, through the same
   * manager, or nothing is reserved or sent and the lock is let go. No chain is read under it: which other splits' credits
   * are proven over was read before (by `start`, or by the approval).
   */
  async function pay(ctx: RailContext, agreedTerms: Terms, known?: PurchaseTarget, held?: HeldGrant) {
    setPhase({ kind: "working" });
    if (!services.locks) {
      held?.release();
      setPhase({ kind: "error", message: UNGUARDED });
      return;
    }
    let target: PurchaseTarget;
    try {
      target = known ?? (await resolveTarget(ctx, agreedTerms.offer));
    } catch (e) {
      held?.release();
      setPhase({ kind: "error", message: friendlyError(e, FALLBACK, { sale: true }) });
      return;
    }
    const grantName = exactGrantLock(spendingAuth(target), ctx.buyer);
    if (held && (held.name !== grantName || !stillAgreed(agreedTerms)())) {
      // Another manager than the one approved, or a dialog closed (or terms changed) while the listing was read again.
      held.release();
      if (held.name !== grantName) setPhase({ kind: "error", message: FALLBACK });
      return;
    }
    // The exact grant's lock, then the item's, in every window in that order: the sale reconciles and spends the manager's
    // allowance with no approval starting beside it, and holds the item's record from its first read to its last write,
    // so two windows never both find it free and both send.
    const grantRelease = held?.release ?? (await webLock(services.locks, grantName));
    if (!grantRelease) {
      setPhase({ kind: "error", message: APPROVING_ELSEWHERE });
      return;
    }
    try {
      // Under it, no other split of this account can start (each hands off holding it, its hold written by then): another
      // item's split that may still spend the allowance holds this one (its credit not proven over by the read before the
      // lock; a hold that appeared since counts as in flight). This item's own is the claim's to read, just below.
      const inFlight = splitBlock(ctx.buyer, false);
      if (inFlight) {
        setPhase({ kind: "error", ...inFlight });
        return;
      }
      // The item's claim, read under its lock: a split another tab or surface sent since this screen opened is shown
      // confirming, and nothing is authorized.
      const claim = await claimSplitHold(services.locks, ctx.buyer, lineId);
      if (claim.kind === "held") {
        setSent(claim.hold);
        setPhase({ kind: "review" });
        return;
      }
      if (claim.kind !== "claimed") {
        setPhase({ kind: "error", message: claim.kind === "busy" ? ELSEWHERE : UNGUARDED });
        return;
      }
      try {
        await payHeld({ ...ctx, grantLockHeld: true, live: stillAgreed(agreedTerms) }, agreedTerms, target, claim.attempt);
      } finally {
        claim.release();
      }
    } finally {
      grantRelease();
    }
  }

  async function payHeld(ctx: RailContext, { mixed }: Terms, target: PurchaseTarget, attempt: string) {
    let creditId: string | null = null;
    let creditExpiresAt: number | null = null;
    let sentHash: string | null = null;
    let sentChain: number | null = null;
    // The sale's terms, kept with the hold before it may leave: what tells its execution apart (purchase/splitEvidence.ts).
    // A holder, since the callbacks below set it: read after them, it is whatever they last wrote.
    const kept: { sale: SplitSale | null; reverted: string | null } = { sale: null, reverted: null };
    // Set once the sale itself goes to the relay or the wallet: before it, a failure is the MANA approval's, and no sale left.
    let dispatched = false;
    // Kept with each record, so a purchase of another item this one holds back can say which item to check.
    const itemName = line.display.name;
    /** Held for this account and item, with what is known of it, and shown confirming. */
    /** Whether its record now says so; if not, the notice says it can only be checked by its credit later. */
    const holdConfirming = (record: SplitHold): boolean => {
      // Should this write fail, the record made before the sale left stays: held, with no hash, until the node settles it.
      const kept = writeSplitHold(ctx.buyer, lineId, record);
      setSent(record);
      setPhase({ kind: "pending" });
      if (!kept) setNotice(UNKEPT);
      return kept;
    };
    /**
     * Applies what the chain and the node say of this run's sale as `record` (settleSplitHold), if that settles it: bought
     * completes it, its credit spent on another purchase lets the item go. Whether it did; anything less is the caller's.
     * The outcome shows only once its record is gone (read back): one the storage kept would show the sale confirming on
     * the next open after this one said it was done. Then it stays held, with what is known of it, and Check again applies
     * the outcome once its record can go.
     */
    const settledLive = async (record: SplitHold): Promise<boolean> => {
      const settled = await settleSplitHold(record, { account: ctx.buyer, reader: ctx.reader, client: ctx.client, finalizedTime: readerFinalizedTime(ctx.reader, services.finalizedTimeMs) }).catch(() => null);
      if (settled?.kind !== "spent" && settled?.kind !== "elsewhere") return false;
      if (!clearSplitHold(ctx.buyer, lineId, attempt)) {
        // Kept with the hash that settled it (a sibling the node attributed, say), so the next check reads that one at once.
        if (holdConfirming(settled.kind === "spent" ? { ...record, txHash: settled.txHash } : record)) setNotice(UNCLEARED);
        return true;
      }
      setSent(null);
      if (settled.kind === "spent") {
        setPhase({ kind: "success", txHash: settled.txHash });
        finished(settled.txHash);
      } else setPhase({ kind: "error", message: SPENT_ELSEWHERE });
      return true;
    };
    try {
      const saleChain = manaSpenderFor("combined", target).chainId;
      sentChain = saleChain;
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
      /** Keeps `sale` as this run's record before the sale may leave: a browser that cannot keep it sends nothing. */
      const recordSale = (sale: SplitSale) => {
        kept.sale = sale;
        if (!writeSplitHold(ctx.buyer, lineId, { at: Date.now(), attempt, txHash: null, chainId: saleChain, creditId: id, creditExpiresAt, sale, itemName })) {
          throw new SplitNotRecorded(NOT_RECORDED);
        }
      };
      const gapArgs = {
        credits: [{ ...authorized.credit }],
        manaGapWei: gap,
        onBroadcast: ({ txHash }: { txHash: string }) => {
          sentHash = txHash;
          guard.current.broadcast(id, txHash);
        },
        // A revert of its hash settles nothing on its own (see below): what it says goes to the resolver.
        onReverted: ({ txHash }: { txHash: string | null }) => {
          if (txHash) kept.reverted = txHash;
        },
        onUnobservable: () => guard.current.unobservable(id),
        // Recorded before it may leave, for every tab and a restart: a split the browser cannot record is not sent.
        onDispatch: (call: SplitDispatch) => {
          recordSale({
            buyer: ctx.buyer.toLowerCase(),
            chainId: saleChain,
            manager: call.manager.toLowerCase(),
            creditId: id,
            creditWei: authorized.credit.amount,
            gapWei: gap.toString(),
            target: call.target.toLowerCase(),
            selector: call.selector,
            callData: call.callData,
            hold: { line: lineId, attempt },
          });
          dispatched = true;
        },
        // And the identity of the exact bytes it leaves as, recorded before they leave: only a revert of those is this sale's.
        onEnvelope: (dispatchedAs: `0x${string}`) => {
          // Its terms come first (`onDispatch`); bytes with none recorded before them are not sent.
          if (!kept.sale) throw new SplitNotRecorded(NOT_RECORDED);
          recordSale({ ...kept.sale, envelope: splitEnvelope(dispatchedAs) });
        },
      };
      const hash =
        target.kind === "trade"
          ? await buyWithCreditsAndMana(ctx, { trade: target.trade, coupon: target.coupon, ...gapArgs })
          : await buyMintWithCreditsAndMana(ctx, { mint: target.mint, ...gapArgs });
      guard.current.submitFinished(id);
      // A successful receipt for the hash the rail returned is no proof on its own (the relay may answer another
      // transaction's hash): it completes only once attributed to this sale, as Check again does; until then it is held,
      // with that hash, and shown confirming.
      const returned: SplitHold = { at: Date.now(), attempt, txHash: hash, chainId: sentChain, creditId: id, creditExpiresAt, sale: kept.sale, itemName };
      if (!(await settledLive(returned))) holdConfirming(returned);
    } catch (e) {
      if (creditId) guard.current.submitFinished(creditId);
      const stillPending = isSettlementPending(e) || isPendingTxError(e) || isMetaTxPending(e);
      // Its own bytes mined and reverted say only that THAT invocation failed: a sibling of the same terms may have run, and
      // a credit the node retired stays spendable until it expires. So the revert releases nothing and retires no credit:
      // the node's record may settle it at once (a replay it settled, or the credit spent elsewhere); if not, it is held.
      if (dispatched && kept.reverted && creditId && kept.sale) {
        const revertedAs: SplitHold = { at: Date.now(), attempt, txHash: kept.reverted, chainId: sentChain, creditId, creditExpiresAt, sale: kept.sale, itemName };
        if (await settledLive(revertedAs)) return;
      }
      // A sale handed off that may have consumed its credit (a broadcast, reverted or not, an answer the relay or the
      // wallet lost) is still confirming: held for this account and item, never offered again until it is found out.
      if (dispatched && (stillPending || (creditId !== null && guard.current.mayBeConsumed(creditId)))) {
        holdConfirming({ at: Date.now(), attempt, txHash: sentHash, chainId: sentChain, creditId, creditExpiresAt, sale: kept.sale, itemName });
        return;
      }
      if (creditId && !guard.current.mayBeConsumed(creditId)) await cancelUsdIntents([creditId], ctx.client).catch(() => 0);
      const message =
        e instanceof SplitNotRecorded
          ? NOT_RECORDED
          : e instanceof SaleCancelledError
            ? e.message
            : e instanceof GrantNotRecordedError ||
                e instanceof ExactGrantUnavailableError ||
                e instanceof GrantCancelledError ||
                e instanceof GrantNotInEffectError ||
                e instanceof GrantUncheckedError
              ? nothingBought(e.message)
            : isAuthorizationPending(e) || (stillPending && !dispatched)
              ? "Your MANA approval is still confirming, so nothing was bought. Try again in a moment."
              : isRejection(e) || isSignatureDeclined(e)
                ? "You cancelled the request in your wallet. Nothing was charged."
                : isRevertedTxError(e)
                  ? APPROVAL_REVERTED
                  : friendlyError(e, FALLBACK, { sale: true });
      // Nothing of this run is left confirming: its own record goes, and its outcome shows once it has (read back). One the
      // storage kept still holds the item, here and on the next open: so it is shown held, for what it is, and Check again
      // lets it go once its record can go.
      if (dispatched && !clearSplitHold(ctx.buyer, lineId, attempt)) {
        setUnsent({ attempt, message });
        setSent(readSplitHoldRecord(ctx.buyer, lineId));
        setNotice(UNCLEARED);
        setPhase({ kind: "pending" });
        return;
      }
      setPhase({ kind: "error", message });
    }
  }

  /**
   * Clears the record Check again settled, under the item's lock and only while it is still that attempt's
   * (clearSettledSplitHold). `cleared`: its outcome may now show. `busy`: the notice says why not. `replaced`: another run
   * settled it and wrote another (shown) or none (the item is offered again).
   */
  async function clearChecked(record: SplitHold, account: string): Promise<"cleared" | "busy" | "replaced"> {
    const clearing = await clearSettledSplitHold(services.locks, account, lineId, record);
    if (clearing.kind === "busy") {
      setNotice(clearing.unremoved ? UNCLEARED : services.locks ? CHECKING_ELSEWHERE : UNCHECKABLE);
      return "busy";
    }
    if (clearing.kind === "replaced") {
      setSent(clearing.current);
      if (!clearing.current && phase.kind === "pending") setPhase({ kind: "review" });
      return "replaced";
    }
    return "cleared";
  }

  /** What Check again does with the record it read, while it holds `checking` throughout. */
  async function checkRecord(record: SplitHold, account: string | null) {
    // This run proved it sent nothing: only its record is left to go, and then its outcome shows.
    if (unsent && record.attempt === unsent.attempt && account) {
      const cleared = await clearChecked(record, account);
      if (cleared === "busy") return;
      setUnsent(null);
      if (cleared === "replaced") return;
      setSent(null);
      setPhase({ kind: "error", message: unsent.message });
      return;
    }
    const settled = await settleSplitHold(record, { account, reader: services.reader, client: services.client, finalizedTime });
    if (!settled || !account) return;
    if (settled.kind === "expiring") {
      // Its credit expired unspent: noted, so the grace runs from now in every tab, and the item stays held until it ends.
      const noted = await noteSplitHoldExpiry(services.locks, account, lineId, record);
      // Read again for its note, the record may still lack the hash this dialog knows (its save failed): that hash stays
      // beside it while it is this very attempt's, buyer's and chain's. A record another run replaced or cleared is taken
      // as it is.
      const after = readSplitHold(account, lineId);
      const sameRun = after && record.txHash && !after.txHash && after.attempt !== null && after.attempt === record.attempt && after.sale?.buyer === record.sale?.buyer && after.chainId === record.chainId;
      setSent(sameRun ? { ...after, txHash: record.txHash } : after);
      setNotice(noted === "busy" ? (services.locks ? CHECKING_ELSEWHERE : UNCHECKABLE) : noted === "noted" ? EXPIRING : null);
      return;
    }
    // Applied under the item's lock, and only to the attempt that was read: another tab or surface may have settled it
    // meanwhile and sent a new split, whose record an outcome of this one never wipes.
    if ((await clearChecked(record, account)) !== "cleared") return;
    setSent(null);
    if (settled.kind === "spent") {
      setPhase({ kind: "success", txHash: settled.txHash });
      finished(settled.txHash);
    } else if (settled.kind === "elsewhere") {
      // Its credit paid for something else: nothing was bought here, and there is no credit left to give back.
      setPhase({ kind: "error", message: SPENT_ELSEWHERE });
    } else if (phase.kind === "pending") {
      setPhase({ kind: "review" });
    }
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
      const auth = phase.auth;
      // Read before any lock is taken: which other splits' credits the chain proves expired, and the listing as it stands (an
      // item gone by now asks nothing of the wallet; the sale reads it again once the approval is in).
      await proveCreditsOver(ctx.buyer);
      const target = await resolveTarget(ctx, agreedTerms.offer);
      const grantName = exactGrantLock(auth, ctx.buyer);
      if (exactGrantLock(spendingAuth(target), ctx.buyer) !== grantName) throw new Error("Start the purchase again to approve.");
      // The grant's lock, taken at the click and kept through the sale's hand-off (`approved`, then `pay`): between this
      // approval and its sale no other window's purchase can start and claim the allowance, so the approve never goes out
      // for a sale that is then refused.
      const release = ctx.locks ? await webLock(ctx.locks, grantName) : null;
      if (ctx.locks && !release) throw new GrantInProgressElsewhereError();
      // What held the approval back under its lock, said instead of a plain "cancelled".
      let heldBack: string | null = null;
      try {
        // Under it, before the wallet is asked: a dialog closed, terms changed, or the session or the wallet's account changed
        // while those reads ran; a hold of this item, another item's split that may still spend the allowance, or purchases
        // this browser can't read.
        if (!stillAgreed(agreedTerms)()) throw new GrantCancelledError();
        const blocked = splitBlock(ctx.buyer, true);
        if (blocked) throw new GrantCancelledError(blocked.message);
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
          grantLockHeld: release !== null,
          // Read after the wallet signs and before anything is recorded or sent (or sent again), under the approval's lock:
          // a dialog closed or terms changed meanwhile leave no grant, and neither does a hold of this item by another
          // purchase, nor another item's split that may still spend the allowance (this approve would set it from under it).
          live: () => {
            heldBack = splitBlock(ctx.buyer, true)?.message ?? null;
            return heldBack === null && stillAgreed(agreedTerms)();
          },
        });
      } catch (e) {
        release?.();
        throw e instanceof GrantCancelledError && heldBack ? new GrantCancelledError(heldBack) : e;
      }
      if (release) handedGrant.current = { release, name: grantName, terms: agreedTerms };
    },
    approved: () => {
      const ctx = context();
      const handed = handedGrant.current;
      handedGrant.current = null;
      if (mounted.current && ctx && terms.current && (!handed || handed.terms === terms.current)) void pay(ctx, terms.current, undefined, handed ?? undefined);
      else handed?.release();
    },
    declined: () => setPhase({ kind: "error", message: "MANA spending was not approved, so nothing was bought." }),
    reset: () => {
      if (!busy) setPhase({ kind: "review" });
    },
    remembered: sent !== null,
    checkable: !!sent && (unsent?.attempt === sent.attempt || splitHoldCheckable(sent, Date.now())),
    checking,
    notice,
    unsent: unsent && sent?.attempt === unsent.attempt ? unsent.message : null,
    checkAgain: async () => {
      // One at a time, from its first read to its last write: a press while one runs does nothing.
      if (checkingNow.current) return;
      setNotice(null);
      const account = wallet.account;
      // Read even past the hold: a purchase that mined in the meantime is completed, never offered a second time.
      const stored = readSplitHoldRecord(account, lineId);
      if (!stored) {
        setSent(null);
        if (phase.kind === "pending") setPhase({ kind: "review" });
        return;
      }
      // A hash this dialog learned but couldn't save (the record kept is the one written before the sale left): read with
      // it while that record is this account's and this very attempt's. Its outcome still applies, under the item's lock,
      // only to that attempt.
      const known = sent && sent.txHash && !stored.txHash && stored.attempt !== null && stored.attempt === sent.attempt && stored.sale?.buyer === account?.toLowerCase();
      const record = known ? { ...stored, txHash: sent.txHash, chainId: sent.chainId } : stored;
      checkingNow.current = true;
      setChecking(true);
      try {
        await checkRecord(record, account);
      } finally {
        checkingNow.current = false;
        setChecking(false);
      }
    },
  };
}

export default function CombinedBuy({ line, offer, mixed, onClose, onDone, portal }: CombinedBuyProps) {
  const { phase, busy, agreed, blocked, start, grant, approved, declined, remembered, checkable, checking, notice, unsent, checkAgain } = useCombinedBuy({ line, offer, mixed, onDone });
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
            <p role="status">{unsent ?? "The purchase was sent and is still confirming. It was not sent twice; check your purchases in a moment."}</p>
            {notice ? (
              <p className="shop-orderbuy__note" role="status" data-testid="combined-check-notice">
                {notice}
              </p>
            ) : null}
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
            {phase.kind === "error" && phase.held ? (
              <p className="shop-orderbuy__note">
                <Link to={phase.held.href} className="shop-orderbuy__link" data-testid="combined-held-item">
                  Open {phase.held.name}
                </Link>
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
