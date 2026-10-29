// from: shop@2d3bf62:app/src/pages/Creator.tsx, app/src/pages/Creator.view.ts, app/src/styles/collectionPage.styles.ts;
//       marketplace@f3956826:webapp/src/modules/account/actions.ts (FETCH_CREATORS_ACCOUNT_REQUEST)
// Licence of the Marketplace parts: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.
import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Link, useNavigate, useParams, useSearchParams } from "react-router";

import Button from "../../../atoms/Button";
import EmptyState from "../../../components/EmptyState";
import { shortAddress } from "../../model/address";
import { SHOP_BASE } from "../../routes";
import { fetchCatalogItems, fetchCreatorCollections, type CollectionSummary } from "../api";
import AssetCard from "../components/AssetCard";
import BrowseLayout from "../components/BrowseLayout";
import CollectionCard from "../components/CollectionCard";
import FilterBar, { RARITIES, SORTS, type FilterChip } from "../components/FilterBar";
import Filters, { STATUSES, type FilterStatus } from "../components/Filters";
import IndexNotice from "../components/IndexNotice";
import LoadMore from "../components/LoadMore";
import SkeletonCards from "../components/SkeletonCards";
import { indexUnavailable, type IndexFreshness } from "../freshness";
import { useProfile, useScrollTopOnChange, useSeo, useUrlFilters } from "../hooks";
import { t } from "../intl";
import { SUBCAT_MAP } from "../lib/categories";
import type { CatalogItem } from "../lib/catalogItem";
import { resolveGridView } from "../lib/creatorView";
import { rarityLabel } from "../lib/rarity";
import { routeSegment } from "../lib/routes";
import { useInfiniteShopQuery, useShopQuery } from "../query";
import "../components/browselayout.css";
import "./creator.css";
import CreatorHero from "./creator/CreatorHero";
import collectionsEmptyIllustration from "./creator/collections-empty.svg?url";
import itemsEmptyIllustration from "./creator/items-empty.svg?url";
import searchEmptyIllustration from "./creator/search-empty.svg?url";

const PAGE_SIZE = 48;

