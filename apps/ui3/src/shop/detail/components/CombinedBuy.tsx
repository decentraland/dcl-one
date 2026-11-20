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
import { authorizeUsdCredit, cancelUsdIntents, fetchUserPurchases } from "@shop/credits/credits";
import { invalidateAfterPurchase } from "../../checkout/lib/after-purchase";
import { requireContracts } from "../../checkout/chain/contracts";
import { isSignatureDeclined } from "../../checkout/chain/metatx";
import { isPendingTxError, isRevertedTxError } from "../../checkout/chain/wallet";
import AuthorizeStep from "../../checkout/components/AuthorizeStep";
import { AuthorizationKind, getAuthorizationStatus, isAuthorizationPending, needsApprovalStep, setAuthorization, type ShopAuthorization } from "../../checkout/lib/authorizations";
import { friendlyError, isRejection } from "../../checkout/lib/errors";
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
// since a sale whose outcome is unknown is exactly when the buyer opens the item again elsewhere. The run writes it itself,
// before the sale may leave, so it is there however the split was started (Buy now's dialog or the page's) and even when
// its screen is gone by then (an account switched away remounts the dialog). It holds a second split off for 15 minutes,
// or until Check again reads the receipt.
const SENT_KEY = "dcl-shop-buynow-split:v1:";
export const SENT_HOLD_MS = 15 * 60 * 1000;
const NOT_RECORDED = "Your browser couldn't keep a record of this purchase, so it wasn't sent. Nothing was charged.";

/** The split could not be recorded before leaving, so it was not sent. */
class SplitNotRecorded extends Error {}

/** Another tab or window of this browser sent a split for this account and item that is still held. */
class SplitSentElsewhere extends Error {
  constructor(readonly sent: SentSplit) {
    super("A purchase of this item is already confirming.");
  }
}

/** The storage every tab of this browser shares; none on a page without it, or where the browser blocks it. */
function holdStorage(): Storage | null {
  try {
    return typeof localStorage !== "undefined" ? localStorage : null;
  } catch {
    return null;
  }
}

/**
 * `creditId`: the credit the split reserved. When Check again finds the purchase reverted it retires that reservation, as
 * the live path does; the Credits then come back with the expiry sweep. `creditExpiresAt` (unix seconds): when that
 * credit stops being spendable. A split sent with no hash to read (the relay lost its answer) is let go only once that
 * credit has expired and the node says it was released unspent; one it says was spent completes.
 */
export type SentSplit = { at: number; txHash: string | null; chainId: number | null; creditId: string | null; creditExpiresAt: number | null };

const sentKey = (account: string | null, lineId: string) => `${SENT_KEY}${account?.toLowerCase() ?? "anonymous"}:${lineId}`;

