// from: shop@3a322cf9:app/src/pages/MyAssets.tsx (sidebar, toolbar, empty state); marketplace@f3956826:webapp/src/components/AccountPage (the /account sections);
//       catalyrst-v2 ui3/src/marketplace/pages/{MkAccountPage,MkOnSaleOnRentAccountSections,MkMyBids}.tsx
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { lazy, Suspense, useEffect, useMemo, useState, type ReactNode } from "react";
import { Link, useNavigate, useSearchParams } from "react-router";

import Button from "../../../atoms/Button";
import EmptyState from "../../../components/EmptyState";
import BrowseLayout from "../../discovery/components/BrowseLayout";
import FilterBar, { type SortOption } from "../../discovery/components/FilterBar";
import { CategoryFilter, subCategoryLabel } from "../../discovery/components/Filters";
import Icon from "../../discovery/components/Icon";
import "../../discovery/components/filters.css";
import "../../discovery/components/filterbar.css";
import { CATEGORIES, SUBCAT_MAP } from "@shop/discovery/lib/categories";
import { RARITIES, rarityColor } from "@shop/discovery/lib/rarity";
import itemsEmptyIllustration from "../../components/empty-art/items-empty.svg?url";
import salesEmptyIllustration from "../../components/empty-art/sales-empty.svg?url";
import { shopPath } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { useMyStoreAccess } from "../../seller/lib/featureFlags";
import { fetchImportable } from "../../seller/lib/import";
import { fetchBidsByAddress, fetchMyAssets, type Bid, type MyAsset, type MyAssetsCategory } from "../lib/api";
import { useArchivedBids } from "../store/archivedBids";
import { useMarketplace, useMarketplaceAccess } from "../access";
import AssetTile from "../components/AssetTile";
import { EmptyPanel } from "../../components/EmptyPanel";
import BidRow from "../components/BidRow";
import { EnsureQueryClient, LoadFailure, Loading } from "../components/kit";
import MyItemsSignedOut from "./myitems/SignedOut";
import "./myitems/myitems.css";

export const MY_ITEMS_SECTIONS = ["wearables", "emotes", "names", "land", "bids"] as const;
export type MyItemsSection = (typeof MY_ITEMS_SECTIONS)[number];
/** The sections that are the Marketplace's, shown only with it on (market/access.tsx). */
const MARKETPLACE_SECTIONS: ReadonlySet<MyItemsSection | "creations"> = new Set(["land", "bids"]);

const SECTION_LABEL: Record<MyItemsSection, string> = { wearables: "Wearables", emotes: "Emotes", names: "Names", land: "LAND", bids: "Bids" };
const CATEGORY: Record<Exclude<MyItemsSection, "bids" | "land">, MyAssetsCategory> = { wearables: "wearable", emotes: "emote", names: "ens" };
const SORTS = [
  { key: "newest", label: "filterBar.sortNewest", server: "newest" },
  { key: "name", label: "filterBar.sortName", server: "name" },
  { key: "cheapest", label: "filterBar.sortCheapest", server: "cheapest" },
] as const satisfies readonly SortOption[];
type SortKey = (typeof SORTS)[number]["key"];
/** The sidebar's CategoryFilter keys for the sections it lists (upstream's CATEGORY_OF_SECTION). */
const CATEGORY_OF_SECTION: Partial<Record<MyItemsSection | "creations", string>> = { wearables: "wearable", emotes: "emote", names: "names" };
const SECTION_OF_CATEGORY = Object.fromEntries(Object.entries(CATEGORY_OF_SECTION).map(([section, category]) => [category, section])) as Record<string, MyItemsSection>;

/** The section a sub-category (or one of its leaves) belongs to: Head is a wearable's, Dance an emote's. */
function sectionOfSub(key: string): MyItemsSection | null {
  for (const top of CATEGORIES)
    for (const sub of top.subs ?? []) if (sub.key === key || sub.subs?.some((leaf) => leaf.key === key)) return SECTION_OF_CATEGORY[top.key] ?? null;
  return null;
}
type Status = "all" | "on_sale" | "not_for_sale";
const STATUS_LABEL: Record<Status, string> = { all: "All", on_sale: "On sale", not_for_sale: "Not for sale" };
const PAGE_SIZE = 48;

