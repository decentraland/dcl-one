// from: marketplace@3bf3662f:webapp/src/components/CancelSalePage (CancelSalePage.tsx, CancelSalePage.css),
//       webapp/src/modules/translation/locales/en.json (cancel_sale_page.*); shop@2d3bf62:app/src/components/RemoveListingModal/RemoveListingModal.tsx,
//       app/src/lib/buy.ts (cancelListing modes); catalyrst-v2 ui3/src/marketplace/pages/MkCancelSalePage.tsx
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useNavigate } from "react-router";

import Button from "../../../atoms/Button";
import { shopPath } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { markListingCancelled } from "../../market/lib/dead-listings";
import { fetchLegacyOrders } from "../../market/lib/api";
import { cancelTrade, fetchTrade, GaslessCancelFailedError, type CancelMode } from "../../market/lib/trades";
import { cancelLegacyOrder } from "../../market/purchase/executeOrder";
import { EnsureQueryClient, NetworkNotice, useShopProvider, useWrite, WalletGate, WriteStatus } from "../../market/components/kit";
import { listingLabel } from "../../market/components/AssetTile";
import { TokenFrame, useIsOwner, WithRouteToken, type TokenRecord } from "../../market/components/TokenAction";
import { patchManageCaches } from "../lib/manage-cache";
import { MarketSignIn } from "../pages/SellerSignIn";
import "../pages/sellerpages.css";

export function RemoveListing({ token }: { token: TokenRecord }) {
  const qc = useQueryClient();
  const { account } = useShopWallet();
  const provider = useShopProvider();
  const navigate = useNavigate();
  const write = useWrite();
  const [gaslessFailed, setGaslessFailed] = useState(false);
  const tradeId = token.listing?.tradeId ?? null;

  const cancel = (mode: CancelMode) =>
    write.run(mode === "direct" ? "Confirm the cancellation in your wallet (you pay the gas)…" : "Confirm in your wallet (no gas needed)…", "Removed from sale.", async () => {
      let hash: string;
      if (tradeId) {
        const trade = await fetchTrade(tradeId);
        try {
          hash = await cancelTrade({ provider: provider!, trade, mode });
        } catch (e) {
          if (e instanceof GaslessCancelFailedError) setGaslessFailed(true);
          throw e;
        }
        markListingCancelled(qc, tradeId);
      } else {
        const [order] = await fetchLegacyOrders({ contractAddress: token.contractAddress, tokenId: token.tokenId, owner: token.owner });
        if (!order) throw new Error("This listing is already gone.");
        hash = await cancelLegacyOrder({ provider: provider!, order });
      }
      patchManageCaches(qc, { address: account ?? undefined, contractAddress: token.contractAddress, tokenId: token.tokenId }, { kind: "removed" });
      await qc.invalidateQueries({ queryKey: ["token", token.contractAddress, token.tokenId] });
      return hash;
    });

  if (write.state.kind === "done") {
    return (
      <div className="mk-section" data-testid="shop-remove-done">
        <p role="status">“{token.name}” is no longer for sale. You can put it on sale again anytime.</p>
        <Link className="mk-link" to={shopPath("myItems")}>
          Back to My items
        </Link>
      </div>
    );
  }

  return (
    <div className="seller-cancel">
      {!tradeId ? <NetworkNotice chainId={token.chainId} /> : null}
      <div className="seller-buttons">
        <button type="button" className="seller-btn" disabled={write.busy} onClick={() => navigate(shopPath("tokenDetail", { contractAddress: token.contractAddress, tokenId: token.tokenId }))}>
          Cancel
        </button>
        <button type="button" className="seller-btn seller-btn--primary" disabled={write.busy} onClick={() => void cancel(tradeId ? "gasless-only" : "direct")}>
          {write.busy ? "Removing…" : "Confirm"}
        </button>
      </div>
      <WriteStatus state={write.state} />
      {gaslessFailed ? (
        <div className="mk-note mk-note--warning" role="alert" data-testid="shop-gasless-failed">
          <p>The free cancellation did not go through. You can cancel with a regular transaction; your wallet pays the network fee.</p>
          <NetworkNotice chainId={token.chainId} />
          <Button size="sm" variant="secondary" disabled={write.busy} onClick={() => void cancel("direct")}>
            Cancel and pay the gas
          </Button>
        </div>
      ) : null}
    </div>
  );
}

function DisabledActions({ token }: { token: TokenRecord }) {
  const navigate = useNavigate();
  return (
    <div className="seller-buttons">
      <button type="button" className="seller-btn" onClick={() => navigate(shopPath("tokenDetail", { contractAddress: token.contractAddress, tokenId: token.tokenId }))}>
        Cancel
      </button>
      <button type="button" className="seller-btn seller-btn--primary" disabled>
        Confirm
      </button>
    </div>
  );
}

export function RemoveListingView({ token }: { token: TokenRecord }) {
  const owner = useIsOwner(token);
  const subtitle = !token.isOnSale ? (
    <span data-testid="shop-not-on-sale">
      There are no active listings for <b>{token.name}</b>.
    </span>
  ) : !owner ? (
    <span data-testid="shop-not-owner">
      You are not the owner of <b>{token.name}</b>.
    </span>
  ) : (
    <>
      You are about to remove the listing of <b>{token.name}</b> for {listingLabel(token)}.
    </>
  );
  return (
    <TokenFrame token={token} title="Remove listing" subtitle={subtitle}>
      {!owner || !token.isOnSale ? (
        <DisabledActions token={token} />
      ) : (
        <WalletGate what="Removing a listing">
          <RemoveListing token={token} />
        </WalletGate>
      )}
    </TokenFrame>
  );
}

export default function RemoveListingModal() {
  const { account } = useShopWallet();
  if (!account) return <MarketSignIn />;
  return (
    <EnsureQueryClient>
      <div className="seller-token">
        <WithRouteToken title="Remove listing">{(token) => <RemoveListingView token={token} />}</WithRouteToken>
      </div>
    </EnsureQueryClient>
  );
}
