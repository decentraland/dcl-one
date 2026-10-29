// from: shop@2d3bf62:app/src/pages/Collection.tsx, app/src/styles/collectionPage.styles.ts,
//       app/src/components/CollectionHero/CollectionHero.tsx (the store cover, else the bundled default cover);
//       marketplace@f3956826:webapp/src/components/Routes/Routes.tsx (`/collections/:contractAddress`),
//       webapp/src/modules/collection/actions.ts (FETCH_SINGLE_COLLECTION_REQUEST), webapp/src/modules/item/actions.ts (FETCH_COLLECTION_ITEMS_REQUEST)
// Licence of the Marketplace parts: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.

import { useMemo, useState } from "react";
import { Link, useParams } from "react-router";

import Button from "../../../atoms/Button";
import EmptyState from "../../../components/EmptyState";
import { shortAddress } from "../../model/address";
import { SHOP_BASE } from "../../routes";
import { fetchCatalogItems, fetchCollection } from "../api";
import AssetCard from "../components/AssetCard";
import BrowseLayout from "../components/BrowseLayout";
import FilterBar, { RARITIES, SORTS, type FilterChip } from "../components/FilterBar";
import Filters, { STATUSES, type FilterStatus } from "../components/Filters";
import { CollectionHero } from "../components/Hero";
import IndexNotice from "../components/IndexNotice";
import LoadMore from "../components/LoadMore";
import SkeletonCards from "../components/SkeletonCards";
import { indexUnavailable, type IndexFreshness } from "../freshness";
import { useProfile, useScrollTopOnChange, useSeo, useUrlFilters } from "../hooks";
import { t } from "../intl";
import { SUBCAT_MAP } from "../lib/categories";
import type { CatalogItem } from "../lib/catalogItem";
import { rarityLabel } from "../lib/rarity";
import { routeSegment } from "../lib/routes";
import { useInfiniteShopQuery, useShopQuery } from "../query";
import { fetchStore } from "../../seller/lib/store";
import CreatorCard from "./collection/CreatorCard";
import defaultCover from "./collection/default-cover.jpeg";
import "../components/browselayout.css";
import "./collection/collection.css";

const PAGE_SIZE = 48;

