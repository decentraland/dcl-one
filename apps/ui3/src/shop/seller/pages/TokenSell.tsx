// from: marketplace@3bf3662f:webapp/src/components/SellPage (SellPage.tsx, SellPage.css, SellModal/SellModal.tsx),
//       webapp/src/components/ConfirmInputValueModal, webapp/src/modules/order/utils.ts (getDefaultExpirationDate),
//       webapp/src/modules/translation/locales/en.json (sell_page.*); shop@2d3bf62:app/src/lib/trades.ts;
//       catalyrst-v2 sites/src/features/stories/marketplace/sell-list (the v2 sell wizard's steps)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
import { useQueryClient } from "@tanstack/react-query";
import { useId, useMemo, useState } from "react";
import { Link, useNavigate } from "react-router";

import ManaMark from "../../../atoms/ManaMark";
import Modal from "../../../components/Modal";
import { isPolygon, type ChainId } from "../../model/chain";
import { formatMana } from "../../model/money";
import { shopPath } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { fetchLegacyOrders } from "../../market/lib/api";
import { markListingCancelled } from "../../market/lib/dead-listings";
import { cancelLegacyOrder } from "../../market/purchase/executeOrder";
import { parseManaInput } from "../../market/lib/mana";
import { cancelTrade, createManaListing, ensureApproval, fetchTrade } from "../../market/lib/trades";
import { EnsureQueryClient, NetworkNotice, useShopProvider, useWrite, WalletGate, WriteStatus } from "../../market/components/kit";
import { listingLabel } from "../../market/components/AssetTile";
import { TokenFrame, useIsOwner, WithRouteToken, type TokenRecord } from "../../market/components/TokenAction";
import { postListingWithRetry } from "../lib/import";
import { patchManageCaches } from "../lib/manage-cache";
import { useShopCapabilities } from "../lib/featureFlags";
import { MarketSignIn } from "./SellerSignIn";
import "./sellerpages.css";

const DAY = 86_400_000;
const DEFAULT_EXPIRATION_DAYS = 30;

