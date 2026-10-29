// from: shop@2d3bf62:app/src/pages/MyAssets.tsx; marketplace@f3956826:webapp/src/components/AccountPage (the /account sections);
//       catalyrst-v2 ui3/src/marketplace/pages/{MkAccountPage,MkOnSaleOnRentAccountSections,MkMyBids}.tsx
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { lazy, Suspense, useEffect, useMemo, useState, type ReactNode } from "react";
import { Link, useNavigate, useSearchParams } from "react-router";

import Button from "../../../atoms/Button";
import EmptyState from "../../../components/EmptyState";
import BrowseLayout from "../../discovery/components/BrowseLayout";
import { SortDropdown } from "../../discovery/components/FilterBar";
import Icon from "../../discovery/components/Icon";
import "../../discovery/components/filters.css";
import "../../discovery/components/filterbar.css";
import { RARITIES, rarityColor } from "../../discovery/lib/rarity";
import { shopPath } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { myStoreAccess, useShopCapabilities } from "../../seller/lib/featureFlags";
import { fetchImportable } from "../../seller/lib/import";
import { fetchBidsByAddress, fetchMyAssets, type Bid, type MyAsset, type MyAssetsCategory } from "../lib/api";
import { useArchivedBids } from "../store/archivedBids";
import AssetTile from "../components/AssetTile";
import BidRow from "../components/BidRow";
import { EnsureQueryClient, LoadFailure, Loading } from "../components/kit";
import MyItemsSignedOut from "./myitems/SignedOut";
import "./myitems/myitems.css";

export const MY_ITEMS_SECTIONS = ["wearables", "emotes", "names", "land", "bids"] as const;
export type MyItemsSection = (typeof MY_ITEMS_SECTIONS)[number];

const SECTION_LABEL: Record<MyItemsSection, string> = { wearables: "Wearables", emotes: "Emotes", names: "Names", land: "LAND", bids: "Bids" };
const CATEGORY: Record<Exclude<MyItemsSection, "bids" | "land">, MyAssetsCategory> = { wearables: "wearable", emotes: "emote", names: "ens" };
const SORTS = [
  { key: "newest", label: "Newest" },
  { key: "name", label: "Name (A–Z)" },
  { key: "cheapest", label: "Cheapest" },
] as const;
type SortKey = (typeof SORTS)[number]["key"];
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

type OwnedFilters = { status: Status; rarities: string[]; search: string; sort: SortKey };

function useOwned(address: string, section: Exclude<MyItemsSection, "bids">, f: OwnedFilters) {
  const showRarity = section === "wearables" || section === "emotes";
  const query = useInfiniteQuery({
    queryKey: ["my-assets", address, section, f.status, f.rarities, f.search, f.sort],
    initialPageParam: 0,
    queryFn: async ({ pageParam }) => {
      const base = { first: PAGE_SIZE, skip: pageParam, search: f.search || undefined, sortBy: f.sort, onlyOnSale: f.status === "on_sale" || undefined };
      if (section === "land") {
        const [parcels, estates] = await Promise.all([
          fetchMyAssets(address, { ...base, category: "parcel" }),
          fetchMyAssets(address, { ...base, category: "estate" }),
        ]);
        return { items: [...parcels.assets, ...estates.assets], total: parcels.total + estates.total };
      }
      const res = await fetchMyAssets(address, { ...base, category: CATEGORY[section], rarities: showRarity && f.rarities.length ? f.rarities : undefined });
      return { items: res.assets, total: res.total };
    },
    getNextPageParam: (last, pages) => {
      const loaded = pages.reduce((n, p) => n + p.items.length, 0);
      return loaded < last.total && last.items.length > 0 ? loaded : undefined;
    },
  });
  const items = useMemo(() => {
    const all = query.data?.pages.flatMap((p) => p.items) ?? [];
    return f.status === "not_for_sale" ? all.filter((a) => !a.isOnSale) : all;
  }, [query.data, f.status]);
  const total = f.status === "not_for_sale" ? items.length : (query.data?.pages[0]?.total ?? items.length);
  return { query, items, total };
}

