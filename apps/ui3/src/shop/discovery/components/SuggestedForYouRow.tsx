// from: shop@94584a5c:app/src/components/SuggestedForYouRow/SuggestedForYouRow.tsx, app/src/hooks/useSuggestedForYou.ts
// "Suggested for you": ranked and explained by the server for this visitor (the signed-in account, or
// seeds from the cart and recently viewed). It is asked for only while upstream's rollout flag is on
// (lib/remoteFlags.ts), and it renders only when the server personalised the answer
// and returned enough of it (suggestedHiddenReason); every card carries its reason line, linked to
// the item that triggered it when there is one.

import { useEffect, useMemo } from "react";
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
import { keyOf, prefetchShopQuery, useShopQuery } from "../query";
import AssetCard from "./AssetCard";
import Rail from "./Rail";
import { isResale } from "@shop/market/access-rule";
import { shopReads } from "../reads";

/** The one read size: the item page reads this many and shows fewer, so the cart page reuses its answer. */
export const RAIL_SIZE = 12;
const STALE_MS = 60_000;

type SeedItem = { contractAddress: string; itemId: string | null };

function onIdle(run: () => void): () => void {
  if (typeof window.requestIdleCallback === "function") {
    const handle = window.requestIdleCallback(run, { timeout: 2_000 });
    return () => window.cancelIdleCallback(handle);
  }
  const handle = setTimeout(run, 0);
  return () => clearTimeout(handle);
}

/**
 * `show` keeps the answer's first rows, as a read of that size returns them. `nextCartItem` is what this page's Add
 * to cart puts in the cart: once this read settles, the answer with it in the cart is read on idle, so the rail does
 * not reload behind the cart drawer.
 */
export function useSuggestedForYou(first: number, { exclude, show, nextCartItem }: { exclude?: string[]; show?: number; nextCartItem?: SeedItem | null } = {}) {
  const enabled = useRemoteFlag(REMOTE_FLAG.suggestedForYou);
  const { account } = useShopWallet();
  const cartLines = useCart((s) => s.lines);
  const recentlyViewed = useRecentlyViewed();
  const cart = useMemo(() => cartLines.map((l) => ({ contractAddress: l.asset.contract, itemId: l.asset.token.kind === "item" ? l.asset.token.itemId : null })), [cartLines]);
  const seeds = useMemo(() => buildSuggestionSeeds({ cart, recentlyViewed }), [cart, recentlyViewed]);
  const hasSignal = !!account || seeds.length > 0;
  const excludeKey = (exclude ?? []).join(",");
  const keyFor = (s: string[]) => ["suggested", account ?? "-", seedsKey(s), first, excludeKey];
  const fetchFor = (s: string[]) => (signal: AbortSignal) => fetchSuggestedItems({ address: account ?? undefined, seeds: s, exclude, first }, shopReads(signal));
  const key = enabled && hasSignal ? keyFor(seeds) : null;
  const query = useShopQuery(key, fetchFor(seeds), { staleMs: STALE_MS });

  const nextSeeds = useMemo(() => (nextCartItem ? buildSuggestionSeeds({ cart: [...cart, nextCartItem], recentlyViewed }) : null), [cart, nextCartItem, recentlyViewed]);
  const settled = !key || query.data !== undefined || !!query.error;
  const nextKey = enabled && settled && nextSeeds && seedsKey(nextSeeds) !== seedsKey(seeds) ? keyFor(nextSeeds) : null;
  const nextId = nextKey ? keyOf(nextKey) : null;
  useEffect(() => {
    if (!nextKey || !nextSeeds) return;
    return onIdle(() => prefetchShopQuery(nextKey, fetchFor(nextSeeds), STALE_MS));
    // The key names everything the read depends on.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nextId]);

  // A resale is the Marketplace's (market/access.tsx): without it the rail keeps creators' primary sales only. The
  // suggestions route reads no listing type, so the rows are narrowed here.
  const marketplace = useMarketplace();
  const result = useMemo(() => {
    if (!query.data) return undefined;
    const rows = show === undefined ? query.data.data : query.data.data.slice(0, show);
    return { ...query.data, data: marketplace ? rows : rows.filter((s) => !isResale(s)) };
  }, [query.data, marketplace, show]);
  return { result, isLoading: query.isLoading, isError: !!query.error, enabled, hasSignal };
}

export default function SuggestedForYouRow({ exclude, title, first = RAIL_SIZE, show }: { exclude?: string[]; title?: string; first?: number; show?: number }) {
  const { result, isLoading, isError, enabled, hasSignal } = useSuggestedForYou(first, { exclude, show });
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
