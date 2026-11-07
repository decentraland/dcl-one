
import { parseEvmAddress } from "../../model/address";
import { fetchCatalogItems, fetchShopItems, type EmoteAttributeFilters } from "../api";
import { DEALS_SORTS, RELEVANCE_SORT, SORTS, type FilterChip } from "../components/FilterBar";
import { STATUSES, type FilterStatus } from "../components/Filters";
import { shopFlags } from "../flags";
import { t } from "../intl";
import { CATEGORIES, SUBCAT_MAP } from "./categories";

export const ITEMS_PAGE_SIZE = 48;

export type ItemsGridScope = {
  contracts?: string[];
  hideNames?: boolean;
  lockStatus?: FilterStatus;
  /** Whether resales may show (market/access.tsx); off, the grid lists primary sales only, as the old shop. */
  marketplace?: boolean;
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
    creators: [] as string[],
    collection: "",
    ...EMOTE_FILTER_DEFAULTS,
    sort: defaultSortFor(!!q, dealsRequested),
  };
}

/** The emote attributes in a browse page's URL state (`emotePlayMode`, `emoteSound`, `emoteProps`). */
export const EMOTE_FILTER_DEFAULTS = { emotePlayMode: "all", emoteSound: false, emoteProps: false };
export type EmoteFilterState = typeof EMOTE_FILTER_DEFAULTS;

/**
 * A browse page's emote filters: the sidebar props (offered only while browsing emotes), the active
 * chips, and the state a category change resets.
 */
export function emoteFilterBindings(category: string, state: EmoteFilterState, set: (patch: Partial<EmoteFilterState>) => void) {
  const emote = emoteAttributes(category, state);
  const browsing = category === "emote";
  const props = {
    emotePlayMode: state.emotePlayMode,
    onEmotePlayMode: browsing ? (v: string) => set({ emotePlayMode: v }) : undefined,
    emoteSound: state.emoteSound,
    onEmoteSound: browsing ? (v: boolean) => set({ emoteSound: v }) : undefined,
    emoteProps: state.emoteProps,
    onEmoteProps: browsing ? (v: boolean) => set({ emoteProps: v }) : undefined,
  };
  const chips: FilterChip[] = [];
  if (emote.emotePlayMode) chips.push({ key: "emotePlayMode", label: t(emote.emotePlayMode === "loop" ? "filter.emoteLoop" : "filter.emotePlayOnce"), onRemove: () => set({ emotePlayMode: "all" }) });
  if (emote.emoteHasSound) chips.push({ key: "emoteSound", label: t("filter.emoteWithSound"), onRemove: () => set({ emoteSound: false }) });
  if (emote.emoteHasGeometry) chips.push({ key: "emoteProps", label: t("filter.emoteWithProps"), onRemove: () => set({ emoteProps: false }) });
  const resetFor = (nextCategory: string): Partial<EmoteFilterState> => (nextCategory === "emote" ? {} : EMOTE_FILTER_DEFAULTS);
  return { emote, props, chips, resetFor };
}

/** The emote attributes of a filter state, sent only while browsing emotes. */
export function emoteAttributes(category: string, state: EmoteFilterState): EmoteAttributeFilters {
  if (category !== "emote") return {};
  const emotePlayMode = state.emotePlayMode === "simple" || state.emotePlayMode === "loop" ? state.emotePlayMode : undefined;
  return { emotePlayMode, emoteHasSound: state.emoteSound || undefined, emoteHasGeometry: state.emoteProps || undefined };
}

export type ItemsFilterState = ReturnType<typeof itemsFilterDefaults>;

export const dealsRequestedIn = (searchParams: URLSearchParams): boolean => shopFlags().creatorSales && searchParams.get("deals") === "true";

export function itemsGrid(filterState: ItemsFilterState, q: string, { contracts, hideNames = false, lockStatus, marketplace = false }: ItemsGridScope = {}) {
  const flags = shopFlags();
  const { subCategory, rarities, priceMin, priceMax, smart, sort } = filterState;
  const defaultStatus = defaultStatusFor(!!q);
  const bodyShape = ["male", "female", "unisex"].includes(filterState.bodyShape) ? filterState.bodyShape : "all";
  // Without the Marketplace there is no listing-type choice: a `?listingType=` in the URL selects nothing, and the
  // reads ask for primary sales only.
  const listingType = marketplace && (filterState.listingType === "primary" || filterState.listingType === "secondary") ? filterState.listingType : undefined;
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
  const emote = emoteAttributes(category, filterState);
  // The creators and collection pickers; a page already scoped to its contracts (an event) offers neither.
  const creators = contracts ? [] : [...new Set(filterState.creators.map(parseEvmAddress).filter((a): a is NonNullable<typeof a> => !!a))];
  const collection = contracts ? null : parseEvmAddress(filterState.collection);
  const selectedContracts = contracts ?? (collection ? [collection] : undefined);

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
    listingType: (marketplace ? listingType : "primary") as "primary" | "secondary" | undefined,
    contractAddresses: selectedContracts?.length ? selectedContracts : undefined,
    creators: creators.length ? creators : undefined,
    ...emote,
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
    // Nothing not for sale has a price, so the range applies to the other statuses only.
    minPriceCredits: status === "not_for_sale" ? undefined : min,
    maxPriceCredits: status === "not_for_sale" ? undefined : max,
    contractAddresses: selectedContracts?.length ? selectedContracts : undefined,
    creators: creators.length ? creators : undefined,
    ...emote,
  };
  const key = category === "names" || selectsNothing ? null : isUnified ? ["shop-items", unifiedFilters] : ["catalog-items", catalogFilters];
  const fetchPage = (skip: number, opts: Parameters<typeof fetchShopItems>[1]) =>
    isUnified ? fetchShopItems({ ...unifiedFilters, first: ITEMS_PAGE_SIZE, skip }, opts) : fetchCatalogItems({ ...catalogFilters, first: ITEMS_PAGE_SIZE, skip }, opts);
  return { defaultStatus, deals, category, status, isUnified, min, max, bodyShape, listingType, wearableGenders, emote, creators, collection, sortOptions, sortOption, key, fetchPage };
}
