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
  GrantNotInEffectError,
  GrantNotRecordedError,
  isAuthorizationPending,
  needsApprovalStep,
  setAuthorization,
  type ShopAuthorization,
} from "../../checkout/lib/authorizations";
import { friendlyError, isRejection } from "../../checkout/lib/errors";
import { webLock } from "../../checkout/lib/web-lock";
import { isSettlementPending } from "../../checkout/purchase/gasless";
import { purchaseTargetFor, resolveLine, tradeManaPrice } from "../../checkout/purchase/cart";
import { buyMintWithCreditsAndMana, buyWithCreditsAndMana, manaSpenderFor, splitTerms, type PurchaseTarget } from "../../checkout/purchase/mana";
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
  writeSplitHold,
  type SplitHold,
  noteSplitHoldExpiry,
  type SplitSettlement,
} from "../../checkout/purchase/splitHold";
import { splitEnvelope, type SplitSale } from "../../checkout/purchase/splitEvidence";
import { createSpendGuard } from "@shop/checkout/purchase/spend-guard";
import { isMetaTxPending } from "../../checkout/purchase/tx-confirm";
import { useShopServices } from "../../checkout/services";
import type { CartAddInput } from "../../cart/store";
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
/** How often, and how far apart, the sale asks again for the grant's lock right after this tab's own approval let it go. */
const GRANT_LOCK_TRIES = 5;
const GRANT_LOCK_RETRY_MS = 50;

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
   * Reads the sent split's transaction (purchase/splitHold.ts, settleSplitHold): its terms confirmed complete it. Anything
   * else, its own bytes reverted included, leaves it to the node's record of its credit: spent by this split completes it,
   * by another purchase or expired unspent lets the item go, anything else keeps it held. No credit is retired here. Only
   * the attempt it read is cleared, under the item's lock.
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
  /** Whether the purchase agreed on as `agreed` is still this dialog's: open, with those very terms. */
  const stillAgreed = (agreed: Terms) => () => mounted.current && terms.current === agreed;
  // The hold's key: the item, whatever listing this dialog buys it through (another surface may use another).
  const lineId = splitHoldLine(line.asset, line.listing);
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
  const [notice, setNotice] = useState<string | null>(null);
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

  /**
   * The grant's lock, asked for `tries` times a little apart: right after this tab's own approval let it go, the browser
   * frees it a turn later, and that must not read as another window's approval.
   */
  async function grantLock(name: string, tries: number): Promise<(() => void) | null> {
    for (let i = 1; ; i++) {
      const release = await webLock(services.locks, name);
      if (release || i >= tries) return release;
      await new Promise((resolve) => setTimeout(resolve, GRANT_LOCK_RETRY_MS));
    }
  }

  async function pay(ctx: RailContext, agreedTerms: Terms, known?: PurchaseTarget, afterApproval = false) {
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
    const grantRelease = await grantLock(exactGrantLock(spendingAuth(target), ctx.buyer), afterApproval ? GRANT_LOCK_TRIES : 1);
    if (!grantRelease) {
      setPhase({ kind: "error", message: APPROVING_ELSEWHERE });
      return;
    }
    try {
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
    /** Held for this account and item, with what is known of it, and shown confirming. */
    const holdConfirming = (record: SplitHold) => {
      // Should this write fail, the record made before the sale left stays: held, with no hash, until the node settles it.
      writeSplitHold(ctx.buyer, lineId, record);
      setSent(record);
      setPhase({ kind: "pending" });
    };
    /**
     * Applies what the chain and the node say of this run's sale as `record` (settleSplitHold), if that settles it: bought
     * completes it, its credit spent on another purchase lets the item go. Whether it did; anything less is the caller's.
     * The outcome shows only once its record is gone (read back): one the storage kept would show the sale confirming on
     * the next open after this one said it was done. Then it stays held, with what is known of it, and Check again applies
     * the outcome once its record can go.
     */
    const settledLive = async (record: SplitHold): Promise<boolean> => {
      const settled = await settleSplitHold(record, { account: ctx.buyer, reader: ctx.reader, client: ctx.client }).catch(() => null);
      if (settled?.kind !== "spent" && settled?.kind !== "elsewhere") return false;
      if (!clearSplitHold(ctx.buyer, lineId, attempt)) {
        holdConfirming(record);
        setNotice(UNCLEARED);
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
        // A revert of its hash settles nothing on its own (see below): what it says goes to the resolver.
        onReverted: ({ txHash }: { txHash: string | null }) => {
          if (txHash) kept.reverted = txHash;
        },
        onUnobservable: () => guard.current.unobservable(id),
        // Recorded before it may leave, for every tab and a restart: a split the browser cannot record is not sent.
        onDispatch: (call: { manager: string; target: string; selector: `0x${string}`; callData: `0x${string}` }) => {
          kept.sale = {
            buyer: ctx.buyer.toLowerCase(),
            chainId: sentChain!,
            manager: call.manager.toLowerCase(),
            creditId: id,
            creditWei: authorized.credit.amount,
            gapWei: gap.toString(),
            target: call.target.toLowerCase(),
            selector: call.selector,
            callData: call.callData,
            hold: { line: lineId, attempt },
          };
          if (!writeSplitHold(ctx.buyer, lineId, { at: Date.now(), attempt, txHash: null, chainId: sentChain, creditId: id, creditExpiresAt, sale: kept.sale })) {
            throw new SplitNotRecorded(NOT_RECORDED);
          }
          dispatched = true;
        },
        // And the identity of the exact bytes it leaves as, recorded before they leave: only a revert of those is this sale's.
        onEnvelope: (dispatchedAs: `0x${string}`) => {
          kept.sale = { ...kept.sale!, envelope: splitEnvelope(dispatchedAs) };
          if (!writeSplitHold(ctx.buyer, lineId, { at: Date.now(), attempt, txHash: null, chainId: sentChain, creditId: id, creditExpiresAt, sale: kept.sale })) {
            throw new SplitNotRecorded(NOT_RECORDED);
          }
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
      const returned: SplitHold = { at: Date.now(), attempt, txHash: hash, chainId: sentChain, creditId: id, creditExpiresAt, sale: kept.sale };
      if (!(await settledLive(returned))) holdConfirming(returned);
    } catch (e) {
      if (creditId) guard.current.submitFinished(creditId);
      const stillPending = isSettlementPending(e) || isPendingTxError(e) || isMetaTxPending(e);
      // Its own bytes mined and reverted say only that THAT invocation failed: a sibling of the same terms may have run, and
      // a credit the node retired stays spendable until it expires. So the revert releases nothing and retires no credit:
      // the node's record may settle it at once (a replay it settled, or the credit spent elsewhere); if not, it is held.
      if (dispatched && kept.reverted && creditId && kept.sale) {
        const revertedAs: SplitHold = { at: Date.now(), attempt, txHash: kept.reverted, chainId: sentChain, creditId, creditExpiresAt, sale: kept.sale };
        if (await settledLive(revertedAs)) return;
      }
      // A sale handed off that may have consumed its credit (a broadcast, reverted or not, an answer the relay or the
      // wallet lost) is still confirming: held for this account and item, never offered again until it is found out.
      if (dispatched && (stillPending || (creditId !== null && guard.current.mayBeConsumed(creditId)))) {
        holdConfirming({ at: Date.now(), attempt, txHash: sentHash, chainId: sentChain, creditId, creditExpiresAt, sale: kept.sale });
        return;
      }
      if (creditId && !guard.current.mayBeConsumed(creditId)) await cancelUsdIntents([creditId], ctx.client).catch(() => 0);
      // Nothing of this run is left confirming: its own record goes.
      if (dispatched) clearSplitHold(ctx.buyer, lineId, attempt);
      const message =
        e instanceof SplitNotRecorded
          ? NOT_RECORDED
          : e instanceof GrantNotRecordedError || e instanceof ExactGrantUnavailableError || e instanceof GrantCancelledError || e instanceof GrantNotInEffectError
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
        // A dialog closed, or terms changed, while the wallet signs: that approval is recorded and sent no more.
        live: stillAgreed(agreedTerms),
      });
    },
    approved: () => {
      const ctx = context();
      if (mounted.current && ctx && terms.current) void pay(ctx, terms.current, undefined, true);
    },
    declined: () => setPhase({ kind: "error", message: "MANA spending was not approved, so nothing was bought." }),
    reset: () => {
      if (!busy) setPhase({ kind: "review" });
    },
    remembered: sent !== null,
    checkable: !!sent && splitHoldCheckable(sent, Date.now()),
    checking,
    notice,
    checkAgain: async () => {
      setNotice(null);
      const account = wallet.account;
      // Read even past the hold: a purchase that mined in the meantime is completed, never offered a second time.
      const record = readSplitHoldRecord(account, lineId);
      if (!record) {
        setSent(null);
        if (phase.kind === "pending") setPhase({ kind: "review" });
        return;
      }
      setChecking(true);
      let settled: SplitSettlement | null;
      try {
        settled = await settleSplitHold(record, { account, reader: services.reader, client: services.client });
      } finally {
        setChecking(false);
      }
      if (!settled || !account) return;
      if (settled.kind === "expiring") {
        // Its credit expired unspent: noted, so the grace runs from now in every tab, and the item stays held until it ends.
        const noted = await noteSplitHoldExpiry(services.locks, account, lineId, record);
        setSent(readSplitHold(account, lineId));
        setNotice(noted === "busy" ? (services.locks ? CHECKING_ELSEWHERE : UNCHECKABLE) : noted === "noted" ? EXPIRING : null);
        return;
      }
      // Applied under the item's lock, and only to the attempt that was read: another tab or surface may have settled it
      // meanwhile and sent a new split, whose record an outcome of this one never wipes.
      const clearing = await clearSettledSplitHold(services.locks, account, lineId, record);
      if (clearing.kind === "busy") {
        setNotice(clearing.unremoved ? UNCLEARED : services.locks ? CHECKING_ELSEWHERE : UNCHECKABLE);
        return;
      }
      if (clearing.kind === "replaced") {
        setSent(clearing.current);
        if (!clearing.current && phase.kind === "pending") setPhase({ kind: "review" });
        return;
      }
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
    },
  };
}

export default function CombinedBuy({ line, offer, mixed, onClose, onDone, portal }: CombinedBuyProps) {
  const { phase, busy, agreed, blocked, start, grant, approved, declined, remembered, checkable, checking, notice, checkAgain } = useCombinedBuy({ line, offer, mixed, onDone });
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
