
import { fetchCatalogItems, fetchShopItems } from "../api";
import { DEALS_SORTS, RELEVANCE_SORT, SORTS } from "../components/FilterBar";
import { STATUSES, type FilterStatus } from "../components/Filters";
import { shopFlags } from "../flags";
import { CATEGORIES, SUBCAT_MAP } from "./categories";

export const ITEMS_PAGE_SIZE = 48;

export type ItemsGridScope = {
  contracts?: string[];
  hideNames?: boolean;
  lockStatus?: FilterStatus;
};

export const defaultStatusFor = (searching: boolean): FilterStatus => (searching ? "all" : "on_sale");
export const defaultSortFor = (searching: boolean, deals: boolean): string => (searching ? "relevance" : deals ? "discount" : "newest");

export function itemsQuery(searchParams: URLSearchParams): { rawQuery: string; q: string } {
  const rawQuery = (searchParams.get("q") ?? "").trim();
  return { rawQuery, q: rawQuery.toLowerCase() };
}

export function itemsFilterDefaults(q: string, dealsRequested: boolean) {
  return {
    category: "all",
    status: defaultStatusFor(!!q) as string,
    subCategory: null as string | null,
    rarities: [] as string[],
    bodyShape: "all",
    listingType: "all",
    priceMin: "",
    priceMax: "",
    smart: false,
    deals: false,
    sort: defaultSortFor(!!q, dealsRequested),
  };
}

export type ItemsFilterState = ReturnType<typeof itemsFilterDefaults>;

export const dealsRequestedIn = (searchParams: URLSearchParams): boolean => shopFlags().creatorSales && searchParams.get("deals") === "true";

export function itemsGrid(filterState: ItemsFilterState, q: string, { contracts, hideNames = false, lockStatus }: ItemsGridScope = {}) {
  const flags = shopFlags();
  const { subCategory, rarities, priceMin, priceMax, smart, sort } = filterState;
  const defaultStatus = defaultStatusFor(!!q);
  const bodyShape = ["male", "female", "unisex"].includes(filterState.bodyShape) ? filterState.bodyShape : "all";
  const listingType = filterState.listingType === "primary" || filterState.listingType === "secondary" ? filterState.listingType : undefined;
  const deals = flags.creatorSales && filterState.deals;
  const offered = CATEGORIES.map((c) => c.key).filter((key) => !(hideNames && key === "names"));
  const category = offered.includes(filterState.category) ? filterState.category : "all";
  const status: FilterStatus = lockStatus ?? (STATUSES.includes(filterState.status as FilterStatus) ? (filterState.status as FilterStatus) : defaultStatus);
  const selectsNothing = contracts !== undefined && contracts.length === 0;
  const isUnified = status === "on_sale";
  const min = priceMin && !Number.isNaN(Number(priceMin)) ? Number(priceMin) : undefined;
  const max = priceMax && !Number.isNaN(Number(priceMax)) ? Number(priceMax) : undefined;
  const wearableCategories = subCategory ? SUBCAT_MAP[subCategory] : undefined;
  const wearableGenders = category !== "emote" && bodyShape !== "all" ? [bodyShape] : undefined;
  const sortOptions = [...(q ? [RELEVANCE_SORT] : []), ...(deals ? DEALS_SORTS : SORTS)];
  const sortOption = sortOptions.find((s) => s.key === sort) ?? sortOptions[0]!;

  const unifiedFilters = {
    category,
    rarities: rarities.length ? rarities : undefined,
    wearableCategories,
    wearableGenders,
    minPriceCredits: min,
    maxPriceCredits: max,
    search: q || undefined,
    sortBy: sortOption.server,
    isSmart: smart || undefined,
    onSale: true,
    discounted: deals || undefined,
    listingType: (flags.secondarySales ? listingType : "primary") as "primary" | "secondary" | undefined,
    contractAddresses: contracts?.length ? contracts : undefined,
  };
  const catalogFilters = {
    category,
    rarities: rarities.length ? rarities : undefined,
    wearableCategories,
    wearableGenders,
    search: q || undefined,
    sortBy: sortOption.server,
    isWearableSmart: smart || undefined,
    isOnSale: status === "not_for_sale" ? false : undefined,
    contractAddresses: contracts?.length ? contracts : undefined,
  };
  const key = category === "names" || selectsNothing ? null : isUnified ? ["shop-items", unifiedFilters] : ["catalog-items", catalogFilters];
  const fetchPage = (skip: number, opts: Parameters<typeof fetchShopItems>[1]) =>
    isUnified ? fetchShopItems({ ...unifiedFilters, first: ITEMS_PAGE_SIZE, skip }, opts) : fetchCatalogItems({ ...catalogFilters, first: ITEMS_PAGE_SIZE, skip }, opts);
  return { defaultStatus, deals, category, status, isUnified, min, max, bodyShape, listingType, wearableGenders, sortOptions, sortOption, key, fetchPage };
}
