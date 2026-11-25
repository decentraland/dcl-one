// from: marketplace@3bf3662f:webapp/src/components/AssetPage/SaleActionBox/BuyNFTButtons/BuyNFTButtons.tsx,
//       webapp/src/modules/order/sagas.ts (EXECUTE_ORDER for an order with a tradeId: accept the signed trade)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { Button } from "@dcl/one-ui";
import Modal from "../../../components/Modal";
import { chainLabel, type ChainId } from "@shop/model/chain";
import { formatMana, type ManaWei } from "@shop/model/money";
import { purchaseFailure, type PurchaseFailure } from "../../market/lib/tx";
import { fetchTrade, TRADE_ASSET, type Trade } from "../../market/lib/trades";
import { buyTradeWithMana, type OrderPhase } from "../../market/purchase/executeOrder";
import { useShopWallet } from "../../wallet/context";
import { invalidateAfterPurchase } from "../../checkout/lib/after-purchase";
import ConfirmingNotice from "./ConfirmingNotice";
import "./legacyorderbuy.css";

type LandTradeBuyProps = {
  name: string;
  tradeId: string;
   quoted: ManaWei;
  onClose(): void;
  onDone?(txHash: string): void;
  /** The sale was sent and is still confirming: the dialog stays on its notice, and the page may read the LAND again. */
  onPending?(txHash: string): void;
   loadTrade?: (tradeId: string) => Promise<Trade>;
  /** The LAND on sale, as its page's queries key it: a purchase reads that token again, not every token. */
  land?: { contractAddress: string; tokenId: string };
};

type Phase = { kind: "review" } | { kind: "working"; step: OrderPhase } | { kind: "success"; txHash: string } | PurchaseFailure;

const STEP_TEXT: Record<OrderPhase, string> = {
  approving: "Allow the marketplace to use your MANA in your wallet…",
  confirming: "Confirm the purchase in your wallet…",
  pending: "Waiting for the transaction to confirm…",
};

export function tradeManaPrice(trade: Trade): ManaWei | null {
  const leg = trade.received.find((a) => a.assetType === TRADE_ASSET.ERC20);
  return leg && "amount" in leg ? (BigInt(leg.amount) as ManaWei) : null;
}

