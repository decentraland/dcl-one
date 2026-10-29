// from: marketplace@3bf3662f:webapp/src/components/AssetPage/SaleActionBox/BuyNFTButtons/BuyNFTButtons.tsx,
//       webapp/src/modules/order/sagas.ts (EXECUTE_ORDER for an order with a tradeId: accept the signed trade)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { useEffect, useState } from "react";

import Button from "../../../atoms/Button";
import Modal from "../../../components/Modal";
import { chainLabel, type ChainId } from "../../model/chain";
import { formatMana, type ManaWei } from "../../model/money";
import { declined, isTxPending, isTxReverted } from "../../market/lib/tx";
import { fetchTrade, TRADE_ASSET, type Trade } from "../../market/lib/trades";
import { buyTradeWithMana, type OrderPhase } from "../../market/purchase/executeOrder";
import { useShopWallet } from "../../wallet/context";
import "./legacyorderbuy.css";

type LandTradeBuyProps = {
  name: string;
  tradeId: string;
   quoted: ManaWei;
  onClose(): void;
  onDone?(txHash: string): void;
   loadTrade?: (tradeId: string) => Promise<Trade>;
};

type Phase =
  { kind: "review" } | { kind: "working"; step: OrderPhase } | { kind: "success"; txHash: string } | { kind: "pending"; txHash: string } | { kind: "error"; message: string };

const STEP_TEXT: Record<OrderPhase, string> = {
  approving: "Allow the marketplace to use your MANA in your wallet…",
  confirming: "Confirm the purchase in your wallet…",
  pending: "Waiting for the transaction to confirm…",
};

export function tradeManaPrice(trade: Trade): ManaWei | null {
  const leg = trade.received.find((a) => a.assetType === TRADE_ASSET.ERC20);
  return leg && "amount" in leg ? (BigInt(leg.amount) as ManaWei) : null;
}

export default function LandTradeBuy({ name, tradeId, quoted, onClose, onDone, loadTrade = fetchTrade }: LandTradeBuyProps) {
  const wallet = useShopWallet();
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
      onDone?.(hash);
    } catch (e) {
      if (isTxPending(e)) setPhase({ kind: "pending", txHash: (e as { txHash: string }).txHash });
      else if (declined(e)) setPhase({ kind: "error", message: "You cancelled the request in your wallet. Nothing was bought." });
      else if (isTxReverted(e)) setPhase({ kind: "error", message: "The purchase failed on-chain: the listing may have been sold or cancelled. Nothing was bought." });
      else if (sent) setPhase({ kind: "pending", txHash: sent });
      else setPhase({ kind: "error", message: (e as Error)?.message || "We couldn't complete the purchase. Nothing was bought." });
    }
  }

  return (
    <Modal onClose={busy ? undefined : onClose} closeOnBackdrop={!busy} closeOnEsc={!busy} showClose={!busy} width={460} ariaLabel={title}>
      <div className="shop-orderbuy" data-testid="land-trade-buy">
        <h2>{title}</h2>
        {phase.kind === "success" ? (
          <>
            <p role="status">Purchase complete. {name} is on its way to your wallet.</p>
            <Button onClick={onClose}>Done</Button>
          </>
        ) : phase.kind === "pending" ? (
          <>
            <p role="status">The purchase was sent and is still confirming. It was not sent twice; check your activity in a moment.</p>
            <Button onClick={onClose}>Close</Button>
          </>
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
              <Button onClick={() => void buy()} disabled={!ready || busy} data-testid="order-buy">
                {busy ? "Working…" : `Buy for ${formatMana(shown, { group: true })} MANA`}
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
