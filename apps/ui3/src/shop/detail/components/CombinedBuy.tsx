// from: shop@94584a5c:app/src/components/BuyModal/BuyModal.tsx (confirmCombined: the credit sized to the
//       balance, the MANA gap in one useCredits, releaseReservation behind the spend guard),
//       app/src/lib/buy-mana.ts (buyWithCreditsAndMana, buyMintWithCreditsAndMana)
// Upstream's credits+MANA rail, kept INSIDE the MANA option of an eligible primary wearable or emote for a
// web3 wallet (SHOP.md): the whole credits the balance holds first, then MANA for the remainder, in ONE
// CreditsManager.useCredits. The MANA allowance for the manager is approved first (explicitly, for a
// self-custody wallet), so no credit is held while the buyer decides; then ONE credit is authorized for
// the credits leg, the MANA the contract would pull is checked against the amount shown, and the call is
// relayed (or sent by the wallet when the relay cannot run). A credit is released only when nothing may
// have consumed it; a broadcast that has not confirmed is pending, never re-sent.

import { useEffect, useRef, useState } from "react";

import Button from "../../../atoms/Button";
import Modal from "../../../components/Modal";
import { authorizeUsdCredit, cancelUsdIntents } from "../../credits/credits";
import { requireContracts } from "../../checkout/chain/contracts";
import { isSignatureDeclined } from "../../checkout/chain/metatx";
import { isPendingTxError, isRevertedTxError } from "../../checkout/chain/wallet";
import AuthorizeStep from "../../checkout/components/AuthorizeStep";
import { AuthorizationKind, getAuthorizationStatus, needsApprovalStep, setAuthorization, type ShopAuthorization } from "../../checkout/lib/authorizations";
import { friendlyError, isRejection } from "../../checkout/lib/errors";
import { isSettlementPending } from "../../checkout/purchase/gasless";
import { purchaseTargetFor, resolveLine } from "../../checkout/purchase/cart";
import { buyMintWithCreditsAndMana, buyWithCreditsAndMana, manaSpenderFor, type PurchaseTarget } from "../../checkout/purchase/mana";
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

// A split sent and not confirmed yet, per account and item, in this tab. The run writes it itself, so it is there however
// the split was started (Buy now's dialog or the page's) and even when its screen is gone by then (an account switched
// away remounts the dialog). It holds a second split off for 15 minutes, or until Check again reads the receipt.
const SENT_KEY = "dcl-shop-buynow-split:v1:";
export const SENT_HOLD_MS = 15 * 60 * 1000;

/**
 * `creditId`: the credit the split reserved. When Check again finds the purchase reverted it retires that reservation, as
 * the live path does; the Credits then come back with the expiry sweep.
 */
export type SentSplit = { at: number; txHash: string | null; chainId: number | null; creditId: string | null };

const sentKey = (account: string | null, lineId: string) => `${SENT_KEY}${account?.toLowerCase() ?? "anonymous"}:${lineId}`;

/** The record as stored, held or not. A bare number is the first format's (a timestamp, written before the hash was kept). */
function readSentRecord(account: string | null, lineId: string): SentSplit | null {
  try {
    const raw = sessionStorage.getItem(sentKey(account, lineId));
    const parsed = raw ? (JSON.parse(raw) as unknown) : null;
    if (typeof parsed === "number") return { at: parsed, txHash: null, chainId: null, creditId: null };
    const sent = parsed as Partial<SentSplit> | null;
    if (!sent || typeof sent.at !== "number") return null;
    return {
      at: sent.at,
      txHash: typeof sent.txHash === "string" ? sent.txHash : null,
      chainId: typeof sent.chainId === "number" ? sent.chainId : null,
      creditId: typeof sent.creditId === "string" ? sent.creditId : null,
    };
  } catch {
    return null;
  }
}

const held = (sent: SentSplit, now: number) => now >= sent.at && now - sent.at < SENT_HOLD_MS;