function sectionOf(raw: string | null): MyItemsSection {
  switch (raw) {
    case "emotes":
      return "emotes";
    case "ens":
    case "names":
      return "names";
    case "land":
    case "parcels":
    case "estates":
    case "on_rent":
      return "land";
    case "bids":
      return "bids";
    default:
      return "wearables";
  }
}

export function OwnedActions({ asset }: { asset: MyAsset }) {
  const params = { contractAddress: asset.contractAddress, tokenId: asset.tokenId };
  return (
    <>
      <Link className="mk-link" to={shopPath("tokenSell", params)}>
        {asset.isOnSale ? "Update price" : "Sell"}
      </Link>
      {asset.isOnSale ? (
        <Link className="mk-link" to={shopPath("tokenCancel", params)}>
          Remove from sale
        </Link>
      ) : null}
      <Link className="mk-link" to={shopPath("tokenTransfer", params)}>
        Transfer
      </Link>
      {asset.category === "ens" || asset.category === "parcel" || asset.category === "estate" ? (
        <Link className="mk-link" to={shopPath("tokenManage", params)}>
          Manage
        </Link>
      ) : null}
      {asset.category === "parcel" || asset.category === "estate" ? (
        <Link className="mk-link" to={shopPath("tokenRent", params)}>
          {asset.openRentalId ? "Rental" : "Rent out"}
        </Link>
      ) : null}
    </>
  );
}

function ImportNudge({ address }: { address: string }) {
  const { data } = useQuery({ queryKey: ["importable", address], queryFn: () => fetchImportable(address), staleTime: 5 * 60_000, retry: false });
  const count = data?.creations.length ?? 0;
  if (count === 0) return null;
  return (
    <div className="mk-note" role="status" data-testid="shop-import-nudge">
      <p>
        {count === 1 ? "One of your creations is" : `${count} of your creations are`} still listed in MANA only. Move {count === 1 ? "it" : "them"} into the Shop so buyers can
        pay with Credits too.
      </p>
      <Link className="mk-link" to="/shop/activity?section=listings">
        Review listings
      </Link>
    </div>
  );
}

type OwnedFilters = { status: Status; rarities: string[]; subCategory: string | null; search: string; sort: SortKey };

function useOwned(address: string, section: Exclude<MyItemsSection, "bids">, f: OwnedFilters) {
  const showRarity = section === "wearables" || section === "emotes";
  const query = useInfiniteQuery({
    queryKey: ["my-assets", address, section, f.status, f.rarities, f.subCategory, f.search, f.sort],
    initialPageParam: 0,
    queryFn: async ({ pageParam }) => {
      const base = {
        first: PAGE_SIZE,
        skip: pageParam,
        search: f.search || undefined,
        sortBy: f.sort,
        onlyOnSale: f.status === "on_sale" || undefined,
        onlyNotOnSale: f.status === "not_for_sale" || undefined,
      };
      if (section === "land") {
        const [parcels, estates] = await Promise.all([
          fetchMyAssets(address, { ...base, category: "parcel" }),
          fetchMyAssets(address, { ...base, category: "estate" }),
        ]);
        return { items: [...parcels.assets, ...estates.assets], total: parcels.total + estates.total };
      }
      const sub = showRarity && f.subCategory ? SUBCAT_MAP[f.subCategory] : undefined;
      const res = await fetchMyAssets(address, {
        ...base,
        category: CATEGORY[section],
        rarities: showRarity && f.rarities.length ? f.rarities : undefined,
        wearableCategories: section === "wearables" ? sub : undefined,
        emoteCategories: section === "emotes" ? sub : undefined,
      });
      return { items: res.assets, total: res.total };
    },
    getNextPageParam: (last, pages) => {
      const loaded = pages.reduce((n, p) => n + p.items.length, 0);
      return loaded < last.total && last.items.length > 0 ? loaded : undefined;
    },
  });
  const items = useMemo(() => query.data?.pages.flatMap((p) => p.items) ?? [], [query.data]);
  const total = query.data?.pages[0]?.total ?? items.length;
  return { query, items, total };
}

