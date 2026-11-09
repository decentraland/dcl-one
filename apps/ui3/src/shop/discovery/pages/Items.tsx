// from: shop@2d3bf62:app/src/pages/Assets.tsx, app/src/pages/Assets.styles.ts, app/src/styles/grid.styles.ts,
//       app/src/components/EmptyState/EmptyState.tsx, app/src/assets/empty/search-empty.svg (./assets);
//       marketplace@f3956826:webapp/src/components/Routes/Routes.tsx (`/browse`), webapp/src/modules/item/actions.ts (FETCH_ITEMS_REQUEST)
// Licence of the Marketplace parts: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.

import { useEffect, useMemo, useState } from "react";
import { Navigate, useNavigate, useSearchParams } from "react-router";

import Button from "../../../atoms/Button";
import { SHOP_BASE } from "../../routes";
import AssetCard from "../components/AssetCard";
import BrowseLayout from "../components/BrowseLayout";
import FilterBar, { RARITIES, type FilterChip } from "../components/FilterBar";
import Filters, { type FilterStatus } from "../components/Filters";
import IndexNotice from "../components/IndexNotice";
import { useCollectionName, useCreatorNames } from "../components/PickerFilters";
import LoadMore from "../components/LoadMore";
import SkeletonCards from "../components/SkeletonCards";
import { indexUnavailable, type IndexFreshness } from "../freshness";
import { useMarketplace } from "../../market/access";
import { shopFlags } from "../flags";
import { useListingsFreshness, useScrollTopOnChange, useSeo, useUrlFilters } from "../hooks";
import { t } from "@shop/discovery/intl";
import type { CatalogItem } from "@shop/discovery/lib/catalogItem";
import { dealsRequestedIn, defaultSortFor, emoteFilterBindings, ITEMS_PAGE_SIZE, itemsFilterDefaults, itemsGrid, itemsQuery } from "../lib/itemsGrid";
import { listingKey } from "../lib/listingKey";
import { rarityLabel } from "@shop/discovery/lib/rarity";
import { NAMES_ROUTE } from "../lib/routes";
import { useInfiniteShopQuery } from "../query";
import searchEmpty from "./assets/search-empty.svg?url";
import "../components/browselayout.css";
import "./items.css";
import { shopReads } from "../reads";

const SEO_TITLE = "Browse Collectibles";
const seoSearchTitle = (query: string) => `Search results for “${query}”`;
const EMPTY_SEARCH_BEFORE = "We couldn't find any results for “";
const EMPTY_SEARCH_AFTER = "”. Try a different search or explore the Shop.";

export type ItemsProps = {
  contracts?: string[];
  hideNames?: boolean;
  seo?: { title?: string };
  lockStatus?: FilterStatus;
};