export default function Creator() {
  const params = useParams<{ address: string }>();
  const address = routeSegment(params.address)?.toLowerCase();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const collectionsMode = searchParams.has("collections");
  const profile = useProfile(address);
  const name = profile?.name || (address ? shortAddress(address) : t("creator.fallbackName"));
  useSeo({ title: name });

  const filterDefaults = useMemo(
    () => ({ category: "all", status: "all", subCategory: null as string | null, rarities: [] as string[], priceMin: "", priceMax: "", smart: false, sort: "newest" }),
    [],
  );
  const [filterState, setFilters] = useUrlFilters(filterDefaults);
  const { category, subCategory, rarities, priceMin, priceMax, smart, sort } = filterState;
  const status: FilterStatus = STATUSES.includes(filterState.status as FilterStatus) ? (filterState.status as FilterStatus) : "all";
  useScrollTopOnChange(`${collectionsMode}:${category}:${subCategory ?? ""}`);
  const [filtersOpen, setFiltersOpen] = useState(false);

  const min = priceMin && !Number.isNaN(Number(priceMin)) ? Number(priceMin) : undefined;
  const max = priceMax && !Number.isNaN(Number(priceMax)) ? Number(priceMax) : undefined;
  const filters = {
    creator: address,
    category,
    rarities: rarities.length ? rarities : undefined,
    wearableCategories: subCategory ? SUBCAT_MAP[subCategory] : undefined,
    minPriceCredits: min,
    maxPriceCredits: max,
    isWearableSmart: smart || undefined,
    isOnSale: status === "all" ? undefined : status === "on_sale",
    sortBy: (SORTS.find((s) => s.key === sort) ?? SORTS[0]!).server,
  };
  const grid = useInfiniteShopQuery<CatalogItem, { freshness: IndexFreshness | null }>(address && !collectionsMode ? ["creator-items", filters] : null, (skip, signal) =>
    fetchCatalogItems({ ...filters, first: PAGE_SIZE, skip }, { signal }),
  );
  const baseline = useShopQuery(address ? ["creator-item-count", address] : null, (signal) => fetchCatalogItems({ creator: address, first: 1 }, { signal }).then((r) => r.total));
  const collections = useInfiniteShopQuery<CollectionSummary>(address && collectionsMode ? ["creator-collections", address] : null, (skip, signal) =>
    fetchCreatorCollections(address!, { first: PAGE_SIZE, skip }, { signal }).then((r) => ({ items: r.collections, total: r.total })),
  );

  const searchWith = (next: URLSearchParams, collections: boolean) => {
    next.delete("collections");
    const rest = next.toString();
    return collections ? (rest ? `?${rest}&collections` : "?collections") : rest ? `?${rest}` : "";
  };
  const toggleCollections = () => navigate({ search: searchWith(new URLSearchParams(searchParams), !collectionsMode) }, { replace: true });
  const pickCategory = (key: string) => {
    if (!collectionsMode) return setFilters({ category: key, subCategory: null });
    const next = new URLSearchParams(searchParams);
    if (key === filterDefaults.category) next.delete("category");
    else next.set("category", key);
    next.delete("subCategory");
    navigate({ search: searchWith(next, false) }, { replace: true });
  };
  const toggleRarity = (r: string) => setFilters({ rarities: rarities.includes(r) ? rarities.filter((x) => x !== r) : [...rarities, r] });
  const clearFilters = () => setFilters(filterDefaults);
  const chips: FilterChip[] = [];
  if (category === "wearable" || category === "emote")
    chips.push({ key: "category", label: t(category === "emote" ? "categories.emotes" : "categories.wearables"), onRemove: () => pickCategory("all") });
  if (min != null || max != null) chips.push({ key: "price", label: t("filter.price"), onRemove: () => setFilters({ priceMin: "", priceMax: "" }) });
  for (const r of RARITIES) if (rarities.includes(r)) chips.push({ key: `rarity-${r}`, label: rarityLabel(r), onRemove: () => toggleRarity(r) });
  if (smart) chips.push({ key: "smart", label: t("filter.smart"), onRemove: () => setFilters({ smart: false }) });
  if (status !== "all") chips.push({ key: "status", label: status === "on_sale" ? t("filter.onSale") : t("filter.notForSale"), onRemove: () => setFilters({ status: "all" }) });

  const view = resolveGridView({
    gridLoading: grid.isLoading || grid.isPlaceholder,
    gridError: !!grid.error,
    gridCount: grid.items.length,
    baselinePending: baseline.isLoading || (baseline.data === undefined && !baseline.error),
    baselineError: !!baseline.error,
    baselineCount: baseline.data,
  });
  const unavailable = indexUnavailable(grid.error) ?? indexUnavailable(collections.error);
  const illustration = (src: string) => <img className="shop-creator__empty-illus" src={src} alt="" />;
  const exploreCta = { label: t("assets.empty.cta"), onClick: () => navigate(`${SHOP_BASE}/items`) };

  if (!address) return null;

  return (
    <div className="shop-page shop-creator" data-testid="creator-page">
      <nav className="shop-crumbs" aria-label={t("creator.breadcrumbAria")}>
        <Link to={`${SHOP_BASE}/items`}>{t("creator.breadcrumb")}</Link>
        <span aria-hidden>/</span>
        <span aria-current="page">{name}</span>
      </nav>
      <CreatorHero address={address} />
      <BrowseLayout
        open={filtersOpen}
        onClose={() => setFiltersOpen(false)}
        sidebar={
          <>
            <Filters
              category={collectionsMode ? COLLECTIONS_CATEGORY : category}
              subCategory={collectionsMode ? null : subCategory}
              onCategory={pickCategory}
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
            <CollectionsCategory selected={collectionsMode} onToggle={toggleCollections} />
          </>
        }
      >
        {collectionsMode ? (
          <section data-testid="creator-collections">
            <div className="shop-creator__collbar">
              <span className="shop-creator__count" data-testid="creator-collections-count" aria-busy={collections.isLoading || undefined}>
                {collections.isLoading ? <span className="shop-creator__countskel" aria-hidden /> : t("creator.collectionsCount", { count: collections.total })}
              </span>
            </div>
            {unavailable ? <IndexNotice freshness={unavailable} what={t("creator.collections")} onRetry={collections.refetch} /> : null}
            {collections.error && !unavailable ? (
              <div className="shop-page__error" role="alert">
                <p>{t("creator.error")}</p>
                <Button variant="secondary" size="sm" onClick={collections.refetch}>
                  {t("loadMore.retry")}
                </Button>
              </div>
            ) : !collections.isLoading && collections.items.length === 0 && !unavailable ? (
              <EmptyState
                data-testid="creator-collections-empty"
                icon={illustration(collectionsEmptyIllustration)}
                title={t("creator.collectionsEmptyTitle")}
                subtitle={t("creator.collectionsEmpty")}
                actions={[exploreCta]}
              />
            ) : (
              <div className="shop-grid" data-variant="collections">
                {collections.isLoading ? (
                  <SkeletonCards count={9} />
                ) : (
                  <>
                    {collections.items.map((c) => (
                      <CollectionCard key={c.contractAddress} collection={c} itemCount={c.itemCount} />
                    ))}
                    {collections.isFetchingNextPage ? <SkeletonCards count={6} /> : null}
                  </>
                )}
              </div>
            )}
            <LoadMore hasNextPage={collections.hasNextPage} isFetching={collections.isFetchingNextPage} isError={collections.isFetchNextPageError} onLoadMore={collections.fetchNextPage} />
          </section>
        ) : (
          <>
            <FilterBar sort={sort} onSort={(v) => setFilters({ sort: v })} total={grid.total} loading={view === "skeletons"} onOpenFilters={() => setFiltersOpen(true)} chips={chips} onClearChips={clearFilters} />
            {unavailable ? <IndexNotice freshness={unavailable} what={name} onRetry={grid.refetch} /> : <IndexNotice freshness={grid.meta?.freshness} what={name} />}
            {view === "error" && !unavailable ? (
              <div className="shop-page__error" role="alert" data-testid="creator-error">
                <p>{t("creator.error")}</p>
                <Button variant="secondary" size="sm" onClick={() => (grid.error ? grid.refetch() : baseline.refetch())}>
                  {t("loadMore.retry")}
                </Button>
              </div>
            ) : view === "no-creations" ? (
              <EmptyState data-testid="creator-empty" icon={illustration(itemsEmptyIllustration)} title={t("creator.emptyNoCreationsTitle")} subtitle={t("creator.emptyNoCreations")} actions={[exploreCta]} />
            ) : view === "filters" ? (
              <EmptyState
                data-testid="creator-empty-filters"
                icon={illustration(searchEmptyIllustration)}
                title={t("creator.emptyFiltersTitle")}
                subtitle={t("creator.emptyFilters", { count: baseline.data ?? 0 })}
                actions={[{ label: t("filterBar.clearAll"), onClick: clearFilters }]}
              />
            ) : unavailable ? null : (
              <>
                <div className="shop-grid" data-testid="grid">
                  {view === "skeletons" ? (
                    <SkeletonCards count={15} />
                  ) : (
                    <>
                      {grid.items.map((item, i) => (
                        <AssetCard key={item.id} item={item} mode={isForSale(item) ? "shop" : "view"} source="creator" position={i} creatorName={profile?.name || null} />
                      ))}
                      {grid.isFetchingNextPage ? <SkeletonCards count={6} /> : null}
                    </>
                  )}
                </div>
                <LoadMore hasNextPage={grid.hasNextPage} isFetching={grid.isFetchingNextPage} isError={grid.isFetchNextPageError} onLoadMore={grid.fetchNextPage} />
              </>
            )}
          </>
        )}
      </BrowseLayout>
    </div>
  );
}