function OwnedGrid({
  section,
  owned,
  onSale,
  filtered,
  onClear,
}: {
  section: Exclude<MyItemsSection, "bids">;
  owned: ReturnType<typeof useOwned>;
  onSale: boolean;
  /** A search, status, rarity or sub-category narrows the list: an empty grid is then no match, not an empty backpack. */
  filtered: boolean;
  onClear: () => void;
}) {
  const { query, items } = owned;
  const marketplace = useMarketplace();
  const what = SECTION_LABEL[section].toLowerCase();
  if (query.isPending) return <Loading label={`Loading your ${what}…`} />;
  if (query.isError) return <LoadFailure error={query.error} what={`Your ${what}`} onRetry={() => void query.refetch()} />;
  if (items.length === 0) {
    // As upstream's owned-empty: a "nothing on sale" grid is a listings state, not an empty backpack.
    const icon = onSale ? salesEmptyIllustration : itemsEmptyIllustration;
    if (filtered) return <EmptyPanel testId="owned-empty" icon={icon} title="Nothing matches these filters" body="Try other filters, or clear them to see everything you own." cta={{ label: "Clear all", onClick: onClear }} />;
    if (section === "names") return <EmptyPanel testId="owned-empty" icon={itemsEmptyIllustration} title="No NAMEs yet" body="Claim a unique NAME to stand out in Decentraland — it’ll appear here once it’s yours." />;
    return <EmptyPanel testId="owned-empty" icon={icon} title="Nothing here... yet!" body="Explore the Shop and find something awesome to make it yours." cta={{ label: "Explore Shop", to: shopPath("items") }} />;
  }
  return (
    <>
      <div className="shop-grid mk-grid" data-testid="grid">
        {items.map((asset) => (
          <AssetTile key={asset.id} asset={asset} actions={marketplace ? <OwnedActions asset={asset} /> : undefined} />
        ))}
      </div>
      {query.hasNextPage ? (
        <div className="mk-more">
          <Button variant="secondary" disabled={query.isFetchingNextPage} onClick={() => void query.fetchNextPage()}>
            {query.isFetchingNextPage ? "Loading…" : "Load more"}
          </Button>
        </div>
      ) : null}
    </>
  );
}

function FilterGroup({ title, summary, defaultOpen = true, children }: { title: string; summary?: string; defaultOpen?: boolean; children: ReactNode }) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <section className="shop-filters__section">
      <button type="button" className="shop-filters__head" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        <span className="shop-filters__title">{title}</span>
        <Icon name="chevron-down" size={20} className={open ? "shop-cats__chev shop-cats__chev--open" : "shop-cats__chev"} />
      </button>
      {!open && summary ? <p className="shop-filters__summary">{summary}</p> : null}
      <div hidden={!open} className="shop-filters__content">
        {children}
      </div>
    </section>
  );
}