export default function Collection() {
  const params = useParams<{ contractAddress: string }>();
  const contractAddress = routeSegment(params.contractAddress);
  const filterDefaults = useMemo(
    () => ({ category: "all", status: "all", subCategory: null as string | null, rarities: [] as string[], priceMin: "", priceMax: "", smart: false, sort: "newest" }),
    [],
  );
  const [filterState, setFilters] = useUrlFilters(filterDefaults);
  const { category, subCategory, rarities, priceMin, priceMax, smart, sort } = filterState;
  const status: FilterStatus = STATUSES.includes(filterState.status as FilterStatus) ? (filterState.status as FilterStatus) : "all";
  useScrollTopOnChange(`${category}:${subCategory ?? ""}`);
  const [filtersOpen, setFiltersOpen] = useState(false);

  const min = priceMin && !Number.isNaN(Number(priceMin)) ? Number(priceMin) : undefined;
  const max = priceMax && !Number.isNaN(Number(priceMax)) ? Number(priceMax) : undefined;
  const filters = {
    contractAddress,
    category,
    rarities: rarities.length ? rarities : undefined,
    wearableCategories: subCategory ? SUBCAT_MAP[subCategory] : undefined,
    minPriceCredits: min,
    maxPriceCredits: max,
    isWearableSmart: smart || undefined,
    isOnSale: status === "all" ? undefined : status === "on_sale",
    sortBy: (SORTS.find((s) => s.key === sort) ?? SORTS[0]!).server,
  };
  const grid = useInfiniteShopQuery<CatalogItem, { freshness: IndexFreshness | null }>(contractAddress ? ["collection-page", filters] : null, (skip, signal) =>
    fetchCatalogItems({ ...filters, first: PAGE_SIZE, skip }, { signal }),
  );
  const meta = useShopQuery(contractAddress ? ["collection-meta", contractAddress] : null, (signal) => fetchCollection(contractAddress!, { signal }), { staleMs: 5 * 60_000 });
  const title = meta.data?.name || t("collection.fallbackTitle");
  useSeo({ title: meta.data?.name });
  const creator = meta.data?.creator || grid.items[0]?.creator;
  const creatorProfile = useProfile(creator);
  const creatorName = creatorProfile?.name || (creator ? shortAddress(creator) : "");
  const store = useShopQuery(creator ? ["store", creator.toLowerCase()] : null, () => fetchStore(creator!), { staleMs: 5 * 60_000 });

  const toggleRarity = (r: string) => setFilters({ rarities: rarities.includes(r) ? rarities.filter((x) => x !== r) : [...rarities, r] });
  const clearFilters = () => setFilters(filterDefaults);
  const chips: FilterChip[] = [];
  if (min != null || max != null) chips.push({ key: "price", label: t("filter.price"), onRemove: () => setFilters({ priceMin: "", priceMax: "" }) });
  for (const r of RARITIES) if (rarities.includes(r)) chips.push({ key: `rarity-${r}`, label: rarityLabel(r), onRemove: () => toggleRarity(r) });
  if (smart) chips.push({ key: "smart", label: t("filter.smart"), onRemove: () => setFilters({ smart: false }) });
  if (status !== "all") chips.push({ key: "status", label: status === "on_sale" ? t("filter.onSale") : t("filter.notForSale"), onRemove: () => setFilters({ status: "all" }) });

  const unavailable = indexUnavailable(grid.error);

  return (
    <div className="shop-page shop-collection" data-testid="collection-page">
      <nav className="shop-crumbs" aria-label={t("collection.breadcrumbAria")}>
        <Link to={`${SHOP_BASE}/items`}>{t("collection.breadcrumb")}</Link>
        {creator ? (
          <>
            <span aria-hidden>/</span>
            <Link to={`${SHOP_BASE}/items/creator/${creator}`}>{creatorName}</Link>
            <span aria-hidden>/</span>
            <Link to={`${SHOP_BASE}/items/creator/${creator}?collections`}>{t("categories.collections")}</Link>
          </>
        ) : null}
        <span aria-hidden>/</span>
        <span aria-current="page">{title}</span>
      </nav>
      <CollectionHero name={title} cover={store.data?.cover || defaultCover} />
      <BrowseLayout
        open={filtersOpen}
        onClose={() => setFiltersOpen(false)}
        sidebarTop={<CreatorCard address={creator} />}
        sidebar={
          <Filters
            category={category}
            subCategory={subCategory}
            onCategory={(key) => setFilters({ category: key, subCategory: null })}
            onSub={(v) => setFilters({ subCategory: v })}
            priceMin={priceMin}
            priceMax={priceMax}
            onPriceMin={(v) => setFilters({ priceMin: v })}
            onPriceMax={(v) => setFilters({ priceMax: v })}
            rarities={rarities}
            onToggleRarity={toggleRarity}
            status={status}
            onStatus={(s) => setFilters({ status: s })}
            smart={smart}
            onSmart={(v) => setFilters({ smart: v })}
            hideNames
          />
        }
      >
        <FilterBar sort={sort} onSort={(v) => setFilters({ sort: v })} total={grid.total} loading={grid.isLoading || grid.isPlaceholder} onOpenFilters={() => setFiltersOpen(true)} chips={chips} onClearChips={clearFilters} />
        {unavailable ? <IndexNotice freshness={unavailable} what={title} onRetry={grid.refetch} /> : <IndexNotice freshness={grid.meta?.freshness} what={title} />}
        {grid.error && !unavailable ? (
          <div className="shop-page__error" role="alert" data-testid="collection-error">
            <p>{t("collection.error")}</p>
            <Button variant="secondary" size="sm" onClick={grid.refetch}>
              {t("loadMore.retry")}
            </Button>
          </div>
        ) : null}
        {!grid.isLoading && !grid.error && grid.items.length === 0 ? (
          <EmptyState data-testid="collection-empty" title={t("assets.empty.title")} subtitle={t("collection.empty")} actions={[{ label: t("filterBar.clearAll"), onClick: clearFilters }]} />
        ) : unavailable ? null : (
          <>
            <div className="shop-grid" data-testid="grid">
              {grid.isLoading || grid.isPlaceholder ? (
                <SkeletonCards count={15} />
              ) : (
                <>
                  {grid.items.map((item, i) => (
                    <AssetCard key={item.id} item={item} source="collection" position={i} creatorName={creatorName || null} />
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