export default function LandTradeBuy({ name, tradeId, quoted, onClose, onDone, onPending, loadTrade = fetchTrade, land }: LandTradeBuyProps) {
  const wallet = useShopWallet();
  const qc = useQueryClient();
  const [phase, setPhase] = useState<Phase>({ kind: "review" });
  const [accepted, setAccepted] = useState(false);
  const [trade, setTrade] = useState<{ value: Trade | null; failed: boolean }>({ value: null, failed: false });
  useEffect(() => {
    let live = true;
    loadTrade(tradeId).then(
      (value) => live && setTrade({ value, failed: false }),
      () => live && setTrade({ value: null, failed: true }),
    );
    return () => {
      live = false;
    };
  }, [tradeId, loadTrade]);

  const price = trade.value ? tradeManaPrice(trade.value) : null;
  const chainId = (trade.value?.chainId ?? 1) as ChainId;
  const network = chainLabel(chainId);
  const moved = price !== null && price !== quoted;
  const provider = wallet.session.kind === "web3" ? (wallet.host?.provider?.() ?? null) : null;
  const wrongNetwork = !!provider && wallet.chainId !== null && wallet.chainId !== chainId;
  const blocked = trade.failed
    ? "This listing could not be read. It may have been sold or cancelled."
    : !trade.value
      ? "Reading the listing…"
      : price === null
        ? "This listing is not priced in MANA and cannot be bought here."
        : !provider
          ? "Connect a web3 wallet with MANA to buy this."
          : wallet.accountMismatch
            ? "Your wallet is on another account than the one you're signed in with. Switch it back, or sign in with it."
            : null;
  const ready = !blocked && !wrongNetwork && wallet.chainId === chainId && (!moved || accepted);
  const busy = phase.kind === "working";
  const title = `Buy ${name}`;
  const shown = price ?? quoted;

  async function buy() {
    if (!ready || !provider || busy || !trade.value) return;
    setPhase({ kind: "working", step: "approving" });
    let sent: string | null = null;
    try {
      const hash = await buyTradeWithMana({
        provider,
        trade: trade.value,
        onPhase: (step) => setPhase({ kind: "working", step }),
        onSubmitted: (h) => {
          sent = h;
        },
      });
      setPhase({ kind: "success", txHash: hash });
      // The trade spent MANA: the balances, the token and the buyer's lists are read again.
      invalidateAfterPurchase(qc, land);
      onDone?.(hash);
    } catch (e) {
      const failure = purchaseFailure(e, sent);
      setPhase(failure);
      if (failure.kind === "pending") {
        // The sale sent but not confirmed: the MANA is probably spent, so the balances are read again here too.
        invalidateAfterPurchase(qc, land);
        onPending?.(failure.txHash);
      }
    }
  }

  return (
    <Modal onClose={busy ? undefined : onClose} closeOnBackdrop={!busy} closeOnEsc={!busy} showClose={!busy} width={460} ariaLabel={title}>
      <div className="shop-orderbuy" data-testid="land-trade-buy">
        <h2>{title}</h2>
        {phase.kind === "success" ? (
          <>
            <p role="status">Purchase complete. {name} is on its way to your wallet.</p>
            <Button size="md" onClick={onClose}>Done</Button>
          </>
        ) : phase.kind === "pending" || phase.kind === "approval-pending" ? (
          <ConfirmingNotice outcome={phase} chainId={chainId} onClose={onClose} />
        ) : (
          <>
            <dl className="shop-orderbuy__terms">
              <div>
                <dt>Price</dt>
                <dd data-testid="order-price">{formatMana(shown, { group: true })} MANA</dd>
              </div>
              <div>
                <dt>Network</dt>
                <dd>{network}</dd>
              </div>
              <div>
                <dt>Paid with</dt>
                <dd>MANA from your wallet; the network fee is paid by your wallet.</dd>
              </div>
            </dl>
            {moved ? (
              <label className="shop-orderbuy__moved" role="alert">
                <input type="checkbox" checked={accepted} onChange={(e) => setAccepted(e.target.checked)} disabled={busy} />
                The price changed from {formatMana(quoted, { group: true })} MANA to {formatMana(price!, { group: true })} MANA. Buy at the new price.
              </label>
            ) : null}
            {blocked ? (
              <p className="shop-orderbuy__note" role="status">
                {blocked}
              </p>
            ) : null}
            {!blocked && wrongNetwork ? (
              <div className="shop-orderbuy__note" role="status">
                <p>
                  Your wallet is on {chainLabel(wallet.chainId!)}. This purchase runs on {network}.
                </p>
                <Button size="sm" variant="secondary" disabled={wallet.pendingChainId !== null} onClick={() => void wallet.switchTo(chainId)}>
                  {wallet.pendingChainId !== null ? "Confirm in your wallet…" : `Switch to ${network}`}
                </Button>
              </div>
            ) : null}
            {phase.kind === "working" ? (
              <p className="shop-orderbuy__note" role="status" aria-live="polite">
                {STEP_TEXT[phase.step]}
              </p>
            ) : null}
            {phase.kind === "error" ? (
              <p className="shop-orderbuy__error" role="alert">
                {phase.message}
              </p>
            ) : null}
            <div className="shop-orderbuy__actions">
              <Button size="md" onClick={() => void buy()} disabled={!ready || busy} data-testid="order-buy">
                {busy ? "Working…" : `Buy for ${formatMana(shown, { group: true })} MANA`}
              </Button>
              <Button size="md" variant="outline" onClick={onClose} disabled={busy}>
                Cancel
              </Button>
            </div>
          </>
        )}
      </div>
    </Modal>
  );
}