/** from: shop@3a322cf9:app/src/pages/MyAssets.styles.ts (SearchBar): the page's own search, in the toolbar beside Sort By. */
function MySearch({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  return (
    <div className="shop-myitems__search">
      <Icon name="search" size={18} />
      <input type="search" value={value} placeholder="Search your items" aria-label="Search your items" onChange={(e) => onChange(e.target.value)} />
      {value ? (
        <button type="button" aria-label="Clear search" onClick={() => onChange("")}>
          <Icon name="close" size={12} />
        </button>
      ) : null}
    </div>
  );
}

export function BidsSection({ address, bidId = null }: { address: string; bidId?: string | null }) {
  const [tab, setTab] = useState<"received" | "made">("received");
  const [showArchived, setShowArchived] = useState(false);
  const archived = useArchivedBids(address);
  const navigate = useNavigate();
  const query = useQuery({ queryKey: ["bids-by-address", address], queryFn: () => fetchBidsByAddress(address) });
  const linked = bidId && query.data ? (query.data.received.find((b) => b.id === bidId) ?? null) : null;
  useEffect(() => {
    if (linked?.tokenId) {
      navigate(`${shopPath("tokenBids", { contractAddress: linked.contractAddress.toLowerCase(), tokenId: linked.tokenId })}?bid=${encodeURIComponent(linked.id)}`, { replace: true });
    }
  }, [linked, navigate]);
  const rows: Bid[] = useMemo(() => {
    const list = tab === "received" ? (query.data?.received ?? []) : (query.data?.made ?? []);
    if (tab === "made") return list;
    return list.filter((b) => archived.includes(b.id) === showArchived);
  }, [query.data, tab, archived, showArchived]);

  return (
    <div className="mk-section" aria-label="Bids">
      <div className="mk-tabs" role="tablist" aria-label="Bids">
        {(["received", "made"] as const).map((t) => (
          <button key={t} type="button" role="tab" aria-selected={tab === t} className="mk-tab" onClick={() => setTab(t)}>
            {t === "received" ? "Received" : "Made"}
          </button>
        ))}
        {tab === "received" ? (
          <label className="mk-inline-check">
            <input type="checkbox" checked={showArchived} onChange={(e) => setShowArchived(e.target.checked)} /> Show archived
          </label>
        ) : null}
      </div>
      <p className="mk-hint">Bids are paid in MANA. Accepting one sends your item to the bidder and the MANA to you.</p>
      {bidId && query.data && !linked ? (
        <p className="mk-note" role="status" data-testid="shop-bid-from-link">
          The bid from your link is no longer open.
        </p>
      ) : null}
      {query.isPending ? (
        <Loading label="Loading bids…" />
      ) : query.isError ? (
        <LoadFailure error={query.error} what="Your bids" onRetry={() => void query.refetch()} />
      ) : rows.length === 0 ? (
        <EmptyState variant="inline" title={tab === "received" ? (showArchived ? "No archived bids" : "No bids on your items yet") : "You have not placed any bids"} data-testid="shop-empty" />
      ) : (
        <ul className="mk-list">
          {rows.map((bid) => (
            <BidRow key={bid.id} bid={bid} viewer={address} archived={archived.includes(bid.id)} />
          ))}
        </ul>
      )}
    </div>
  );
}

const CreationsSection = lazy(async () => ({ default: (await import("../../seller/pages/MyStore")).MyStoreView }));

function OwnedMain({
  address,
  section,
  filtersOpen,
  setFiltersOpen,
  renderSidebar,
  subCategory,
  onSubCategory,
}: {
  address: string;
  section: Exclude<MyItemsSection, "bids">;
  filtersOpen: boolean;
  setFiltersOpen: (v: boolean) => void;
  renderSidebar: (extra: ReactNode) => ReactNode;
  subCategory: string | null;
  onSubCategory: (key: string | null) => void;
}) {
  const [status, setStatus] = useState<Status>("all");
  const [rarities, setRarities] = useState<string[]>([]);
  const [searchInput, setSearchInput] = useState("");
  const [search, setSearch] = useState("");
  const [sort, setSort] = useState<SortKey>("newest");
  useEffect(() => {
    const id = setTimeout(() => setSearch(searchInput.trim()), 300);
    return () => clearTimeout(id);
  }, [searchInput]);
  const showRarity = section === "wearables" || section === "emotes";
  const owned = useOwned(address, section, { status, rarities, subCategory: showRarity ? subCategory : null, search, sort });
  const toggleRarity = (r: string) => setRarities((rs) => (rs.includes(r) ? rs.filter((x) => x !== r) : [...rs, r]));

  // The chip bar's Clear all clears the chips; the search is not one and stays. The empty panel's clears everything.
  const clearChips = () => {
    setStatus("all");
    setRarities([]);
    onSubCategory(null);
  };
  const clearAll = () => {
    clearChips();
    setSearchInput("");
    setSearch("");
  };
  const filtered = status !== "all" || rarities.length > 0 || !!search || (showRarity && !!subCategory);

  const chips = [
    ...(status !== "all" ? [{ key: "status", label: STATUS_LABEL[status], onRemove: () => setStatus("all") }] : []),
    ...rarities.map((r) => ({ key: `rarity-${r}`, label: r[0]!.toUpperCase() + r.slice(1), onRemove: () => toggleRarity(r) })),
    ...(showRarity && subCategory ? [{ key: "sub", label: subCategoryLabel(subCategory), onRemove: () => onSubCategory(null) }] : []),
  ];

  const filters = (
    <>
      {showRarity ? (
        <FilterGroup title="Rarity" defaultOpen={false} summary={RARITIES.filter((r) => rarities.includes(r)).join(", ")}>
          <div className="shop-filters__rarities" role="group" aria-label="Rarity" data-testid="rarity-filter">
            {RARITIES.map((r) => {
              const on = rarities.includes(r);
              return (
                <button key={r} type="button" className="shop-filters__rarity" data-rarity={r} aria-pressed={on} onClick={() => toggleRarity(r)}>
                  <span className="shop-filters__swatch" style={{ background: rarityColor(r) }}>
                    {on ? <Icon name="check" size={12} /> : null}
                  </span>
                  <span className="shop-myitems__rarityname">{r}</span>
                </button>
              );
            })}
          </div>
        </FilterGroup>
      ) : null}
      <FilterGroup title="Status" summary={status === "all" ? "" : STATUS_LABEL[status]}>
        <div role="group" aria-label="Status">
          {(["all", "on_sale", "not_for_sale"] as Status[]).map((s) => (
            <button key={s} type="button" className="shop-filters__radio shop-myitems__radio" aria-pressed={status === s} onClick={() => setStatus(s)}>
              <span className="shop-myitems__dot" aria-hidden />
              <span>{STATUS_LABEL[s]}</span>
            </button>
          ))}
        </div>
      </FilterGroup>
    </>
  );

  return (
    <BrowseLayout sidebar={renderSidebar(filters)} open={filtersOpen} onClose={() => setFiltersOpen(false)} testId="my-items-browse">
      <ImportNudge address={address} />
      <FilterBar
        sort={sort}
        onSort={(k) => setSort(k as SortKey)}
        sortOptions={[...SORTS]}
        // A failed read has no count: "0 Items" over an outage would read as an empty backpack.
        total={owned.query.isError ? null : owned.total}
        loading={owned.query.isPending}
        query={search}
        onOpenFilters={() => setFiltersOpen(true)}
        chips={chips}
        onClearChips={clearChips}
        search={<MySearch value={searchInput} onChange={setSearchInput} />}
      />
      <OwnedGrid section={section} owned={owned} onSale={status === "on_sale"} filtered={filtered} onClear={clearAll} />
    </BrowseLayout>
  );
}

export function MyItemsView({
  address,
  section,
  onSection,
  storeOn = true,
  bidId = null,
}: {
  address: string;
  section: MyItemsSection | "creations";
  onSection: (s: MyItemsSection | "creations") => void;
  storeOn?: boolean;
  bidId?: string | null;
}) {
  // LAND and Bids are the Marketplace's (market/access.tsx): without it, they show the wearables instead. While the
  // answer is pending a link to them waits, rather than mounting the wearables (and their reads) first.
  const access = useMarketplaceAccess();
  const marketplace = access === "on";
  const shown = (section === "creations" && storeOn) || (!marketplace && section !== "creations" && MARKETPLACE_SECTIONS.has(section)) ? "wearables" : section;
  const [filtersOpen, setFiltersOpen] = useState(false);
  const waiting = access === "pending" && section !== "creations" && MARKETPLACE_SECTIONS.has(section);
  // A sub-category filters the section it was picked in: any section change (the sidebar, the header's My Assets, a
  // ?section= link, Back) drops it, as it drops the section's other filters (OwnedMain is keyed by section).
  const [subCategory, setSubCategory] = useState<string | null>(null);
  const [subFor, setSubFor] = useState(shown);
  if (subFor !== shown) {
    setSubFor(shown);
    setSubCategory(null);
  }
  const pick = (s: MyItemsSection | "creations") => {
    setFiltersOpen(false);
    onSection(s);
  };
  // Only the shown section's group is open (CategoryFilter collapses the others), so a picked sub-category is its.
  const pickSub = (key: string | null) => {
    if (key === null || sectionOfSub(key) === shown) setSubCategory(key);
  };
  // As upstream: the Collectibles category nav (Wearables and Emotes open to their sub-categories), with no
  // "Shop All" and with "My Creations" as its extra entry while the store dashboard is off. dcl-one's own
  // sections (LAND, Bids, My store) follow it.
  const renderSidebar = (extra: ReactNode) => (
    <div className="shop-filters" data-testid="my-items-sidebar">
      <nav className="shop-myitems__nav" aria-label="My items sections">
        <CategoryFilter
          title="Items"
          category={CATEGORY_OF_SECTION[shown] ?? ""}
          subCategory={subCategory}
          onCategory={(key) => {
            const next = SECTION_OF_CATEGORY[key];
            if (!next || next === shown) return;
            // Wearables and Emotes open their sub-categories in place: the mobile drawer stays open to choose one.
            if (CATEGORIES.some((top) => top.key === key && top.subs)) onSection(next);
            else pick(next);
          }}
          onSub={pickSub}
          onCollections={storeOn ? undefined : () => pick("creations")}
          collections={!storeOn && shown === "creations"}
          extraLabel="My Creations"
          hideAll
        />
        <div className="shop-cats shop-myitems__cats">
          {(marketplace ? (["land", "bids"] as const) : []).map((s) => (
            <button key={s} type="button" className="shop-cats__cat" data-selected={shown === s || undefined} aria-pressed={shown === s} onClick={() => pick(s)}>
              <span>{SECTION_LABEL[s]}</span>
            </button>
          ))}
          {storeOn ? (
            <Link className="shop-cats__cat" to={shopPath("myStore")}>
              <span>My store</span>
            </Link>
          ) : null}
        </div>
      </nav>
      {extra}
    </div>
  );

  if (waiting) return <Loading label="Loading your items…" />;
  if (shown === "creations" || shown === "bids") {
    return (
      <BrowseLayout sidebar={renderSidebar(null)} open={filtersOpen} onClose={() => setFiltersOpen(false)} testId="my-items-browse">
        <ImportNudge address={address} />
        <div className="shop-filterbar">
          <div className="shop-filterbar__right">
            <button type="button" className="shop-filterbar__pill" onClick={() => setFiltersOpen(true)}>
              Filters
              <Icon name="filter" size={16} />
            </button>
          </div>
        </div>
        {shown === "creations" ? (
          <Suspense fallback={<Loading label="Loading your creations…" />}>
            <CreationsSection address={address} />
          </Suspense>
        ) : (
          <BidsSection address={address} bidId={bidId} />
        )}
      </BrowseLayout>
    );
  }
  return (
    <OwnedMain
      key={shown}
      address={address}
      section={shown}
      filtersOpen={filtersOpen}
      setFiltersOpen={setFiltersOpen}
      renderSidebar={renderSidebar}
      subCategory={subCategory}
      onSubCategory={pickSub}
    />
  );
}

export default function MyItems() {
  const { account, resolving } = useShopWallet();
  const [params, setParams] = useSearchParams();
  const storeOn = useMyStoreAccess(account, resolving) === "on";
  const section = params.get("section") === "creations" && !storeOn ? "creations" : sectionOf(params.get("section"));
  const onSection = (next: MyItemsSection | "creations") =>
    setParams(
      (prev) => {
        const p = new URLSearchParams(prev);
        p.set("section", next);
        return p;
      },
      { replace: true },
    );
  return (
    <EnsureQueryClient>
      <section className="shop-myitems" data-testid="my-items">
        <h1 className="u-sr-only">My items</h1>
        {account ? <MyItemsView address={account} section={section} onSection={onSection} storeOn={storeOn} bidId={params.get("bid")} /> : <MyItemsSignedOut />}
      </section>
    </EnsureQueryClient>
  );
}
