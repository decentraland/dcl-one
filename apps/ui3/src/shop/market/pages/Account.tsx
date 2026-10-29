// from: marketplace@3bf3662f:webapp/src/components/AccountPage (the public /accounts/:address page: AccountBanner,
//       then AssetBrowse in the ACCOUNT view), webapp/src/components/AssetBrowse/AssetBrowse.tsx,
//       webapp/src/components/AssetTopbar/AssetTopbar.tsx (search, count, sort, selected-filter chips),
//       webapp/src/modules/routing/url-parser.ts (getSortByOptionsFromUrl, hasFiltersEnabled)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { useInfiniteQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { useParams, useSearchParams } from "react-router";

import { fetchCatalogItems } from "../../discovery/api";
import AssetCard from "../../discovery/components/AssetCard";
import type { CatalogItem } from "../../discovery/lib/catalogItem";
import { parseEvmAddress, sameAddress } from "../../model/address";
import { useShopWallet } from "../../wallet/context";
import { EnsureQueryClient, LoadFailure } from "../components/kit";
import AccountBanner from "./account/AccountBanner";
import AccountSidebar from "./account/AccountSidebar";
import NftCard from "./account/NftCard";
import { fetchAccountNfts, isWearableSection, parseSection, sortOptions, wearableCategoriesOf, type AccountNft, type AssetType, type Section } from "./account/api";
import "./account/account.css";

const PAGE_SIZE = 24;

type Browse = { assetType: AssetType; section: Section; onlyOnSale: boolean; sortBy: string; search: string; rarities: string[] };

export function readBrowse(params: URLSearchParams): Browse {
  const assetType: AssetType = params.get("assetType") === "item" ? "item" : "nft";
  const section = parseSection(params.get("section"), assetType);
  const onlyOnSale = params.get("onlyOnSale") === "true";
  const options = sortOptions(assetType, onlyOnSale);
  const raw = params.get("sortBy");
  const sortBy = options.some((o) => o.value === raw) ? raw! : assetType === "item" ? "newest" : options[0]!.value;
  const rarities = (params.get("rarities") ?? "").split("_").filter(Boolean);
  return { assetType, section, onlyOnSale, sortBy, search: params.get("search") ?? "", rarities };
}

type Page = { kind: "nft"; items: AccountNft[]; total: number } | { kind: "item"; items: CatalogItem[]; total: number };

function useAccountBrowse(address: string, b: Browse) {
  return useInfiniteQuery({
    queryKey: ["account-browse", address.toLowerCase(), b],
    initialPageParam: 0,
    queryFn: async ({ pageParam }): Promise<Page> => {
      if (b.assetType === "item") {
        const res = await fetchCatalogItems({
          creator: address.toLowerCase(),
          first: PAGE_SIZE,
          skip: pageParam,
          category: b.section === "emotes" ? "emote" : "wearable",
          wearableCategories: wearableCategoriesOf(b.section),
          rarities: b.rarities.length ? b.rarities : undefined,
          search: b.search || undefined,
          sortBy: b.sortBy,
        });
        return { kind: "item", items: res.items, total: res.total };
      }
      const res = await fetchAccountNfts(address, {
        section: b.section,
        first: PAGE_SIZE,
        skip: pageParam,
        search: b.search || undefined,
        sortBy: b.sortBy,
        onlyOnSale: b.onlyOnSale,
        rarities: isWearableSection(b.section) || b.section === "emotes" ? b.rarities : undefined,
      });
      return { kind: "nft", items: res.assets, total: res.total };
    },
    getNextPageParam: (last, pages) => {
      const loaded = pages.reduce((n, p) => n + p.items.length, 0);
      return loaded < last.total && last.items.length > 0 ? loaded : undefined;
    },
  });
}

