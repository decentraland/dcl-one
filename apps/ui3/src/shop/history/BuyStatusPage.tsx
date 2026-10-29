// from: marketplace@3bf3662f:webapp/src/components/Routes/Routes.tsx (the buy/status routes) and the
//       BuyPage/StatusPage it renders (+ StatusPage.css, images/buy-nfts-status-page-stars-*.svg); v2 ui3/src/marketplace/pages/MkBuyStatusPage.tsx (moved here: its copy and
//       states). Marketplace UI: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.
import { useQuery } from "@tanstack/react-query";
import { Link, Navigate, useParams, useSearchParams } from "react-router";

import Spinner from "../../atoms/Spinner";
import { CHAIN } from "../model/chain";
import { useShopWallet } from "../wallet/context";
import { explorerTxUrl, settlementChainId } from "../checkout/chain/config";
import { SHOP_QUERY_KEYS } from "../checkout/lib/after-purchase";
import { fetchAssetDisplay } from "../checkout/lib/api";
import { showsWalletConfirmations } from "../checkout/lib/wallet-kind";
import { EnsureQueryClient, useShopServices } from "../checkout/services";
import { myItemsRouteFor } from "../discovery/lib/routes";
import { fetchUserPurchases } from "../credits/credits";
import { useSettlement } from "./useSettlement";
import "./buystatus.css";

type BuyStatus = "pending" | "complete" | "failed" | "cancelled" | "unknown";

const COPY: Record<BuyStatus, { title: string; body: string }> = {
  pending: { title: "Your item is on its way!", body: "The transaction can take up to 3 minutes to be processed." },
  complete: { title: "All done!", body: "You can find your item in" },
  failed: { title: "Something went wrong", body: "The purchase could not be completed and nothing was delivered. Credits it reserved come back to your balance. You can try again from" },
  cancelled: { title: "Purchase cancelled", body: "This purchase was cancelled before it went out. You can try again from" },
  unknown: { title: "", body: "" },
};

function useStatus(contractAddress: string, ids: { tokenId?: string; itemId?: string }): { status: BuyStatus; txUrl: string | null; loading: boolean } {
  const [params] = useSearchParams();
  const txHash = params.get("txHash");
  const chainId = Number(params.get("chainId")) || (ids.tokenId && contractAddress ? settlementChainId() : CHAIN.POLYGON);
  const settlement = useSettlement(txHash ? [{ hash: txHash, chainId }] : [], { skip: !txHash });
  const { client } = useShopServices();
  const wallet = useShopWallet();
  const account = wallet.account;
  const purchases = useQuery({
    queryKey: [SHOP_QUERY_KEYS.purchases, account, "status", contractAddress, ids.itemId, ids.tokenId],
    enabled: !txHash && !!account,
    queryFn: () => fetchUserPurchases(account!, { all: true, first: 50 }, client),
  });
  if (txHash) {
    const status: BuyStatus = settlement === "confirmed" ? "complete" : settlement === "failed" ? "failed" : "pending";
    return { status, txUrl: explorerTxUrl(chainId, txHash), loading: false };
  }
  const mine = (purchases.data?.items ?? [])
    .filter((p) => p.contractAddress?.toLowerCase() === contractAddress.toLowerCase() && (!ids.itemId || p.itemId === ids.itemId))
    .sort((a, b) => b.createdAt - a.createdAt)[0];
  if (!mine) return { status: "unknown", txUrl: null, loading: !!account && purchases.isPending };
  const url = mine.txHash ? explorerTxUrl(settlementChainId(), mine.txHash) : null;
  if (mine.status === "SETTLED") return { status: "complete", txUrl: url, loading: false };
  if (mine.status === "PENDING") return { status: "pending", txUrl: url, loading: false };
  return { status: mine.submittedTxHash ? "failed" : "cancelled", txUrl: url, loading: false };
}

async function fetchCategory(client: ReturnType<typeof useShopServices>["client"], contractAddress: string, ids: { tokenId?: string; itemId?: string }): Promise<string | null> {
  try {
    if (ids.itemId) {
      const body = await client.market.items<{ data?: { category?: string }[] }>({ contractAddress, itemId: ids.itemId, first: 1 });
      return body?.data?.[0]?.category ?? null;
    }
    if (ids.tokenId) {
      const body = await client.market.nfts<{ data?: { nft?: { category?: string } }[] }>({ contractAddress, tokenId: ids.tokenId, first: 1 });
      return body?.data?.[0]?.nft?.category ?? null;
    }
  } catch {
  }
  return null;
}

function BuyStatusScreen() {
  const { contractAddress = "", tokenId, itemId } = useParams();
  const { client } = useShopServices();
  const wallet = useShopWallet();
  const { status, txUrl, loading } = useStatus(contractAddress, { tokenId, itemId });
  const display = useQuery({
    queryKey: ["shop-asset-display", contractAddress, tokenId ?? null, itemId ?? null],
    queryFn: () => fetchAssetDisplay(contractAddress, { tokenId, itemId }, client),
    staleTime: 5 * 60_000,
  });
  const category = useQuery({
    queryKey: ["shop-asset-category", contractAddress, tokenId ?? null, itemId ?? null],
    queryFn: () => fetchCategory(client, contractAddress, { tokenId, itemId }),
    staleTime: 5 * 60_000,
  });
  if (status === "unknown" && !loading) return <Navigate to="/shop" replace />;
  const itemHref = tokenId ? `/shop/token/${contractAddress}/${tokenId}` : `/shop/item/${contractAddress}/${itemId}`;
  const section = category.data === "emote" ? "emote" : "wearable";
  const browseHref = `/shop/items?category=${section}`;
  const myItemsHref = myItemsRouteFor([section]);
  const copy = COPY[status];
  const selfCustody = showsWalletConfirmations(wallet.host?.walletKind ?? null);
  return (
    <main className="shop-buystatus" aria-labelledby="shop-buystatus-title">
      <section className="shop-buystatus__card" data-status={status} data-testid="buy-status" role={status === "failed" ? "alert" : "status"}>
        <div className="shop-buystatus__img" aria-hidden="true">
          <div className="shop-buystatus__frame">{display.data?.thumbnail ? <img src={display.data.thumbnail} alt="" /> : null}</div>
        </div>
        <h1 id="shop-buystatus-title" className="shop-buystatus__title">
          {loading ? "Checking your purchase…" : copy.title}
        </h1>
        {status === "pending" || loading ? (
          <div className="shop-buystatus__status">
            <Spinner size={20} />
            <p>Processing Transaction</p>
          </div>
        ) : null}
        {!loading ? (
          <p className="shop-buystatus__desc">
            {status === "complete" ? (
              <>
                {copy.body} <Link to={myItemsHref}>My Items</Link>.
              </>
            ) : status === "failed" || status === "cancelled" ? (
              <>
                {copy.body} <Link to={itemHref}>the item's page</Link>.
              </>
            ) : (
              copy.body
            )}
          </p>
        ) : null}
        {txUrl && selfCustody ? (
          <a className="shop-buystatus__tx" href={txUrl} target="_blank" rel="noopener noreferrer">
            View transaction
          </a>
        ) : null}
        {status === "complete" ? (
          <Link className="shop-buystatus__cta" to={browseHref}>
            Explore more collectibles
          </Link>
        ) : null}
      </section>
    </main>
  );
}

export default function BuyStatusPage() {
  return (
    <EnsureQueryClient>
      <BuyStatusScreen />
    </EnsureQueryClient>
  );
}