/** The split this account sent for this line, while it is held: recorded no later than now and no longer ago than the hold. */
export function readSentSplit(account: string | null, lineId: string, now: number = Date.now()): SentSplit | null {
  const sent = readSentRecord(account, lineId);
  return sent && held(sent, now) ? sent : null;
}

function writeSentSplit(account: string | null, lineId: string, sent: SentSplit): void {
  try {
    sessionStorage.setItem(sentKey(account, lineId), JSON.stringify(sent));
  } catch {
    // Without storage the screen still shows it while open.
  }
}

function clearSentSplit(account: string | null, lineId: string): void {
  try {
    sessionStorage.removeItem(sentKey(account, lineId));
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
  /** A split for this account and line was sent and is not known to be confirmed yet (this run's, or an earlier one). */
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
    const left = sent.at + SENT_HOLD_MS - Date.now();
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
    let sentHash: string | null = null;
    let sentChain: number | null = null;
    try {
      const target = known ?? (await resolveTarget(ctx, offer));
      sentChain = manaSpenderFor("combined", target).chainId;
      const item = line.asset.token.kind === "item" ? { contractAddress: line.asset.contract, itemId: line.asset.token.itemId } : undefined;
      const authorized = await authorizeUsdCredit(
        mixed.creditsCents,
        { ...(target.kind === "trade" ? { tradeId: target.trade.id } : {}), ...(item ? { item } : {}) },
        ctx.client,
      );
      creditId = authorized.credit.id;
      const price = target.kind === "trade" ? null : BigInt(target.mint.item.priceWei);
      const gap = price === null ? mixed.manaWei : price - BigInt(authorized.credit.availableAmount);
      if (gap > mixed.manaWei) throw new Error("The MANA this needs moved past the amount shown. Nothing was charged; check the price again.");
      const id = creditId;
      guard.current.submitStarted(id);
      const gapArgs = {
        credits: [{ ...authorized.credit }],
        manaGapWei: gap > 0n ? gap : mixed.manaWei,
        onBroadcast: ({ txHash }: { txHash: string }) => {
          sentHash = txHash;
          guard.current.broadcast(id, txHash);
        },
        onReverted: ({ txHash }: { txHash: string | null }) => {
          if (txHash) guard.current.reverted(txHash);
        },
        onUnobservable: () => guard.current.unobservable(id),
      };
      const hash =
        target.kind === "trade"
          ? await buyWithCreditsAndMana(ctx, { trade: target.trade, coupon: target.coupon, ...gapArgs })
          : await buyMintWithCreditsAndMana(ctx, { mint: target.mint, ...gapArgs });
      guard.current.submitFinished(id);
      clearSentSplit(ctx.buyer, lineId);
      setSent(null);
      setPhase({ kind: "success", txHash: hash });
      onDone?.(hash);
    } catch (e) {
      if (creditId) guard.current.submitFinished(creditId);
      if (isSettlementPending(e) || isPendingTxError(e) || isMetaTxPending(e)) {
        // Recorded for the account that sent it, whatever the wallet shows by now.
        const record = { at: Date.now(), txHash: sentHash, chainId: sentChain, creditId };
        writeSentSplit(ctx.buyer, lineId, record);
        setSent(record);
        setPhase({ kind: "pending" });
        return;
      }
      if (creditId && !guard.current.mayBeConsumed(creditId)) await cancelUsdIntents([creditId], ctx.client).catch(() => 0);
      const message =
        isRejection(e) || isSignatureDeclined(e)
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
      await setAuthorization({ auth: phase.auth, active: true, provider: ctx.provider, account: ctx.buyer, walletKind: ctx.walletKind, reader: ctx.reader, relayerUrl: ctx.relayerUrl, fetchImpl: ctx.fetchImpl });
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
    // A receipt to read, or a hold that has run out (Check again then lets the item go).
    checkable: !!sent && ((!!sent.txHash && sent.chainId !== null) || !held(sent, Date.now())),
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
              onDone?.(record.txHash);
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
      // No receipt (or nothing to read): it stays held while the hold lasts, and is let go once it has run out.
      if (!held(record, Date.now())) {
        release();
        if (phase.kind === "pending") setPhase({ kind: "review" });
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
