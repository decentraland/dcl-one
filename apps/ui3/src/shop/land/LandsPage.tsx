// from: marketplace@3bf3662f:webapp/src/components/LandsPage/LandsPage.tsx, webapp/src/components/AssetBrowse/
// {AssetBrowse.tsx,AssetBrowse.css,MapBrowse/MapBrowse.tsx,MapTopbar/MapTopbar.tsx}, webapp/src/components/
// AssetTopbar/AssetTopbar.tsx, webapp/src/components/Vendor/NFTSidebar (the LAND categories), webapp/src/
// components/AssetFilters/{AssetFilters.tsx,LandStatusFilter,PriceFilter,EstateSizeFilter,RentalPeriodFilter,
// LocationFilter}/*.tsx, webapp/src/modules/ui/browse (the LAND section), webapp/src/components/AssetList/
// AssetList.tsx (infinite scroll)
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useNavigate, useSearchParams } from "react-router";

import Button from "../../atoms/Button";
import ManaMark from "../../atoms/ManaMark";
import EmptyState from "../../components/EmptyState";
import type { ShopClient } from "../api/shop";
import { chainLabel, gasTokenSymbol } from "../model/chain";
import { reasonText } from "../payment/copy";
import { useShopWallet } from "../wallet/context";
import {
  fetchLandNfts,
  fetchOwnedLand,
  indexOutage,
  LAND_SORT_LABEL,
  landFiltersFromSearch,
  landFiltersToSearch,
  landParcels,
  landSortsFor,
  LAND_PAGE_SIZE,
  type NftResult,
  type LandCategoryFilter,
  type LandFilters,
  type LandStatusFilter,
} from "./api";
import { landChainId } from "./contracts";
import LandCard from "./LandCard";
import LandMap from "./LandMap";
import LandPriceChart from "./LandPriceChart";
import { daysByPeriod, PERIOD_LABEL, PERIOD_OPTIONS } from "./rentals";
import { fetchTiles, MapNotReadyError, nearestTile, type TilesResult } from "./tiles";
import { peekShopQuery } from "../discovery/query";
import { useResource } from "./useResource";
import "./land.css";
import "./lands.css";

const STATUS_LABEL: Record<LandStatusFilter, string> = { all: "All land", sale: "Available to buy", rent: "Available to rent" };
const CATEGORIES: [LandCategoryFilter, string][] = [
  ["land", "Land"],
  ["parcel", "Parcels"],
  ["estate", "Estates"],
];
const PLAZA_MIN = "1";
const PLAZA_MAX = "10";

export type LandsPageProps = {
  client?: ShopClient;
  loadTiles?: (signal: AbortSignal) => Promise<TilesResult>;
};

export function LandWalletNotice() {
  const { session, host } = useShopWallet();
  if (session.kind === "web3") return null;
  const chainId = landChainId();
  return (
    <div className="shop-land-note shop-land-note--warn shop-lb-note" role="note">
      <p>
        {reasonText("landOrRental")} Buying or renting LAND needs a web3 wallet holding MANA on {chainLabel(chainId)}, and some{" "}
        {gasTokenSymbol(chainId)} there to pay the network fee.
      </p>
      {session.kind === "signedOut" ? (
        host?.onSignIn ? (
          <Button size="sm" onClick={() => host.onSignIn?.()}>
            Sign in with a wallet
          </Button>
        ) : null
      ) : host?.onConnectWallet ? (
        <Button size="sm" onClick={() => host.onConnectWallet?.()}>
          Connect a wallet
        </Button>
      ) : null}
    </div>
  );
}

export const landsListingKey = (listKey: string) => ["land-nfts", listKey] as const;

export const landsListKey = (filters: LandFilters, owner: string | null) => `${landFiltersToSearch({ ...filters, page: 1 })}|${owner ?? ""}`;

export function landsFiltersFromParams(params: URLSearchParams): LandFilters {
  const q = new URLSearchParams(params);
  if (q.get("onlyOnRent") !== "true") {
    if (q.get("onlyOnSale") === "true") q.delete("onlyOnSale");
    else q.set("onlyOnSale", "false");
  }
  q.delete("isMap");
  q.delete("owned");
  return landFiltersFromSearch(`?${q}`);
}

export function landsSearch(f: LandFilters): URLSearchParams {
  const q = new URLSearchParams(landFiltersToSearch(f));
  if (f.status === "all") q.delete("onlyOnSale");
  if (f.status === "sale") q.set("onlyOnSale", "true");
  return q;
}

