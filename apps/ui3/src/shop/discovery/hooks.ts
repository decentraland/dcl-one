// from: shop@94584a5c:app/src/hooks/useUrlFilters.ts, shop@2d3bf62:app/src/components/CreatorName/CreatorName.tsx, app/src/lib/avatarColor.ts (getDisplayName), app/src/hooks/useScrollTopOnChange.ts, app/src/hooks/useSaleActive.ts,
//       app/src/hooks/useProfile.ts, app/src/hooks/useSeo.ts, app/src/hooks/useCampaign.ts, app/src/hooks/useCampaignContracts.ts

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router";

import { fetchCommunityProfiles } from "../../data/hooks/useCommunityProfiles";
import { shopClient } from "../api/shop";
import { readMirrorFreshness, type IndexFreshness } from "./freshness";
import { fetchCampaign, type Campaign } from "./lib/contentful";
import { fetchCampaignContracts } from "./lib/campaign";
import { isSaleActive, saleTimeLeft, type SaleInfo } from "./lib/sale";
import { useShopQuery } from "./query";

export type FilterValue = string | string[] | boolean | null;

function encode(value: FilterValue | undefined): string | null {
  if (value === null || value === undefined || value === false || value === "") return null;
  if (Array.isArray(value)) return value.length ? value.join(",") : null;
  if (value === true) return "true";
  return String(value);
}

function decode(raw: string | null, fallback: FilterValue): FilterValue {
  if (raw === null) return fallback;
  if (Array.isArray(fallback)) return raw ? raw.split(",").filter(Boolean) : [];
  if (typeof fallback === "boolean") return raw === "true";
  return raw;
}

export function readUrlFilters<T extends Record<string, FilterValue>>(searchParams: URLSearchParams, defaults: T): T {
  const out = { ...defaults };
  for (const key of Object.keys(defaults) as (keyof T)[]) out[key] = decode(searchParams.get(String(key)), defaults[key] as FilterValue) as T[keyof T];
  return out;
}

export function useUrlFilters<T extends Record<string, FilterValue>>(defaults: T): [T, (patch: Partial<T>) => void] {
  const [searchParams, setSearchParams] = useSearchParams();
  const pending = useRef({ observed: searchParams, value: searchParams });
  if (pending.current.observed !== searchParams) pending.current = { observed: searchParams, value: searchParams };
  const values = useMemo(() => readUrlFilters(searchParams, defaults), [searchParams, defaults]);
  const setFilters = useCallback(
    (patch: Partial<T>) => {
      const next = new URLSearchParams(pending.current.value);
      for (const [key, value] of Object.entries(patch) as [string, FilterValue][]) {
        const encoded = encode(value);
        if (encoded === null || encoded === encode(defaults[key])) next.delete(key);
        else next.set(key, encoded);
      }
      pending.current.value = next;
      setSearchParams(next, { replace: true });
    },
    [setSearchParams, defaults],
  );
  return [values, setFilters];
}

export function useScrollTopOnChange(key: string): void {
  const first = useRef(true);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    if (typeof window !== "undefined" && typeof window.scrollTo === "function") {
      try {
        window.scrollTo({ top: 0 });
      } catch {
      }
    }
  }, [key]);
}

export function useSaleActive(sale: SaleInfo): boolean {
  const [now, setNow] = useState(() => Date.now());
  const left = saleTimeLeft(sale.saleEndsAt, now);
  useEffect(() => {
    if (!Number.isFinite(left) || left <= 0) return;
    const timer = setTimeout(() => setNow(Date.now()), Math.min(left + 50, 2_147_000_000));
    return () => clearTimeout(timer);
  }, [left]);
  return isSaleActive(sale, now);
}

export type CreatorProfile = { name: string; picture: string | null; claimed: boolean | null };

export function useProfiles(addresses: Array<string | null | undefined>): Record<string, CreatorProfile> {
  const wanted = useMemo(() => [...new Set(addresses.filter((a): a is string => !!a && /^0x[0-9a-fA-F]{40}$/.test(a)).map((a) => a.toLowerCase()))].sort(), [addresses]);
  const { data } = useShopQuery(wanted.length ? ["profiles", ...wanted] : null, (signal) => fetchCommunityProfiles(wanted, { signal }), {
    staleMs: 5 * 60_000,
  });
  return data ?? {};
}

export function useProfile(address: string | null | undefined): CreatorProfile | null {
  const profiles = useProfiles(useMemo(() => [address], [address]));
  return address ? (profiles[address.toLowerCase()] ?? null) : null;
}

const PROFILE_BATCH_MS = 10;
let profileQueue: Map<string, Array<{ resolve: (p: CreatorProfile | null) => void; reject: (e: unknown) => void }>> = new Map();
let profileFlush: ReturnType<typeof setTimeout> | null = null;