function OwnedGrid({ section, owned, filtered }: { section: Exclude<MyItemsSection, "bids">; owned: ReturnType<typeof useOwned>; filtered: boolean }) {
  const { query, items } = owned;
  const what = SECTION_LABEL[section].toLowerCase();
  if (query.isPending) return <Loading label={`Loading your ${what}…`} />;
  if (query.isError) return <LoadFailure error={query.error} what={`Your ${what}`} onRetry={() => void query.refetch()} />;
  if (items.length === 0)
    return (
      <EmptyState
        variant="inline"
        title={filtered ? "Nothing matches these filters" : `You have no ${what} yet`}
        actions={filtered ? [] : [{ label: "Browse the Shop", href: shopPath("items") }]}
        data-testid="shop-empty"
      />
    );
  return (
    <>
      <div className="shop-grid mk-grid" data-testid="grid">
        {items.map((asset) => (
          <AssetTile key={asset.id} asset={asset} actions={<OwnedActions asset={asset} />} />
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

function Toolbar({
  total,
  loading,
  search,
  onSearch,
  sort,
  onSort,
  onOpenFilters,
  chips,
  onClear,
}: {
  total: number;
  loading: boolean;
  search: string;
  onSearch: (v: string) => void;
  sort: SortKey;
  onSort: (v: SortKey) => void;
  onOpenFilters: () => void;
  chips: { key: string; label: string; onRemove: () => void }[];
  onClear: () => void;
}) {
  return (
    <div className="shop-filterbar" data-testid="browse-toolbar">
      {loading || total > 0 ? (
        <p className="shop-filterbar__count" aria-live="polite" aria-busy={loading || undefined}>
          {loading ? <span className="shop-filterbar__shimmer" aria-hidden /> : `${total} ${total === 1 ? "Item" : "Items"}`}
        </p>
      ) : null}
      {chips.length ? (
        <div className="shop-filterbar__chips">
          {chips.map((c) => (
            <button key={c.key} type="button" className="shop-filterbar__chip" onClick={c.onRemove} aria-label={`Remove ${c.label} filter`}>
              <span>{c.label}</span>
              <Icon name="close" size={14} />
            </button>
          ))}
          <button type="button" className="shop-filterbar__clear" onClick={onClear}>
            Clear all
          </button>
        </div>
      ) : null}
      <div className="shop-myitems__search">
        <Icon name="search" size={18} />
        <input type="search" value={search} placeholder="Search your items" aria-label="Search your items" onChange={(e) => onSearch(e.target.value)} />
        {search ? (
          <button type="button" aria-label="Clear search" onClick={() => onSearch("")}>
            <Icon name="close" size={16} />
          </button>
        ) : null}
      </div>
      <div className="shop-filterbar__right">
        <SortDropdown value={sort} options={SORTS} onChange={(k) => onSort(k as SortKey)} />
        <button type="button" className="shop-filterbar__pill" onClick={onOpenFilters}>
          Filters
          <Icon name="filter" size={16} />
        </button>
      </div>
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
}: {
  address: string;
  section: Exclude<MyItemsSection, "bids">;
  filtersOpen: boolean;
  setFiltersOpen: (v: boolean) => void;
  renderSidebar: (extra: ReactNode) => ReactNode;
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
  const owned = useOwned(address, section, { status, rarities, search, sort });
  const toggleRarity = (r: string) => setRarities((rs) => (rs.includes(r) ? rs.filter((x) => x !== r) : [...rs, r]));

  const chips = [
    ...(status !== "all" ? [{ key: "status", label: STATUS_LABEL[status], onRemove: () => setStatus("all") }] : []),
    ...rarities.map((r) => ({ key: `rarity-${r}`, label: r[0]!.toUpperCase() + r.slice(1), onRemove: () => toggleRarity(r) })),
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
      <Toolbar
        total={owned.total}
        loading={owned.query.isPending}
        search={searchInput}
        onSearch={setSearchInput}
        sort={sort}
        onSort={setSort}
        onOpenFilters={() => setFiltersOpen(true)}
        chips={chips}
        onClear={() => {
          setStatus("all");
          setRarities([]);
        }}
      />
      <OwnedGrid section={section} owned={owned} filtered={status !== "all" || rarities.length > 0 || !!search} />
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
  const shown = section === "creations" && storeOn ? "wearables" : section;
  const [filtersOpen, setFiltersOpen] = useState(false);
  const pick = (s: MyItemsSection | "creations") => {
    setFiltersOpen(false);
    onSection(s);
  };
  const renderSidebar = (extra: ReactNode) => (
    <div className="shop-filters" data-testid="my-items-sidebar">
      <nav className="shop-cats shop-myitems__cats" aria-label="My items sections">
        <h2 className="shop-myitems__heading">Items</h2>
        {MY_ITEMS_SECTIONS.map((s) => (
          <button key={s} type="button" className="shop-cats__cat" data-selected={shown === s || undefined} aria-current={shown === s ? "page" : undefined} onClick={() => pick(s)}>
            <span>{SECTION_LABEL[s]}</span>
          </button>
        ))}
        {storeOn ? (
          <Link className="shop-cats__cat" to={shopPath("myStore")}>
            <span>My store</span>
          </Link>
        ) : (
          <button type="button" className="shop-cats__cat" data-selected={shown === "creations" || undefined} aria-current={shown === "creations" ? "page" : undefined} onClick={() => pick("creations")}>
            <span>Creations</span>
          </button>
        )}
      </nav>
      {extra}
    </div>
  );

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
  return <OwnedMain key={shown} address={address} section={shown} filtersOpen={filtersOpen} setFiltersOpen={setFiltersOpen} renderSidebar={renderSidebar} />;
}

export default function MyItems() {
  const { account } = useShopWallet();
  const caps = useShopCapabilities();
  const [params, setParams] = useSearchParams();
  const storeOn = myStoreAccess(caps, account) === "on";
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