function SearchGlyph() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="10.5" cy="10.5" r="6.5" fill="none" stroke="currentColor" strokeWidth="3" />
      <path d="M15.5 15.5 21 21" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
    </svg>
  );
}
function TableGlyph() {
  return (
    <svg width="16" height="14" viewBox="0 0 16 14" aria-hidden="true">
      <path fill="currentColor" d="M1.5 0h13A1.5 1.5 0 0 1 16 1.5v11a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 0 12.5v-11A1.5 1.5 0 0 1 1.5 0ZM2 4v3.5h5V4H2Zm7 0v3.5h5V4H9ZM2 9.5V12h5V9.5H2Zm7 0V12h5V9.5H9Z" />
    </svg>
  );
}
function MarkerGlyph() {
  return (
    <svg width="12" height="16" viewBox="0 0 12 16" aria-hidden="true">
      <path fill="currentColor" d="M6 0a6 6 0 0 0-6 6c0 4.5 6 10 6 10s6-5.5 6-10a6 6 0 0 0-6-6Zm0 8.5A2.5 2.5 0 1 1 6 3.5a2.5 2.5 0 0 1 0 5Z" />
    </svg>
  );
}
function FiltersGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M3 6h11M18 6h3M3 12h3M10 12h11M3 18h13M20 18h1" stroke="currentColor" strokeWidth="2" strokeLinecap="round" fill="none" />
      <circle cx="16" cy="6" r="2" fill="none" stroke="currentColor" strokeWidth="2" />
      <circle cx="8" cy="12" r="2" fill="none" stroke="currentColor" strokeWidth="2" />
      <circle cx="18" cy="18" r="2" fill="none" stroke="currentColor" strokeWidth="2" />
    </svg>
  );
}
function ChevronGlyph({ open }: { open: boolean }) {
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" style={{ transform: open ? "none" : "rotate(180deg)" }}>
      <path d="M2 8l4-4 4 4" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function Box({ title, children }: { title: string; children: ReactNode }) {
  const [open, setOpen] = useState(true);
  return (
    <section className="shop-lb-box" aria-label={title}>
      <button type="button" className="shop-lb-box__header" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        <span>{title}</span>
        <ChevronGlyph open={open} />
      </button>
      {open ? <div className="shop-lb-box__body">{children}</div> : null}
    </section>
  );
}

function ViewToggle({ isMap, onChange }: { isMap: boolean; onChange(isMap: boolean): void }) {
  return (
    <div className="shop-lb-toggle" role="group" aria-label="View">
      <button type="button" aria-label="Grid" title="Grid" aria-pressed={!isMap} onClick={() => onChange(false)}>
        <TableGlyph />
      </button>
      <button type="button" aria-label="Map" title="Map" aria-pressed={isMap} onClick={() => onChange(true)}>
        <MarkerGlyph />
      </button>
    </div>
  );
}

const MAX_PAGE = 50;

export function landsListingFilters(f: LandFilters, owner: string | null, page = 1): LandFilters {
  return { ...f, owner, page, minEstateSize: f.minEstateSize || (f.category === "parcel" ? "" : "1") };
}

function useWide(query = "(min-width: 769px)"): boolean {
  const [wide, setWide] = useState(false);
  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") return;
    const m = window.matchMedia(query);
    const on = () => setWide(m.matches);
    on();
    m.addEventListener?.("change", on);
    return () => m.removeEventListener?.("change", on);
  }, [query]);
  return wide;
}

function Sentinel({ onVisible, active }: { onVisible(): void; active: boolean }) {
  const ref = useRef<HTMLDivElement | null>(null);
  const cb = useRef(onVisible);
  cb.current = onVisible;
  useEffect(() => {
    const el = ref.current;
    if (!active || !el || typeof IntersectionObserver === "undefined") return;
    const io = new IntersectionObserver(([entry]) => entry?.isIntersecting && cb.current(), { root: null, rootMargin: "0px", threshold: 0.1 });
    io.observe(el);
    return () => io.disconnect();
  }, [active]);
  return <div ref={ref} className="shop-lb-sentinel" aria-hidden="true" />;
}