function batchedProfile(address: string): Promise<CreatorProfile | null> {
  return new Promise((resolve, reject) => {
    const waiters = profileQueue.get(address) ?? [];
    waiters.push({ resolve, reject });
    profileQueue.set(address, waiters);
    profileFlush ??= setTimeout(() => {
      profileFlush = null;
      const batch = profileQueue;
      profileQueue = new Map();
      fetchCommunityProfiles([...batch.keys()]).then(
        (profiles) => {
          for (const [addr, ws] of batch) for (const w of ws) w.resolve(profiles[addr] ?? null);
        },
        (error) => {
          for (const ws of batch.values()) for (const w of ws) w.reject(error);
        },
      );
    }, PROFILE_BATCH_MS);
  });
}

export function useCreatorProfile(address: string | null | undefined): { profile: CreatorProfile | null; loading: boolean } {
  const addr = address && /^0x[0-9a-fA-F]{40}$/.test(address) ? address.toLowerCase() : null;
  const { data, isLoading } = useShopQuery(addr ? ["creator-name", addr] : null, () => batchedProfile(addr!), { staleMs: 5 * 60_000 });
  return { profile: data ?? null, loading: !!addr && isLoading && data === undefined };
}

export function useCreatorName(address: string | null | undefined): string | undefined {
  const addr = address && /^0x[0-9a-fA-F]{40}$/.test(address) ? address.toLowerCase() : null;
  const { data, isLoading } = useShopQuery(addr ? ["creator-name", addr] : null, () => batchedProfile(addr!), { staleMs: 5 * 60_000 });
  if (!address) return undefined;
  if (!addr) return address;
  if (isLoading && data === undefined) return undefined;
  const name = data?.name?.trim();
  if (!name) return `${addr.slice(0, 6)}…${addr.slice(-4)}`;
  const shown = name.charAt(0).toUpperCase() + name.slice(1);
  return shown;
}

const SITE_NAME = "Decentraland Shop";
export const SHOP_DEFAULT_TITLE = `${SITE_NAME} | Wearables & Emotes for Your Avatar`;

export function shopDocumentTitle(title?: string | null): string {
  return title ? `${title} | ${SITE_NAME}` : SHOP_DEFAULT_TITLE;
}

function setMeta(attr: "name" | "property", key: string, value: string | null): () => void {
  let el = document.head.querySelector<HTMLMetaElement>(`meta[${attr}="${key}"]`);
  const created = !el;
  const previous = el?.getAttribute("content") ?? null;
  if (value === null) {
    if (!el) return () => {};
    el.remove();
    return () => { if (el && previous !== null) document.head.appendChild(el); };
  }
  if (!el) {
    el = document.createElement("meta");
    el.setAttribute(attr, key);
    document.head.appendChild(el);
  }
  el.setAttribute("content", value);
  return () => {
    if (created) el!.remove();
    else if (previous !== null) el!.setAttribute("content", previous);
  };
}

export function useSeo({ title, description, noindex, image }: { title?: string | null; description?: string | null; noindex?: boolean; image?: string | null }): void {
  useEffect(() => {
    if (typeof document === "undefined" || !title) return;
    const previous = document.title;
    const full = shopDocumentTitle(title);
    document.title = full;
    const undo = [setMeta("property", "og:title", full)];
    if (description) undo.push(setMeta("name", "description", description), setMeta("property", "og:description", description));
    if (noindex) undo.push(setMeta("name", "robots", "noindex,nofollow"));
    if (image) undo.push(setMeta("property", "og:image", image));
    return () => {
      document.title = previous;
      for (const u of undo.reverse()) u();
    };
  }, [title, description, noindex, image]);
}

export function useCampaign(): { campaign: Campaign | null; isPending: boolean; error: unknown } {
  const { data, isLoading, error } = useShopQuery(["campaign"], (signal) => fetchCampaign(signal), { staleMs: 5 * 60_000 });
  return { campaign: data ?? null, isPending: isLoading, error };
}

export function useCampaignContracts(tags: string[], named: string[] = []): { contracts: string[] | undefined; isPending: boolean; error: unknown } {
  const key = tags.length ? ["campaign-contracts", ...tags] : null;
  const { data, isLoading, error } = useShopQuery(key, (signal) => fetchCampaignContracts(tags, signal), { staleMs: 5 * 60_000 });
  const contracts = useMemo(() => {
    if (tags.length === 0) return named.length ? [...new Set(named)] : [];
    return data ? [...new Set([...data, ...named])] : undefined;
  }, [tags.length, data, named]);
  return { contracts, isPending: !!key && isLoading, error };
}

export function useListingsFreshness(enabled = true): IndexFreshness | null {
  const { data } = useShopQuery(enabled ? ["trades-mirror"] : null, (signal) => shopClient().market.tradesMirror(undefined, { signal }).then(readMirrorFreshness, () => null), {
    staleMs: 60_000,
  });
  return data ?? null;
}