export default function Items({ contracts, hideNames = false, seo, lockStatus }: ItemsProps = {}) {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const { rawQuery, q } = itemsQuery(searchParams);
  useSeo({ title: seo?.title ?? (rawQuery ? seoSearchTitle(rawQuery) : SEO_TITLE) });

  const flags = shopFlags();
  const marketplace = useMarketplace();
  const dealsRequested = dealsRequestedIn(searchParams);
  const filterDefaults = useMemo(() => itemsFilterDefaults(q, dealsRequested), [dealsRequested, q]);
  const [filterState, setFilters] = useUrlFilters(filterDefaults);
  const { subCategory, rarities, priceMin, priceMax, smart } = filterState;
  const { defaultStatus, deals, category, status, isUnified, min, max, bodyShape, listingType, wearableGenders, creators, collection, sortOptions, sortOption, key, fetchPage } = itemsGrid(filterState, q, { contracts, hideNames, lockStatus, marketplace });
  const emoteFilters = emoteFilterBindings(category, filterState, setFilters);
  useScrollTopOnChange(`${category}:${subCategory ?? ""}`);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const creatorNames = useCreatorNames(creators);
  const collectionName = useCollectionName(collection);

  const grid = useInfiniteShopQuery<CatalogItem, { freshness: IndexFreshness | null }>(key, (skip, signal) => fetchPage(skip, shopReads(signal)));

  const listingsFreshness = useListingsFreshness(isUnified && category !== "names");

  // How many cards the grid last showed, at most one page (see skeletonCount). Kept in state, set once the grid has
  // settled, rather than written during render; declared before the early return, as every hook.
  const [shownCount, setShownCount] = useState(0);
  const settledCount = grid.isLoading || grid.isPlaceholder ? null : Math.min(grid.items.length, ITEMS_PAGE_SIZE);
  useEffect(() => {
    if (settledCount !== null) setShownCount(settledCount);
  }, [settledCount]);
  if (category === "names") return <Navigate to={NAMES_ROUTE} replace />;

  const pickCategory = (key: string) => setFilters({ category: key, subCategory: null, ...emoteFilters.resetFor(key) });
  const toggleRarity = (r: string) => setFilters({ rarities: rarities.includes(r) ? rarities.filter((x) => x !== r) : [...rarities, r] });
  const clearFilters = () => setFilters(filterDefaults);

  const chips: FilterChip[] = [];
  if (status !== "not_for_sale" && (min != null || max != null)) chips.push({ key: "price", label: t("filter.price"), onRemove: () => setFilters({ priceMin: "", priceMax: "" }) });
  for (const r of RARITIES) if (rarities.includes(r)) chips.push({ key: `rarity-${r}`, label: rarityLabel(r), onRemove: () => toggleRarity(r) });
  if (wearableGenders) chips.push({ key: "bodyShape", label: t(`itemDetail.gender${bodyShape[0]!.toUpperCase()}${bodyShape.slice(1)}`), onRemove: () => setFilters({ bodyShape: "all" }) });
  if (isUnified && listingType) chips.push({ key: "listingType", label: t(`filter.${listingType}`), onRemove: () => setFilters({ listingType: "all" }) });
  for (const a of creators) chips.push({ key: `creator-${a}`, label: creatorNames[a]!, onRemove: () => setFilters({ creators: creators.filter((x) => x !== a) }) });
  if (collection) chips.push({ key: "collection", label: collectionName!, onRemove: () => setFilters({ collection: "" }) });
  if (smart) chips.push({ key: "smart", label: t("filter.smart"), onRemove: () => setFilters({ smart: false }) });
  chips.push(...emoteFilters.chips);
  if (deals) chips.push({ key: "deals", label: t("filter.deals"), onRemove: () => setFilters({ deals: false, sort: defaultSortFor(!!q, false) }) });
  if (!lockStatus && status !== defaultStatus)
    chips.push({
      key: "status",
      label: status === "all" ? t("filter.statusAll") : status === "on_sale" ? t("filter.onSale") : t("filter.notForSale"),
      onRemove: () => setFilters({ status: defaultStatus }),
    });

  const unavailable = indexUnavailable(grid.error);
  const showSkeletons = grid.isLoading || grid.isPlaceholder;
  // While a filter's results load over the previous ones, as many skeletons as cards were showing, up to a page
  // (upstream's gridSkeletonCount reads them from the kept page): the grid keeps its height instead of collapsing to
  // one card. The infinite query hides the kept page during a placeholder, so the count is remembered above.
  const skeletonCount = grid.isLoading ? 15 : Math.max(shownCount, 1);

  return (
    <div className="shop-page shop-items" data-testid="items-page">
      <BrowseLayout
        open={filtersOpen}
        onClose={() => setFiltersOpen(false)}
        sidebar={
          <Filters
            category={category}
            subCategory={subCategory}
            onCategory={pickCategory}
            onSub={(v) => setFilters({ subCategory: v })}
            priceMin={priceMin}
            priceMax={priceMax}
            onPriceMin={(v) => setFilters({ priceMin: v })}
            onPriceMax={(v) => setFilters({ priceMax: v })}
            rarities={rarities}
            onToggleRarity={toggleRarity}
            bodyShape={bodyShape}
            onBodyShape={category !== "emote" ? (v) => setFilters({ bodyShape: v }) : undefined}
            listingType={listingType ?? "all"}
            onListingType={isUnified && marketplace ? (v) => setFilters({ listingType: v }) : undefined}
            status={status}
            onStatus={(s) => setFilters({ status: s })}
            hideStatus={!!lockStatus}
            hidePrice={status === "not_for_sale"}
            hideNames={hideNames}
            smart={smart}
            onSmart={(v) => setFilters({ smart: v })}
            creators={creators}
            onCreators={contracts ? undefined : (v) => setFilters({ creators: v })}
            collection={collection}
            onCollection={contracts ? undefined : (v) => setFilters({ collection: v ?? "" })}
            // The collections route's on-sale flag means "the creator still sells it"; resales in the grid need every collection.
            collectionOnSale={isUnified && (!marketplace || listingType === "primary")}
            {...emoteFilters.props}
            deals={deals}
            onDeals={flags.creatorSales ? (v) => setFilters({ deals: v, sort: defaultSortFor(!!q, v) }) : undefined}
          />
        }
      >
        <FilterBar
          sort={sortOption.key}
          sortOptions={sortOptions}
          onSort={(v) => setFilters({ sort: v })}
          total={grid.error ? null : grid.total}
          loading={showSkeletons}
          query={rawQuery}
          onOpenFilters={() => setFiltersOpen(true)}
          chips={chips}
          onClearChips={clearFilters}
        />
        {unavailable ? <IndexNotice freshness={unavailable} what={t("collection.breadcrumb")} onRetry={grid.refetch} /> : <IndexNotice freshness={grid.meta?.freshness ?? listingsFreshness} what={t("collection.breadcrumb")} />}
        {grid.error && !unavailable ? (
          <div className="shop-page__error" role="alert" data-testid="browse-error">
            <p>{t("assets.loadError")}</p>
            <Button variant="secondary" size="sm" onClick={grid.refetch}>
              {t("loadMore.retry")}
            </Button>
          </div>
        ) : null}
        {!showSkeletons && !grid.error && grid.items.length === 0 ? (
          <div className="shop-items__empty" data-testid="browse-empty">
            <img className="shop-items__empty-art" src={searchEmpty} alt="" aria-hidden />
            <div className="shop-items__empty-text">
              <p className="shop-items__empty-title">{t("assets.empty.title")}</p>
              <p className="shop-items__empty-body">
                {rawQuery ? (
                  <>
                    {EMPTY_SEARCH_BEFORE}
                    <b>{rawQuery}</b>
                    {EMPTY_SEARCH_AFTER}
                  </>
                ) : (
                  t("assets.empty.filters")
                )}
              </p>
            </div>
            <button type="button" className="shop-items__empty-cta" onClick={() => navigate(`${SHOP_BASE}/overview`)}>
              {t("assets.empty.cta")}
            </button>
          </div>
        ) : unavailable ? null : (
          <>
            <div className="shop-grid" data-testid="grid">
              {showSkeletons ? (
                <SkeletonCards count={skeletonCount} />
              ) : (
                <>
                  {grid.items.map((item, i) => (
                    <AssetCard key={listingKey(item)} item={item} mode={isUnified ? "shop" : "view"} source="grid" position={i} />
                  ))}
                  {grid.isFetchingNextPage ? <SkeletonCards count={6} /> : null}
                </>
              )}
            </div>
            <LoadMore hasNextPage={grid.hasNextPage} isFetching={grid.isFetchingNextPage} isError={grid.isFetchNextPageError} onLoadMore={grid.fetchNextPage} />
          </>
        )}
      </BrowseLayout>
    </div>
  );
}