export default function LandsPage({ client, loadTiles }: LandsPageProps) {
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const { account } = useShopWallet();
  const filters = useMemo(() => landsFiltersFromParams(params), [params]);
  const isMap = params.get("isMap") === "true";
  const mapSale = params.get("onlyOnSale") === "true" || (params.get("onlyOnSale") === null && params.get("onlyOnRent") !== "true");
  const mapRent = params.get("onlyOnRent") === "true";
  const showOwned = !!account && (isMap ? params.get("owned") !== "false" : params.get("owned") === "true");
  const [searchDraft, setSearchDraft] = useState(filters.search);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const wide = useWide();

  const update = (patch: Partial<LandFilters>, extra: Record<string, string | null> = {}) => {
    const next: LandFilters = { ...filters, ...patch, page: 1 };
    if (patch.status && !landSortsFor(next.status).includes(next.sort)) next.sort = landSortsFor(next.status)[0]!;
    const q = landsSearch(next);
    if (isMap) q.set("isMap", "true");
    if (params.has("owned")) q.set("owned", params.get("owned")!);
    for (const [k, v] of Object.entries(extra)) {
      if (v === null) q.delete(k);
      else q.set(k, v);
    }
    setParams(q, { replace: false });
  };

  const setView = (map: boolean) => {
    if (map === isMap) return;
    const q = new URLSearchParams();
    if (map) {
      q.set("isMap", "true");
      if (filters.status === "rent") q.set("onlyOnRent", "true");
      else q.set("onlyOnSale", "true");
    } else if (filters.status !== "all") {
      q.set(filters.status === "rent" ? "onlyOnRent" : "onlyOnSale", "true");
    }
    setParams(q, { replace: false });
  };

  const setMapLayer = (key: "onlyOnSale" | "onlyOnRent" | "owned", on: boolean) => {
    const q = new URLSearchParams(params);
    const sale = key === "onlyOnSale" ? on : mapSale;
    const rent = key === "onlyOnRent" ? on : mapRent;
    if (key === "owned") {
      if (on) q.delete("owned");
      else q.set("owned", "false");
    }
    q.set("onlyOnSale", String(sale));
    if (rent) q.set("onlyOnRent", "true");
    else q.delete("onlyOnRent");
    setParams(q, { replace: false });
  };

  const tiles = useResource<TilesResult>(isMap ? (signal) => (loadTiles ? loadTiles(signal) : fetchTiles({ signal })) : null, [isMap]);
  const owned = useResource(account && isMap ? (signal) => fetchOwnedLand(account, { client, signal }) : null, [account, client, isMap]);
  const listKey = landsListKey(filters, showOwned ? account : null);
  const listing = useResource(
    !isMap ? (signal) => fetchLandNfts(landsListingFilters(filters, showOwned ? account : null), { client, signal }) : null,
    [isMap, listKey, client],
    isMap ? undefined : peekShopQuery<Awaited<ReturnType<typeof fetchLandNfts>>>(landsListingKey(listKey)),
  );
  const [more, setMore] = useState<{ key: string; pages: NftResult[][]; loading: boolean; failed: boolean }>({ key: "", pages: [], loading: false, failed: false });
  const moreNow = more.key === listKey ? more : { key: listKey, pages: [] as NftResult[][], loading: false, failed: false };
  const loadMoreRef = useRef<AbortController | null>(null);
  useEffect(() => () => loadMoreRef.current?.abort(), [listKey]);
  const loadMore = useCallback(() => {
    if (moreNow.loading) return;
    const page = moreNow.pages.length + 2;
    const ctrl = new AbortController();
    loadMoreRef.current = ctrl;
    setMore({ ...moreNow, loading: true, failed: false });
    fetchLandNfts(landsListingFilters(filters, showOwned ? account : null, page), { client, signal: ctrl.signal }).then(
      (r) => {
        if (!ctrl.signal.aborted) setMore((m) => (m.key === listKey ? { key: listKey, pages: [...m.pages, r.results], loading: false, failed: false } : m));
      },
      () => {
        if (!ctrl.signal.aborted) setMore((m) => (m.key === listKey ? { ...m, loading: false, failed: true } : m));
      },
    );
  }, [moreNow, filters, showOwned, account, client, listKey]);

  const ownedSets = useMemo(() => {
    const mine = new Set<string>();
    const rented = new Set<string>();
    for (const r of owned.data ?? []) {
      for (const p of landParcels(r)) {
        mine.add(`${p.x},${p.y}`);
        if (r.rental?.status === "executed") rented.add(`${p.x},${p.y}`);
      }
    }
    return { mine, rented };
  }, [owned.data]);

  const center = useMemo(() => {
    const t = tiles.data?.tiles;
    if (!t) return undefined;
    const wanted = Object.values(t).filter((tile) =>
      showOwned && ownedSets.mine.size ? ownedSets.mine.has(tile.id) : mapSale ? tile.price !== null : mapRent ? tile.rentalPricePerDay !== null : false,
    );
    return nearestTile({ x: 0, y: 0 }, wanted) ?? { x: 0, y: 0 };
  }, [tiles.data, mapSale, mapRent, showOwned, ownedSets]);

  const outage = listing.status === "error" ? indexOutage(listing.error) : null;
  const total = listing.data?.total ?? 0;
  const results = useMemo(() => [...(listing.data?.results ?? []), ...moreNow.pages.flat()], [listing.data, moreNow.pages]);
  const lastPage = moreNow.pages.at(-1);
  const hasMore =
    !!listing.data &&
    listing.data.results.length === LAND_PAGE_SIZE &&
    results.length < total &&
    (lastPage === undefined || lastPage.length === LAND_PAGE_SIZE) &&
    moreNow.pages.length + 1 < MAX_PAGE &&
    !moreNow.failed;

  if (isMap) {
    return (
      <div className="shop-lb shop-lb--map">
        <h1 className="shop-lb__sr">LAND</h1>
        <section className="shop-lb-map" aria-label="LAND map">
          {tiles.status === "error" ? (
            <div className="shop-land-note shop-land-note--error" role="alert">
              <p>{tiles.error instanceof MapNotReadyError ? tiles.error.message : "The LAND map could not be loaded."}</p>
              <Button size="sm" variant="secondary" onClick={tiles.reload}>
                Try again
              </Button>
              <Button size="sm" variant="ghost" onClick={() => setView(false)}>
                Show the list instead
              </Button>
            </div>
          ) : (
            <LandMap
              className="shop-lb-map__atlas"
              tiles={tiles.data?.tiles ?? null}
              layers={{ onSale: mapSale, forRent: mapRent, owned: showOwned ? ownedSets.mine : null, rented: ownedSets.rented }}
              center={center}
              onOpen={(href) => navigate(href)}
              withLegend
              label="LAND map: drag to move, use the zoom buttons, select a parcel to open it"
            />
          )}
          <div className="shop-lb-maptop">
            <div className="shop-lb-maptop__filters">
              <label className="shop-lb-square">
                <input type="checkbox" checked={mapSale} onChange={(e) => setMapLayer("onlyOnSale", e.target.checked)} /> Available to buy
              </label>
              <label className="shop-lb-square">
                <input type="checkbox" checked={mapRent} onChange={(e) => setMapLayer("onlyOnRent", e.target.checked)} /> Available to rent
              </label>
              {account ? (
                <label className="shop-lb-square">
                  <input type="checkbox" checked={showOwned} onChange={(e) => setMapLayer("owned", e.target.checked)} /> Owned by me
                </label>
              ) : null}
            </div>
            <ViewToggle isMap onChange={setView} />
          </div>
          {tiles.status === "loading" ? (
            <p className="shop-lb-map__loading" role="status" aria-live="polite">
              Loading the map…
            </p>
          ) : null}
        </section>
        <LandWalletNotice />
      </div>
    );
  }

  return (
    <div className="shop-lb">
      <h1 className="shop-lb__sr">LAND</h1>
      <div className="shop-lb__row">
        <div className={`shop-lb__sidebar${filtersOpen ? " is-open" : ""}`}>
          <div className="shop-lb__sheethead">
            <span>Filters</span>
            <button type="button" onClick={() => setFiltersOpen(false)}>
              Done
            </button>
          </div>
          <nav className="shop-lb-cats" aria-label="Categories">
            <p className="shop-lb-cats__title">Categories</p>
            <ul>
              {CATEGORIES.map(([value, label]) => (
                <li key={value}>
                  <button
                    type="button"
                    className={`shop-lb-cats__item${value === "land" ? "" : " shop-lb-cats__item--sub"}`}
                    aria-pressed={filters.category === value}
                    onClick={() => update({ category: value })}
                  >
                    {label}
                  </button>
                </li>
              ))}
            </ul>
          </nav>
          <LandFiltersPanel filters={filters} onChange={update} client={client} />
          {wide ? <LandWalletNotice /> : null}
        </div>
        <section className="shop-lb__main" aria-label="LAND results" aria-busy={listing.status === "loading"}>
          <div className="shop-lb-topbar">
            <form
              role="search"
              className="shop-lb-search"
              onSubmit={(e) => {
                e.preventDefault();
                update({ search: searchDraft.trim() });
              }}
            >
              <SearchGlyph />
              <input type="search" aria-label="Search LAND" placeholder="Search" value={searchDraft} onChange={(e) => setSearchDraft(e.target.value)} />
            </form>
            <ViewToggle isMap={false} onChange={setView} />
          </div>
          <div className="shop-lb-info">
            <p className="shop-lb-info__count" role="status">
              {listing.data ? `${total.toLocaleString("en-US")} ${total === 1 ? "item" : "items"}` : " "}
            </p>
            <div className="shop-lb-info__right">
              <select className="shop-lb-sort" aria-label="Sort by" value={filters.sort} onChange={(e) => update({ sort: e.target.value as LandFilters["sort"] })}>
                {landSortsFor(filters.status).map((s) => (
                  <option key={s} value={s}>
                    {LAND_SORT_LABEL[s]}
                  </option>
                ))}
              </select>
              <button type="button" className="shop-lb-filterbtn" aria-label="Filters" aria-expanded={filtersOpen} onClick={() => setFiltersOpen((o) => !o)}>
                <FiltersGlyph />
              </button>
            </div>
          </div>
          {wide ? null : <LandWalletNotice />}
          {listing.status === "loading" && !listing.data ? (
            <ul className="shop-lb-grid" aria-hidden="true">
              {Array.from({ length: 12 }, (_, i) => (
                <li key={i} className="shop-lb-skeleton" />
              ))}
            </ul>
          ) : outage ? (
            <div className="shop-land-note shop-land-note--warn" role="status">
              <p>LAND listings cannot be shown right now: {outage.message} This is a delay in our index, not an empty market.</p>
              <Button size="sm" variant="secondary" onClick={listing.reload}>
                Try again
              </Button>
            </div>
          ) : listing.status === "error" ? (
            <div className="shop-land-note shop-land-note--error" role="alert">
              <p>The LAND listings could not be loaded.</p>
              <Button size="sm" variant="secondary" onClick={listing.reload}>
                Try again
              </Button>
            </div>
          ) : listing.data && listing.data.results.length === 0 ? (
            <EmptyState
              title="No LAND matches these filters"
              subtitle={filters.status === "rent" ? "Nothing is listed for rent with these filters." : "Try other filters or look at all land."}
              actions={[{ label: "Clear filters", onClick: () => setParams(new URLSearchParams(), { replace: false }) }]}
            />
          ) : listing.data ? (
            <>
              <ul className="shop-lb-grid" role="feed" aria-busy={moreNow.loading}>
                {results.map((r, i) => (
                  <li key={`${r.nft.id}-${i}`}>
                    <LandCard result={r} position={i} />
                  </li>
                ))}
                {hasMore || moreNow.loading
                  ? Array.from({ length: 4 }, (_, i) => <li key={`skeleton-${i}`} className="shop-lb-skeleton" aria-hidden="true" />)
                  : null}
              </ul>
              {hasMore ? <Sentinel active={!moreNow.loading} onVisible={loadMore} /> : null}
              {moreNow.failed ? (
                <div className="shop-land-note shop-land-note--error shop-lb-more" role="alert">
                  <p>More LAND could not be loaded.</p>
                  <Button size="sm" variant="secondary" onClick={() => setMore({ ...moreNow, failed: false })}>
                    Try again
                  </Button>
                </div>
              ) : null}
            </>
          ) : null}
        </section>
      </div>
    </div>
  );
}

