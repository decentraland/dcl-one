// from: shop@94584a5c:app/src/components/SuggestedForYouRow/SuggestedForYouRow.tsx, app/src/hooks/useSuggestedForYou.ts
// "Suggested for you": ranked and explained by the server for this visitor (the signed-in account, or
// seeds from the cart and recently viewed). It is asked for only while upstream's rollout flag is on
// (lib/remoteFlags.ts), and it renders only when the server personalised the answer
// and returned enough of it (suggestedHiddenReason); every card carries its reason line, linked to
// the item that triggered it when there is one.

import { useMemo } from "react";
import { Link } from "react-router";

import { useCart } from "../../cart/store";
import { useMarketplace } from "../../market/access";
import { useShopWallet } from "../../wallet/context";
import { fetchCatalogByIds, fetchSuggestedItems, type SuggestedItem } from "@shop/discovery/api";
import { t } from "@shop/discovery/intl";
import { useRecentlyViewed } from "../lib/recently-viewed";
import { REMOTE_FLAG, useRemoteFlag } from "../lib/remoteFlags";
import { suggestedHiddenReason } from "../lib/suggestionEvents";
import { reasonInterpolatesItemName, reasonKey, reasonLinksToItem, triggerItemPath } from "../lib/suggestionReasons";
import { buildSuggestionSeeds, seedsKey } from "../lib/suggestionSeeds";
import { useShopQuery } from "../query";
import AssetCard from "./AssetCard";
import Rail from "./Rail";
import { isResale } from "@shop/market/access-rule";
import { shopReads } from "../reads";

const RAIL_SIZE = 12;

export function useSuggestedForYou(first: number, { exclude }: { exclude?: string[] } = {}) {
  const enabled = useRemoteFlag(REMOTE_FLAG.suggestedForYou);
  const { account } = useShopWallet();
  const cartLines = useCart((s) => s.lines);
  const recentlyViewed = useRecentlyViewed();
  const seeds = useMemo(() => {
    const cart = cartLines.map((l) => ({ contractAddress: l.asset.contract, itemId: l.asset.token.kind === "item" ? l.asset.token.itemId : null }));
    return buildSuggestionSeeds({ cart, recentlyViewed });
  }, [cartLines, recentlyViewed]);
  const hasSignal = !!account || seeds.length > 0;
  const key = enabled && hasSignal ? ["suggested", account ?? "-", seedsKey(seeds), first, (exclude ?? []).join(",")] : null;
  const query = useShopQuery(key, (signal) => fetchSuggestedItems({ address: account ?? undefined, seeds, exclude, first }, shopReads(signal)), { staleMs: 60_000 });
  // A resale is the Marketplace's (market/access.tsx): without it the rail keeps creators' primary sales only. The
  // suggestions route reads no listing type, so the rows are narrowed here.
  const marketplace = useMarketplace();
  const result = useMemo(
    () => (query.data && !marketplace ? { ...query.data, data: query.data.data.filter((s) => !isResale(s)) } : query.data),
    [query.data, marketplace],
  );
  return { result, isLoading: query.isLoading, isError: !!query.error, enabled, hasSignal };
}

export default function SuggestedForYouRow({ exclude, title, first = RAIL_SIZE }: { exclude?: string[]; title?: string; first?: number }) {
  const { result, isLoading, isError, enabled, hasSignal } = useSuggestedForYou(first, { exclude });
  const items = useMemo(() => result?.data ?? [], [result]);
  const triggerIds = useMemo(() => [...new Set(items.filter((i) => i.reason.itemId && reasonInterpolatesItemName(i.reason.kind)).map((i) => i.reason.itemId!))], [items]);
  const { data: triggers } = useShopQuery(triggerIds.length ? ["suggested-triggers", ...triggerIds] : null, (signal) => fetchCatalogByIds(triggerIds, shopReads(signal)), { staleMs: 5 * 60_000 });
  const names = useMemo(() => new Map((triggers ?? []).map((i) => [`${i.contractAddress.toLowerCase()}-${i.itemId}`, i.name])), [triggers]);

  const hidden = suggestedHiddenReason({ enabled, hasSignal, isLoading, isError, personalized: result?.personalized, rowCount: items.length });
  if (hidden !== null) return null;
  if (!isLoading && items.length === 0) return null;

  return (
    <Rail
      title={title ?? t("overview.suggested.title")}
      items={items}
      loading={isLoading}
      source="suggested"
      testId={isLoading ? "suggested-row-skeleton" : "suggested-row"}
      renderItem={(item, i) => {
        const s = item as SuggestedItem;
        const key = reasonKey(s.reason.kind);
        const name = s.reason.itemId ? names.get(s.reason.itemId.toLowerCase()) : undefined;
        const text = key ? (reasonInterpolatesItemName(s.reason.kind) ? (name ? t(key, { item: name }) : t("overview.suggested.reason.generic")) : t(key)) : null;
        const path = s.reason.itemId && reasonLinksToItem(s.reason.kind) ? triggerItemPath(s.reason.itemId) : null;
        return (
          <div className="shop-suggested">
            <AssetCard item={s} source="suggested" position={i} />
            {text ? (
              <p className="shop-suggested__reason" data-testid="suggested-reason">
                {path ? <Link to={path}>{text}</Link> : text}
              </p>
            ) : null}
          </div>
        );
      }}
    />
  );
}