/** The record as stored, held or not. A bare number is the first format's (a timestamp, written before the hash was kept). */
function readSentRecord(account: string | null, lineId: string): SentSplit | null {
  try {
    const raw = holdStorage()?.getItem(sentKey(account, lineId)) ?? null;
    const parsed = raw ? (JSON.parse(raw) as unknown) : null;
    if (typeof parsed === "number") return { at: parsed, txHash: null, chainId: null, creditId: null, creditExpiresAt: null };
    const sent = parsed as Partial<SentSplit> | null;
    if (!sent || typeof sent.at !== "number") return null;
    return {
      at: sent.at,
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
/** The credit a split with no hash spent, or not, can no longer be spent: only then can the node's record settle it. */
const creditOver = (sent: SentSplit, now: number) => sent.creditExpiresAt === null || now >= sent.creditExpiresAt * 1000;
/** When Check again next has something to do for a split with no hash: its hold and its credit both over. */
const releasableAt = (sent: SentSplit) => Math.max(sent.at + SENT_HOLD_MS, (sent.creditExpiresAt ?? 0) * 1000);

/**
 * The split this account sent for this line, while it is held: recorded no later than now and no longer ago than the hold;
 * and one sent with no hash to read whose credit is known, until Check again finds out what became of that credit.
 */
export function readSentSplit(account: string | null, lineId: string, now: number = Date.now()): SentSplit | null {
  const sent = readSentRecord(account, lineId);
  return sent && (held(sent, now) || (!sent.txHash && !!sent.creditId)) ? sent : null;
}

/** Whether the record was kept (written and read back): a sale is sent only once it is. */
function writeSentSplit(account: string | null, lineId: string, sent: SentSplit): boolean {
  try {
    const storage = holdStorage();
    if (!storage) return false;
    const key = sentKey(account, lineId);
    const value = JSON.stringify(sent);
    storage.setItem(key, value);
    return storage.getItem(key) === value;
  } catch {
    return false;
  }
}

function clearSentSplit(account: string | null, lineId: string): void {
  try {
    holdStorage()?.removeItem(sentKey(account, lineId));
  } catch {
    // Nothing was stored.
  }
}

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
   * Check again has something to do: a transaction to read, or a hold that has run out (it then lets the item go). A split
   * recorded with no hash (a MANA approval still confirming, or a record in the first, timestamp-only format) never has
   * one to read, so it becomes checkable only once its hold runs out. The purchase itself always records its hash: it is
   * broadcast before anything can leave it pending.
   */
  checkable: boolean;
  checking: boolean;
  /**
   * Reads the sent split's receipt: confirmed completes it, reverted fails it (retiring its credit), and either releases
   * the hold. With no receipt, or nothing to read, a hold that has run out is let go; a failed read keeps it held.
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
    };
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
    const agreedTerms: Terms = { offer, mixed };
    terms.current = agreedTerms;
    setAgreed(mixed);
    setPhase({ kind: "working" });
    try {
      const target = await resolveTarget(ctx, agreedTerms.offer);
      const { spender, chainId } = manaSpenderFor("combined", target);
      const auth: ShopAuthorization = { kind: AuthorizationKind.Allowance, contractAddress: requireContracts(chainId).mana.address, spenderAddress: spender, chainId };
      const allowed = await getAuthorizationStatus(auth, ctx.buyer, agreedTerms.mixed.manaWei, ctx.reader).catch(() => false);
      if (!allowed && needsApprovalStep(ctx.walletKind, false)) {
        setPhase({ kind: "approve", auth });
        return;
      }
      await pay(ctx, agreedTerms, target);
    } catch (e) {
      setPhase({ kind: "error", message: friendlyError(e, FALLBACK, { sale: true }) });
    }
  }

  async function pay(ctx: RailContext, { offer, mixed }: Terms, known?: PurchaseTarget) {
    setPhase({ kind: "working" });
    let creditId: string | null = null;
    let creditExpiresAt: number | null = null;
    let sentHash: string | null = null;
    let sentChain: number | null = null;
    // Set once the sale itself goes to the relay or the wallet: before it, a failure is the MANA approval's, and no sale left.
    let dispatched = false;
    try {
      const target = known ?? (await resolveTarget(ctx, offer));
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
        // Recorded before it may leave, for every tab and a restart; checked against another tab's first, in the same
        // turn. A split already held, or one the browser cannot record, is not sent.
        onDispatch: () => {
          const other = readSentSplit(ctx.buyer, lineId);
          if (other) throw new SplitSentElsewhere(other);
          if (!writeSentSplit(ctx.buyer, lineId, { at: Date.now(), txHash: null, chainId: sentChain, creditId: id, creditExpiresAt })) {
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
      clearSentSplit(ctx.buyer, lineId);
      setSent(null);
      setPhase({ kind: "success", txHash: hash });
      finished(hash);
    } catch (e) {
      if (creditId) guard.current.submitFinished(creditId);
      const stillPending = isSettlementPending(e) || isPendingTxError(e) || isMetaTxPending(e);
      // A sale handed off that may have consumed its credit (a broadcast not seen reverting, or an answer the relay lost)
      // is still confirming: held for this account and item, never offered again until it is found out.
      if (dispatched && (stillPending || (creditId !== null && guard.current.mayBeConsumed(creditId)))) {
        // Recorded for the account that sent it, whatever the wallet shows by now, with its hash when there is one. Should
        // this write fail, the record made before it left stays: held, with no hash, until the node settles its credit.
        const record: SentSplit = { at: Date.now(), txHash: sentHash, chainId: sentChain, creditId, creditExpiresAt };
        writeSentSplit(ctx.buyer, lineId, record);
        setSent(record);
        setPhase({ kind: "pending" });
        return;
      }
      if (creditId && !guard.current.mayBeConsumed(creditId)) await cancelUsdIntents([creditId], ctx.client).catch(() => 0);
      // Nothing of this run is left confirming: its own record goes. Another tab's, which stopped it, stays and is shown.
      if (dispatched) clearSentSplit(ctx.buyer, lineId);
      if (e instanceof SplitSentElsewhere) {
        setSent(e.sent);
        setPhase({ kind: "review" });
        return;
      }
      const message =
        e instanceof SplitNotRecorded
          ? NOT_RECORDED
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
      // Read even past the hold: a purchase that mined in the meantime is completed, never offered a second time.
      const record = readSentRecord(wallet.account, lineId);
      const release = () => {
        clearSentSplit(wallet.account, lineId);
        setSent(null);
      };
      if (!record) {
        setSent(null);
        if (phase.kind === "pending") setPhase({ kind: "review" });
        return;
      }
      if (record.txHash && record.chainId !== null) {
        setChecking(true);
        try {
          const receipt = await services.reader.receipt(record.chainId, record.txHash);
          if (receipt) {
            release();
            if (receipt.status === "reverted") {
              // The only purchase this credit went into reverted, so nothing consumed it: retire it now, as the live path
              // does. Retired credits come back with the expiry sweep, not at once, which is what the copy says.
              if (record.creditId) await cancelUsdIntents([record.creditId], services.client).catch(() => 0);
              setPhase({ kind: "error", message: REVERTED });
            } else {
              setPhase({ kind: "success", txHash: record.txHash });
              finished(record.txHash);
            }
            return;
          }
        } catch {
          // A failed read is not an answer: still confirming.
          return;
        } finally {
          setChecking(false);
        }
      }
      // No receipt (or nothing to read): it stays held while the hold lasts.
      if (held(record, Date.now())) return;
      if (!record.txHash && record.creditId) {
        // Nothing to read on chain (the relay lost its answer): fifteen minutes prove nothing. Whether the credit was spent
        // decides, asked of the node once it can no longer be: spent completes the split, released lets the item go,
        // anything else (still reserved, not listed, an unanswered read) keeps it held.
        if (!creditOver(record, Date.now()) || !wallet.account) return;
        setChecking(true);
        try {
          const { items } = await fetchUserPurchases(wallet.account, { all: true, first: 50 }, services.client);
          // The node lists a credit by its lines (`<salt>:<position>`); the split's is one line.
          const credit = items.find((p) => p.id.split(":")[0]!.toLowerCase() === record.creditId!.toLowerCase());
          if (credit?.status === "SETTLED") {
            release();
            setPhase({ kind: "success", txHash: credit.txHash });
            finished(credit.txHash);
          } else if (credit?.status === "EXPIRED") {
            release();
            if (phase.kind === "pending") setPhase({ kind: "review" });
          }
        } catch {
          // A failed read is not an answer: still held.
        } finally {
          setChecking(false);
        }
        return;
      }
      // A broadcast whose receipt never came, or a record from before credits were kept with it: let go after the hold.
      release();
      if (phase.kind === "pending") setPhase({ kind: "review" });
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