const COLLECTIONS_CATEGORY = "__collections";

function isForSale(item: CatalogItem): boolean {
  return item.priceCredits > 0 || !!item.manaWei;
}

function CollectionsCategory({ selected, onToggle }: { selected: boolean; onToggle: () => void }) {
  const anchor = useRef<HTMLSpanElement>(null);
  const [host, setHost] = useState<Element | null>(null);
  useEffect(() => {
    const root = anchor.current?.parentElement;
    if (!root) return;
    const find = () => {
      const list = root.querySelector('[data-testid="category-filter"]');
      setHost(list && !list.querySelector('[data-testid="filter-collections"]') ? list : null);
    };
    find();
    const observer = new MutationObserver((records) => {
      if (records.some((r) => [...r.addedNodes, ...r.removedNodes].some((n) => n instanceof Element && (n.matches('[data-testid="category-filter"]') || !!n.querySelector('[data-testid="category-filter"]'))))) find();
    });
    observer.observe(root, { childList: true, subtree: true });
    return () => observer.disconnect();
  }, []);
  const row = (
    <div className="shop-cats__group" data-creator-collections>
      <button type="button" className="shop-cats__cat" data-testid="creator-filter-collections" data-selected={selected || undefined} aria-pressed={selected} onClick={onToggle}>
        <span>{t("categories.collections")}</span>
      </button>
    </div>
  );
  return <span ref={anchor} hidden>{host ? createPortal(row, host) : null}</span>;
}