function SearchBox({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  useEffect(() => {
    if (text.trim() === value) return;
    const id = setTimeout(() => onChange(text.trim()), 400);
    return () => clearTimeout(id);
  }, [text, value, onChange]);
  return (
    <label className="acct-search">
      <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
        <circle cx="6.8" cy="6.8" r="4.8" fill="none" stroke="currentColor" strokeWidth="2" />
        <path d="M10.5 10.5L14 14" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
      </svg>
      <input type="search" value={text} placeholder="Search" aria-label="Search" onChange={(e) => setText(e.target.value)} />
    </label>
  );
}

function Skeletons({ count }: { count: number }) {
  return (
    <>
      {Array.from({ length: count }, (_, i) => (
        <div key={i} className="acct-skeleton" aria-hidden="true" />
      ))}
    </>
  );
}

export function AccountView({ address }: { address: string }) {
  const { account } = useShopWallet();
  const own = sameAddress(address, account);
  const [params, setParams] = useSearchParams();
  const b = useMemo(() => readBrowse(params), [params]);
  const [railOpen, setRailOpen] = useState(false);
  const query = useAccountBrowse(address, b);
  const sentinel = useRef<HTMLDivElement>(null);

  const update = (patch: Record<string, string | null>) =>
    setParams(
      (prev) => {
        const p = new URLSearchParams(prev);
        for (const [k, v] of Object.entries(patch)) {
          if (v == null || v === "") p.delete(k);
          else p.set(k, v);
        }
        return p;
      },
      { replace: true, preventScrollReset: true },
    );
  const updateRef = useRef(update);
  updateRef.current = update;
  const onSearch = useMemo(() => (v: string) => updateRef.current({ search: v }), []);

  const { hasNextPage, isFetchingNextPage, fetchNextPage } = query;
  useEffect(() => {
    const el = sentinel.current;
    if (!el || !hasNextPage || typeof IntersectionObserver === "undefined") return;
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting) && !isFetchingNextPage) void fetchNextPage();
      },
      { rootMargin: "600px" },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);

  const pages = query.data?.pages ?? [];
  const total = pages[0]?.total;
  const nfts = pages.flatMap((p) => (p.kind === "nft" ? p.items : []));
  const items = pages.flatMap((p) => (p.kind === "item" ? p.items : []));
  const count = nfts.length + items.length;
  const options = sortOptions(b.assetType, b.onlyOnSale);
  const includesNotOnSale = b.assetType === "nft" && b.section !== "on_rent" && !b.onlyOnSale;
  const hasFilters = includesNotOnSale || b.rarities.length > 0;

  return (
    <div className="acct" data-testid="shop-account">
      {own ? <h1 className="acct-vh">Your account</h1> : <AccountBanner address={address} />}
      <div className="acct-browse" data-rail-open={railOpen || undefined}>
        <AccountSidebar
          assetType={b.assetType}
          section={b.section}
          rarities={b.rarities}
          onBrowse={(next) => {
            setRailOpen(false);
            update({ assetType: next.assetType ?? b.assetType, section: next.section, rarities: null, sortBy: null, ...(next.section === "on_rent" ? { onlyOnSale: null } : {}) });
          }}
          onRarities={(r) => update({ rarities: r.join("_") })}
        />
        <div className="acct-main">
          <SearchBox value={b.search} onChange={onSearch} />
          <div className="acct-topbar">
            <span className="acct-count" data-testid="shop-account-count">
              {total === undefined ? "" : b.search ? (total > 0 ? `${total.toLocaleString("en-US")} results for "${b.search}"` : "No results") : `${total.toLocaleString("en-US")} items`}
            </span>
            <div className="acct-topbar__right">
              <label className="acct-sort">
                <span className="acct-vh">Sort by</span>
                <select value={b.sortBy} onChange={(e) => update({ sortBy: e.target.value })}>
                  {options.map((o) => (
                    <option key={o.value} value={o.value}>
                      {o.label}
                    </option>
                  ))}
                </select>
              </label>
              <button type="button" className="acct-filterbtn" aria-label="Filters" aria-expanded={railOpen} onClick={() => setRailOpen((v) => !v)}>
                <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
                  <path d="M2 4h12M4 8h8M6 12h4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
                </svg>
              </button>
            </div>
          </div>
          {hasFilters ? (
            <div className="acct-chips">
              {includesNotOnSale ? (
                <button type="button" className="acct-chip" onClick={() => update({ onlyOnSale: "true", sortBy: null })}>
                  Includes not on sale <span aria-hidden="true">✖</span>
                </button>
              ) : null}
              {b.rarities.map((r) => (
                <button key={r} type="button" className="acct-chip" onClick={() => update({ rarities: b.rarities.filter((x) => x !== r).join("_") })}>
                  {r} <span aria-hidden="true">✖</span>
                </button>
              ))}
              <button type="button" className="acct-clear" onClick={() => update({ rarities: null, onlyOnSale: "true", search: null, sortBy: null })}>
                <svg viewBox="0 0 16 16" width="18" height="18" aria-hidden="true">
                  <path d="M3 4h10M6.5 4V2.5h3V4M4.5 4l.7 9.5h5.6l.7-9.5M7 6.5v5M9 6.5v5" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round" />
                </svg>
                Clear all filters
              </button>
            </div>
          ) : b.assetType === "nft" && b.onlyOnSale && b.section !== "on_rent" ? (
            <div className="acct-chips">
              <button type="button" className="acct-chip acct-chip--ghost" onClick={() => update({ onlyOnSale: null, sortBy: null })}>
                Include not on sale
              </button>
            </div>
          ) : null}
          {query.isError ? (
            <LoadFailure error={query.error} what={b.assetType === "item" ? "Their creations" : "Their assets"} onRetry={() => void query.refetch()} />
          ) : !query.isPending && count === 0 ? (
            <div className="acct-empty" data-testid="shop-empty">
              <p>{b.search || hasFilters ? "No results found." : "No items found."}</p>
            </div>
          ) : (
            <div className="acct-grid" data-kind={b.assetType}>
              {nfts.map((a) => (
                <NftCard key={a.id} asset={a} />
              ))}
              {items.map((item, i) => (
                <AssetCard key={item.id} item={item} source="creator" position={i} />
              ))}
              {query.isPending || isFetchingNextPage ? <Skeletons count={query.isPending ? 12 : 8} /> : null}
            </div>
          )}
          {hasNextPage ? <div ref={sentinel} className="acct-sentinel" /> : null}
        </div>
      </div>
    </div>
  );
}

export default function Account() {
  const { address } = useParams();
  const parsed = parseEvmAddress(address ?? "");
  return (
    <EnsureQueryClient>
      {parsed ? (
        <AccountView address={parsed} />
      ) : (
        <div className="acct acct-invalid">
          <p role="alert">This link does not name an account.</p>
        </div>
      )}
    </EnsureQueryClient>
  );
}