export function LandFiltersPanel({ filters, onChange, client }: { filters: LandFilters; onChange(patch: Partial<LandFilters>): void; client?: ShopClient }) {
  const [draft, setDraft] = useState(filters);
  const key = landFiltersToSearch(filters);
  useEffect(() => setDraft(filters), [key]); // eslint-disable-line react-hooks/exhaustive-deps
  const commit = (patch: Partial<LandFilters>) => {
    setDraft((d) => ({ ...d, ...patch }));
    onChange(patch);
  };
  type RangeKey = "minPrice" | "maxPrice" | "minEstateSize" | "maxEstateSize" | "minDistanceToPlaza" | "maxDistanceToPlaza";
  const field = (key: RangeKey, label: string, placeholder: string, mana = false) => (
    <span className={`shop-lb-input${mana ? " shop-lb-input--mana" : ""}`}>
      {mana ? <ManaMark size={14} network="ethereum" /> : null}
      <input
        type="number"
        min={0}
        inputMode="decimal"
        aria-label={label}
        placeholder={placeholder}
        value={draft[key]}
        onChange={(e) => setDraft((d) => ({ ...d, [key]: e.target.value }))}
        onBlur={() => draft[key] !== filters[key] && commit({ [key]: draft[key] })}
        onKeyDown={(e) => e.key === "Enter" && commit({ [key]: draft[key] })}
      />
    </span>
  );
  const nearPlaza = !!(filters.minDistanceToPlaza || filters.maxDistanceToPlaza);
  return (
    <aside className="shop-lb-filters" aria-label="Filters">
      <Box title="Status">
        <div className="shop-lb-radios shop-lb-radios--status" role="radiogroup" aria-label="Status">
          {(["all", "sale", "rent"] as const).map((s) => (
            <label key={s} className="shop-lb-radio">
              <input type="radio" name="shop-land-status" checked={filters.status === s} onChange={() => commit({ status: s })} />
              <span>{STATUS_LABEL[s]}</span>
            </label>
          ))}
        </div>
      </Box>
      <Box title={filters.status === "rent" ? "Price per day" : "Price"}>
        <LandPriceChart
          filters={filters}
          client={client}
          min={filters.minPrice}
          max={filters.maxPrice}
          onChange={(minPrice, maxPrice) => (minPrice !== filters.minPrice || maxPrice !== filters.maxPrice) && commit({ minPrice, maxPrice })}
          inputs={
            <div className="shop-lb-range">
              <label>
                <span className="shop-lb-range__label">Min</span>
                {field("minPrice", "Min price in MANA", "0", true)}
              </label>
              <label>
                <span className="shop-lb-range__label">Max</span>
                {field("maxPrice", "Max price in MANA", "1000", true)}
              </label>
            </div>
          }
        />
      </Box>
      {filters.category !== "parcel" ? (
        <Box title="Estate size">
          <div className="shop-lb-range shop-lb-range--plain">
            <label>
              <span className="shop-lb-range__label shop-lb-range__label--case">Min (Parcels)</span>
              {field("minEstateSize", "Min estate size", "0")}
            </label>
            <label>
              <span className="shop-lb-range__label shop-lb-range__label--case">Max (Parcels)</span>
              {field("maxEstateSize", "Max estate size", "1000")}
            </label>
          </div>
        </Box>
      ) : null}
      {filters.status === "rent" ? (
        <Box title="Rental period">
          <div className="shop-lb-radios">
            {PERIOD_OPTIONS.map((p) => {
              const days = daysByPeriod[p];
              const on = filters.rentalDays.includes(days);
              return (
                <label key={p} className="shop-lb-square">
                  <input
                    type="checkbox"
                    checked={on}
                    onChange={() => commit({ rentalDays: on ? filters.rentalDays.filter((d) => d !== days) : [...filters.rentalDays, days].sort((a, b) => a - b) })}
                  />{" "}
                  {PERIOD_LABEL[p]}
                </label>
              );
            })}
          </div>
        </Box>
      ) : null}
      <Box title="Location">
        <div className="shop-lb-radios">
          <label className="shop-lb-switch">
            <input type="checkbox" checked={filters.adjacentToRoad} onChange={(e) => commit({ adjacentToRoad: e.target.checked })} />
            <span>Adjacent to road</span>
          </label>
          <label className="shop-lb-switch">
            <input
              type="checkbox"
              checked={nearPlaza}
              onChange={(e) => commit(e.target.checked ? { minDistanceToPlaza: PLAZA_MIN, maxDistanceToPlaza: PLAZA_MAX } : { minDistanceToPlaza: "", maxDistanceToPlaza: "" })}
            />
            <span>Near a plaza</span>
          </label>
          {nearPlaza ? (
            <div className="shop-lb-range shop-lb-range--plain">
              <label>
                <span className="shop-lb-range__label shop-lb-range__label--case">Min distance</span>
                {field("minDistanceToPlaza", "Min distance to plaza", PLAZA_MIN)}
              </label>
              <label>
                <span className="shop-lb-range__label shop-lb-range__label--case">Max distance</span>
                {field("maxDistanceToPlaza", "Max distance to plaza", PLAZA_MAX)}
              </label>
            </div>
          ) : null}
        </div>
      </Box>
    </aside>
  );
}
