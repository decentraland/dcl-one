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

import { useRef, useState } from "react";

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
import { createSpendGuard } from "../../checkout/purchase/spend-guard";
import { isMetaTxPending } from "../../checkout/purchase/tx-confirm";
import { useShopServices } from "../../checkout/services";
import type { CartAddInput } from "../../cart/store";
import { formatMana } from "../../model/money";
import type { CartLine, PurchaseOffer } from "../../model/purchase";
import type { MixedPayment } from "../../payment/present";
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

type Phase =
  | { kind: "review" }
  | { kind: "approve"; auth: ShopAuthorization }
  | { kind: "working" }
  | { kind: "success"; txHash: string | null }
  | { kind: "pending" }
  | { kind: "error"; message: string };

const FALLBACK = "We couldn't complete the purchase. Nothing was charged.";

export default function CombinedBuy({ line, offer, mixed, onClose, onDone, portal }: CombinedBuyProps) {
  const wallet = useShopWallet();
  const services = useShopServices();
  const [phase, setPhase] = useState<Phase>({ kind: "review" });
  const guard = useRef(createSpendGuard());
  const provider = wallet.session.kind === "web3" ? (wallet.host?.provider?.() ?? null) : null;
  const busy = phase.kind === "working";
  const blocked = !provider || !wallet.account ? "Connect your wallet to pay with Credits and MANA." : wallet.accountMismatch ? "Your wallet is on another account than the one you're signed in with." : null;
  const title = `Buy ${line.display.name}`;
  const credits = `${mixed.credits} ${mixed.credits === 1 ? "Credit" : "Credits"}`;
  const mana = `${formatMana(mixed.manaWei, { group: true })} MANA`;

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

  async function resolveTarget(ctx: RailContext): Promise<PurchaseTarget> {
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
    if (!ctx || blocked) return;
    setPhase({ kind: "working" });
    try {
      const target = await resolveTarget(ctx);
      const { spender, chainId } = manaSpenderFor("combined", target);
      const auth: ShopAuthorization = { kind: AuthorizationKind.Allowance, contractAddress: requireContracts(chainId).mana.address, spenderAddress: spender, chainId };
      const allowed = await getAuthorizationStatus(auth, ctx.buyer, mixed.manaWei, ctx.reader).catch(() => false);
      if (!allowed && needsApprovalStep(ctx.walletKind, false)) {
        setPhase({ kind: "approve", auth });
        return;
      }
      await pay(ctx, target);
    } catch (e) {
      setPhase({ kind: "error", message: friendlyError(e, FALLBACK, { sale: true }) });
    }
  }

  async function pay(ctx: RailContext, known?: PurchaseTarget) {
    setPhase({ kind: "working" });
    let creditId: string | null = null;
    try {
      const target = known ?? (await resolveTarget(ctx));
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
        onBroadcast: ({ txHash }: { txHash: string }) => guard.current.broadcast(id, txHash),
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
      setPhase({ kind: "success", txHash: hash });
      onDone?.(hash);
    } catch (e) {
      if (creditId) guard.current.submitFinished(creditId);
      if (isSettlementPending(e) || isPendingTxError(e) || isMetaTxPending(e)) {
        setPhase({ kind: "pending" });
        return;
      }
      if (creditId && !guard.current.mayBeConsumed(creditId)) await cancelUsdIntents([creditId], ctx.client).catch(() => 0);
      const message =
        isRejection(e) || isSignatureDeclined(e)
          ? "You cancelled the request in your wallet. Nothing was charged."
          : isRevertedTxError(e)
            ? "The purchase failed on-chain. Nothing was bought and your Credits were returned."
            : friendlyError(e, FALLBACK, { sale: true });
      setPhase({ kind: "error", message });
    }
  }

  if (phase.kind === "approve") {
    return (
      <AuthorizeStep
        auth={phase.auth}
        requiredWei={mixed.manaWei}
        portal={portal}
        grant={async () => {
          const ctx = context();
          if (!ctx) throw new Error("Connect your wallet again to approve.");
          await setAuthorization({ auth: phase.auth, active: true, provider: ctx.provider, account: ctx.buyer, walletKind: ctx.walletKind, reader: ctx.reader, relayerUrl: ctx.relayerUrl, fetchImpl: ctx.fetchImpl });
        }}
        onAuthorized={() => {
          const ctx = context();
          if (ctx) void pay(ctx);
        }}
        onCancel={() => setPhase({ kind: "error", message: "MANA spending was not approved, so nothing was bought." })}
      />
    );
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
        ) : phase.kind === "pending" ? (
          <>
            <p role="status">The purchase was sent and is still confirming. It was not sent twice; check your purchases in a moment.</p>
            <Button onClick={onClose}>Close</Button>
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