export function dateInputValue(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function startOfInputDate(value: string): number {
  const [y, m, d] = value.split("-").map(Number);
  return y && m && d ? new Date(y, m - 1, d).getTime() : NaN;
}

type Step = "approve" | "cancel" | "sign" | "publish";
const STEP_TEXT: Record<Step, string> = {
  cancel: "Take the current listing down",
  approve: "Allow the marketplace to transfer items of this collection (once)",
  sign: "Sign the listing in your wallet",
  publish: "Publish it in the Shop",
};

export function SellForm({ token, now = Date.now }: { token: TokenRecord; now?: () => number }) {
  const id = useId();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const { account } = useShopWallet();
  const provider = useShopProvider();
  const relist = token.isOnSale;
  const current = token.listing?.price?.kind === "mana" ? token.listing.price.wei : null;
  const [price, setPrice] = useState(() => (relist && current ? formatMana(current) : ""));
  const [expiresAt, setExpiresAt] = useState(() => dateInputValue(now() + DEFAULT_EXPIRATION_DAYS * DAY));
  const [confirming, setConfirming] = useState(false);
  const [confirmed, setConfirmed] = useState("");
  const [step, setStep] = useState<Step | null>(null);
  const write = useWrite();
  const amount = useMemo(() => parseManaInput(price), [price]);
  const expiresAtMs = startOfInputDate(expiresAt);
  const invalidDate = !Number.isFinite(expiresAtMs) || expiresAtMs < now();
  const steps: Step[] = relist ? ["cancel", "approve", "sign", "publish"] : ["approve", "sign", "publish"];
  const back = () => navigate(shopPath("tokenDetail", { contractAddress: token.contractAddress, tokenId: token.tokenId }));

  if (write.state.kind === "done" && amount) {
    return (
      <div className="mk-section" data-testid="shop-sell-done">
        <p role="status">
          “{token.name}” is on sale for {formatMana(amount, { group: true })} MANA.
        </p>
        <Link className="mk-link" to={shopPath("tokenDetail", { contractAddress: token.contractAddress, tokenId: token.tokenId })}>
          View it in the Shop
        </Link>
      </div>
    );
  }

  const submit = () => {
    if (!amount || invalidDate) return;
    setConfirming(false);
    setConfirmed("");
    void write.run("Preparing your listing…", "Listed.", async (progress) => {
      const go = (s: Step) => {
        setStep(s);
        progress(`${STEP_TEXT[s]}…`);
      };
      if (relist) {
        go("cancel");
        if (token.listing?.tradeId) {
          const old = await fetchTrade(token.listing.tradeId);
          await cancelTrade({ provider: provider!, trade: old });
          markListingCancelled(qc, old.id);
        } else {
          const [order] = await fetchLegacyOrders({ contractAddress: token.contractAddress, tokenId: token.tokenId, owner: token.owner });
          if (order) await cancelLegacyOrder({ provider: provider!, order });
        }
      }
      go("approve");
      await ensureApproval({ provider: provider!, contractAddress: token.contractAddress, chainId: token.chainId as ChainId });
      go("sign");
      const trade = await createManaListing({
        provider: provider!,
        nft: { contractAddress: token.contractAddress, tokenId: token.tokenId, chainId: token.chainId as ChainId },
        priceWei: amount,
        expiresAtMs,
      });
      go("publish");
      const created = await postListingWithRetry(trade);
      patchManageCaches(qc, { address: account ?? undefined, contractAddress: token.contractAddress, tokenId: token.tokenId }, { kind: "listed", price: { kind: "mana", wei: amount }, tradeId: created.id });
      await qc.invalidateQueries({ queryKey: ["token", token.contractAddress, token.tokenId] });
      return created.id;
    });
  };
  const confirmMatches = !!amount && parseManaInput(confirmed) === amount;

  return (
    <form
      className="seller-sell"
      onSubmit={(e) => {
        e.preventDefault();
        if (!amount || invalidDate || write.busy) return;
        setConfirming(true);
      }}
    >
      {!isPolygon(token.chainId) ? <NetworkNotice chainId={token.chainId} /> : null}
      {relist ? (
        <p className="seller-sell__notice" role="note">
          Currently listed for {listingLabel(token)}. A new price removes that listing first, then lists the item again.
        </p>
      ) : null}
      <div className="seller-sell__fields">
        <div className="seller-field">
          <label className="seller-field__label" htmlFor={`${id}-price`}>
            Price
          </label>
          <span className="seller-field__control" data-invalid={price && !amount ? "" : undefined}>
            <ManaMark size={16} network={isPolygon(token.chainId) ? "polygon" : "ethereum"} />
            <input
              id={`${id}-price`}
              inputMode="decimal"
              value={price}
              onChange={(e) => setPrice(e.target.value.replace(/[^\d.]/g, ""))}
              placeholder="1000"
              disabled={write.busy}
              autoFocus
              aria-invalid={price && !amount ? true : undefined}
            />
          </span>
          {price && !amount ? <p className="seller-field__error">Enter a positive MANA amount.</p> : null}
          <p className="seller-field__help">Resales are paid in MANA, and the MANA goes to your wallet. Buyers cannot pay for a resale with Credits.</p>
        </div>
        <div className="seller-field">
          <label className="seller-field__label" htmlFor={`${id}-expires`}>
            Expiration date
          </label>
          <span className="seller-field__control" data-invalid={invalidDate ? "" : undefined}>
            <input id={`${id}-expires`} type="date" value={expiresAt} min={dateInputValue(now())} onChange={(e) => setExpiresAt(e.target.value || dateInputValue(now() + DEFAULT_EXPIRATION_DAYS * DAY))} disabled={write.busy} aria-invalid={invalidDate || undefined} />
          </span>
          {invalidDate ? <p className="seller-field__error">This date has already passed</p> : null}
        </div>
      </div>
      {write.busy || write.state.kind === "failed" ? (
        <ol className="mk-steps" aria-label="Steps">
          {steps.map((s) => {
            const at = step ? steps.indexOf(step) : -1;
            const i = steps.indexOf(s);
            return (
              <li key={s} data-state={i < at ? "done" : i === at ? "active" : "todo"}>
                {STEP_TEXT[s]}
              </li>
            );
          })}
        </ol>
      ) : null}
      <div className="seller-buttons">
        <button type="button" className="seller-btn" disabled={write.busy} onClick={back}>
          Cancel
        </button>
        <button type="submit" className="seller-btn seller-btn--primary" disabled={!amount || invalidDate || write.busy}>
          {write.busy ? "Listing…" : relist ? "Update" : "List for sale"}
        </button>
      </div>
      <WriteStatus state={write.state} />
      {confirming && amount ? (
        <Modal onClose={() => setConfirming(false)} className="seller-confirm" width={480}>
          <h2 className="seller-confirm__title">Please confirm</h2>
          <p>
            You are about to list <b>{token.name}</b> on sale for <ManaMark size={14} network={isPolygon(token.chainId) ? "polygon" : "ethereum"} /> {formatMana(amount, { group: true })}.
          </p>
          <p>Please re-enter the price to confirm:</p>
          <label className="seller-field">
            <span className="seller-field__label">Price</span>
            <span className="seller-field__control">
              <ManaMark size={16} network={isPolygon(token.chainId) ? "polygon" : "ethereum"} />
              <input inputMode="decimal" value={confirmed} placeholder={formatMana(amount)} aria-label="Confirm the price" onChange={(e) => setConfirmed(e.target.value.replace(/[^\d.]/g, ""))} autoFocus />
            </span>
          </label>
          <div className="seller-buttons">
            <button type="button" className="seller-btn" onClick={() => (setConfirmed(""), setConfirming(false))}>
              Cancel
            </button>
            <button type="button" className="seller-btn seller-btn--primary" disabled={!confirmMatches} onClick={submit}>
              Proceed
            </button>
          </div>
        </Modal>
      ) : null}
    </form>
  );
}

export function TokenSellView({ token }: { token: TokenRecord }) {
  const owner = useIsOwner(token);
  const caps = useShopCapabilities();
  const relist = token.isOnSale;
  return (
    <TokenFrame
      token={token}
      title={relist ? "Update your listing" : "List for sale"}
      subtitle={
        <>
          Set a {relist ? "new " : ""}price and expiration date for <b>{token.name}</b>.
        </>
      }
    >
      {!owner ? (
        <p role="alert" data-testid="shop-not-owner">
          You are not the owner of <b>{token.name}</b>.
        </p>
      ) : !caps.secondarySales ? (
        <p role="status" data-testid="shop-resales-off">
          This Shop does not list resales right now.
        </p>
      ) : (
        <WalletGate what="Listing an item">
          <SellForm token={token} />
        </WalletGate>
      )}
    </TokenFrame>
  );
}

export default function TokenSell() {
  const { account } = useShopWallet();
  if (!account) return <MarketSignIn />;
  return (
    <EnsureQueryClient>
      <div className="seller-token">
        <WithRouteToken title="List for sale">{(token) => <TokenSellView token={token} />}</WithRouteToken>
      </div>
    </EnsureQueryClient>
  );
}
