// from: catalyrst-v2 ui3/src/marketplace/pages/{MkTransferPage,MkCancelSalePage,MkBidPage}.tsx (the token loading
//       and the asset-action frame they shared); marketplace@f3956826:webapp/src/components/AssetProviderPage
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
// The token behind a /shop/token/:contractAddress/:tokenId/{sell,cancel,transfer,bid,bids} page: its
// public record (anyone may look), whether the signed-in account owns it, and the dark AssetActionLayout
// frame around the action.

import { useQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { useNavigate, useParams } from "react-router";

import ThumbnailImage from "../../../components/ThumbnailImage";
import { parseEvmAddress, sameAddress } from "../../model/address";
import { normalizeDecimal } from "../../model/decimal";
import { shopPath } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import AssetActionLayout from "../frames/AssetActionLayout";
import { fetchToken, type MyAsset } from "../lib/api";
import { EnsureQueryClient, LoadFailure, Loading } from "./kit";

export type TokenRecord = MyAsset & { owner: string };

/** `:contractAddress` and `:tokenId` (or `:itemId`), validated: a malformed id never reaches a request. */
export function useTokenParams(): { contractAddress: string; tokenId: string | null; itemId: string | null } | null {
  const params = useParams();
  const contractAddress = parseEvmAddress(params.contractAddress ?? "");
  const tokenId = params.tokenId ? normalizeDecimal(params.tokenId) : null;
  const itemId = params.itemId ? normalizeDecimal(params.itemId) : null;
  if (!contractAddress || (!tokenId && !itemId)) return null;
  return { contractAddress, tokenId, itemId };
}

export function useToken(contractAddress: string | null, tokenId: string | null) {
  return useQuery({
    queryKey: ["token", contractAddress, tokenId],
    enabled: !!contractAddress && !!tokenId,
    queryFn: () => fetchToken(contractAddress!, tokenId!),
  });
}

export function useIsOwner(token: TokenRecord | null | undefined): boolean {
  const { account } = useShopWallet();
  return !!token && sameAddress(token.owner, account);
}

export function TokenFrame({ token, title, subtitle, children }: { token: Pick<MyAsset, "name" | "image" | "contractAddress" | "tokenId"> | null; title: ReactNode; subtitle?: ReactNode; children: ReactNode }) {
  const navigate = useNavigate();
  return (
    <EnsureQueryClient>
      <AssetActionLayout
        theme="dark"
        title={title}
        subtitle={subtitle}
        onBack={() => (token ? navigate(shopPath("tokenDetail", { contractAddress: token.contractAddress, tokenId: token.tokenId })) : navigate(-1))}
        media={
          token ? (
            <div className="mk-action-layout__media">{token.image ? <ThumbnailImage src={token.image} fallbackSrc={token.image} alt={token.name} /> : <span className="mk-tile__glyph">{token.name.slice(0, 1)}</span>}</div>
          ) : null
        }
      >
        <div className="mk-section">{children}</div>
      </AssetActionLayout>
    </EnsureQueryClient>
  );
}

/** Loads the token of the route and renders `children(token)`; handles bad ids, loading, errors and unknown tokens. */
export function WithRouteToken({ title, children }: { title: string; children: (token: TokenRecord) => ReactNode }) {
  const ids = useTokenParams();
  const query = useToken(ids?.contractAddress ?? null, ids?.tokenId ?? null);
  if (!ids || !ids.tokenId)
    return (
      <TokenFrame token={null} title={title}>
        <p role="alert">This link does not name a token.</p>
      </TokenFrame>
    );
  if (query.isPending)
    return (
      <TokenFrame token={null} title={title}>
        <Loading label="Loading the item…" />
      </TokenFrame>
    );
  if (query.isError)
    return (
      <TokenFrame token={null} title={title}>
        <LoadFailure error={query.error} what="This item" onRetry={() => void query.refetch()} />
      </TokenFrame>
    );
  if (!query.data)
    return (
      <TokenFrame token={null} title={title}>
        <p role="alert">This item does not exist or is not indexed yet.</p>
      </TokenFrame>
    );
  return <>{children(query.data)}</>;
}
