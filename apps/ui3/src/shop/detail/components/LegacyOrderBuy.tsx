// from: marketplace@f3956826:webapp/src/components/AssetPage/SaleActionBox/BuyNFTButtons/BuyNFTButtons.tsx,
//       webapp/src/components/BuyPage/PriceHasChanged/PriceHasChanged.tsx, webapp/src/modules/order/sagas.ts
//       (EXECUTE_ORDER), webapp/src/modules/vendor/decentraland/OrderService.ts (execute)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
// Buying a token listed on the legacy on-chain order contract (LAND parcels and estates, NAME resales,
// classic wearable resales) from its token page. MANA only: no credit price, no top-up, no credits+MANA.
// The cart cannot settle a single-asset order call, so this is the token page's own purchase: a web3
// wallet on the signed account and on the order's network, the MANA allowance for the legacy
// marketplace, then `executeOrder` at the order's exact price. A price that moved since the page's
// quote is shown and must be accepted; a broadcast that has not confirmed is pending, never re-sent.

import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import type { Hex } from "viem";

import { invalidateAfterPurchase } from "../../checkout/lib/after-purchase";
import Button from "../../../atoms/Button";
import Modal from "../../../components/Modal";
import { chainLabel, type ChainId } from "@shop/model/chain";
import { formatMana, type ManaWei } from "@shop/model/money";
import { APPROVAL_STILL_PENDING, purchaseFailure, type PurchaseFailure, type WaitOptions } from "../../market/lib/tx";
import { executeLegacyOrder, type OrderPhase } from "../../market/purchase/executeOrder";
import { useShopWallet } from "../../wallet/context";
import { landCategoryOf } from "@shop/land/contracts";
import { readEstateFingerprint } from "../../land/estate";
import { rpcReader, type ChainReader } from "../../land/evm";
import type { LegacyOrderRef } from "@shop/detail/api";
import "./legacyorderbuy.css";

export type LegacyOrderBuyProps = {
  name: string;
  contractAddress: string;
  tokenId: string;
  order: LegacyOrderRef;
  /** The server's MANA quote the page showed; the order's own price is what settles. */
  quoted: ManaWei | null;
  onClose(): void;
  onDone?(txHash: string): void;
  /** The sale was sent and is still confirming: the dialog stays on its notice, and the page may read the token again. */
  onPending?(txHash: string): void;
  wait?: WaitOptions;
  portal?: boolean;
  /** Reads the Estate registry (defaults to the node's read relay for the order's chain). */
  reader?: ChainReader;
};

type Phase = { kind: "review" } | { kind: "working"; step: OrderPhase } | { kind: "success"; txHash: string } | PurchaseFailure;

const STEP_TEXT: Record<OrderPhase, string> = {
  approving: "Allow the marketplace to use your MANA in your wallet…",
  confirming: "Confirm the purchase in your wallet…",
  pending: "Waiting for the transaction to confirm…",
};

export default function LegacyOrderBuy({ name, contractAddress, tokenId, order, quoted, onClose, onDone, onPending, wait, portal, reader }: LegacyOrderBuyProps) {
  const wallet = useShopWallet();
  const qc = useQueryClient();
  const [phase, setPhase] = useState<Phase>({ kind: "review" });
  const [accepted, setAccepted] = useState(false);
  // An Estate is bought bound to the composition reviewed now: its registry fingerprint, read on open.
  const estate = landCategoryOf(contractAddress)?.category === "estate";
  const [fingerprint, setFingerprint] = useState<{ value: Hex | null; failed: boolean }>({ value: null, failed: false });
  useEffect(() => {
    if (!estate) return;
    const ctrl = new AbortController();
    let r: ChainReader;
    try {
      r = reader ?? rpcReader(order.chainId);
    } catch {
      setFingerprint({ value: null, failed: true });
      return;
    }
    readEstateFingerprint(r, contractAddress, tokenId, ctrl.signal).then(
      (value) => !ctrl.signal.aborted && setFingerprint({ value, failed: false }),
      () => !ctrl.signal.aborted && setFingerprint({ value: null, failed: true }),
    );
    return () => ctrl.abort();
  }, [estate, reader, order.chainId, contractAddress, tokenId]);
  const chainId = order.chainId as ChainId;
  const network = chainLabel(chainId);
  const price = BigInt(order.price) as ManaWei;
  const moved = quoted !== null && quoted !== price;
  const provider = wallet.session.kind === "web3" ? (wallet.host?.provider?.() ?? null) : null;
  const wrongNetwork = !!provider && wallet.chainId !== null && wallet.chainId !== chainId;
  const blocked = !provider
    ? "Connect a web3 wallet with MANA to buy this."
    : wallet.accountMismatch
      ? "Your wallet is on another account than the one you're signed in with. Switch it back, or sign in with it."
      : estate && fingerprint.failed
        ? "We couldn't read this Estate's parcels to confirm what you are buying. Try again in a moment."
        : estate && !fingerprint.value
          ? "Reading this Estate's parcels…"
          : null;
  const ready = !blocked && !wrongNetwork && wallet.chainId === chainId && (!moved || accepted);
  const busy = phase.kind === "working";
  const title = `Buy ${name}`;

  async function buy() {
    if (!ready || !provider || busy) return;
    setPhase({ kind: "working", step: "approving" });
    let sent: string | null = null;
    try {
      const hash = await executeLegacyOrder({
        provider,
        order: { contractAddress, tokenId, price: order.price, chainId: order.chainId, marketplaceAddress: order.marketplaceAddress },
        ...(estate && fingerprint.value ? { fingerprint: fingerprint.value } : {}),
        onPhase: (step) => setPhase({ kind: "working", step }),
        onSubmitted: (h) => {
          sent = h;
        },
        wait,
      });
      setPhase({ kind: "success", txHash: hash });
      // The order spent MANA: the balances, the token and the buyer's lists are read again.
      invalidateAfterPurchase(qc, { contractAddress, tokenId });
      onDone?.(hash);
    } catch (e) {
      const failure = purchaseFailure(e, sent);
      setPhase(failure);
      if (failure.kind === "pending") {
        // The sale sent but not confirmed: the MANA is probably spent, so the balances are read again here too.
        invalidateAfterPurchase(qc, { contractAddress, tokenId });
        onPending?.(failure.txHash);
      }
    }
  }

  return (
    <Modal onClose={busy ? undefined : onClose} closeOnBackdrop={!busy} closeOnEsc={!busy} showClose={!busy} width={460} portal={portal} ariaLabel={title}>
      <div className="shop-orderbuy" data-testid="legacy-order-buy">
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
        ) : phase.kind === "approval-pending" ? (
          <>
            <p role="status">{APPROVAL_STILL_PENDING}</p>
            <Button onClick={onClose}>Close</Button>
          </>
        ) : (
          <>
            <dl className="shop-orderbuy__terms">
              <div>
                <dt>Price</dt>
                <dd data-testid="order-price">{formatMana(price, { group: true })} MANA</dd>
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
                The price changed from {formatMana(quoted!, { group: true })} MANA to {formatMana(price, { group: true })} MANA. Buy at the new price.
              </label>
            ) : null}
            {blocked ? <p className="shop-orderbuy__note" role="status">{blocked}</p> : null}
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
                {busy ? "Working…" : `Buy for ${formatMana(price, { group: true })} MANA`}
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
