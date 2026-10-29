// from: shop@94584a5c:app/src/hooks/{useStore,useStoreStats,useCreatorSales,useImportable}.ts
// The seller pages' reads, as react-query hooks over the seller libraries. Builder reads are signed with
// the session's own identity (the host's), never another account's.

import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { identitySigner, type ShopSigner } from "../api/signer";
import { useShopWallet } from "../wallet/context";
import { fetchFavoriteStats } from "../market/lib/favorites";
import { fetchPublishableItems } from "./lib/builder";
import { fetchCollectionSaleState, type CollectionSaleState } from "./lib/collections";
import { fetchCreatorSales } from "./lib/coupons";
import { fetchImportable } from "./lib/import";
import { countSales, fetchSalesSummary, fetchSellerSales, weiOf } from "./lib/sales";
import { toSaleableCollections } from "./lib/saleableCollections";
import { collectorsOf, daysSinceLastSale, deltaOf, deltaOfWei, topBuyers } from "./lib/storeMetrics";
import { buildStoreStats, type StoreStats } from "./lib/storeStats";
import { fetchStore } from "./lib/store";

/** The Shop signer for the signed-in account's own identity; null when signed out or expired. */
export function useSellerSigner(): ShopSigner | null {
  const { host } = useShopWallet();
  return useMemo(() => identitySigner(host?.identity ?? null), [host?.identity]);
}

export function useStore(address?: string | null) {
  return useQuery({ queryKey: ["store", address?.toLowerCase()], enabled: !!address, staleTime: 5 * 60_000, queryFn: () => fetchStore(address!) });
}

export function useCreatorSales(address: string | null | undefined, enabled = true) {
  return useQuery({
    queryKey: ["creator-sales", address?.toLowerCase()],
    queryFn: () => fetchCreatorSales(address!),
    enabled: !!address && enabled,
    staleTime: 30_000,
    refetchInterval: 60_000,
    retry: false,
  });
}

export function useImportable(address: string | null | undefined) {
  return useQuery({ queryKey: ["importable", address], queryFn: () => fetchImportable(address!), enabled: !!address, staleTime: 5 * 60_000, retry: false });
}

export type StorePeriod = "7d" | "30d" | "all";
const DAY_MS = 86_400_000;
const WINDOW: Record<StorePeriod, number | null> = { "7d": 7, "30d": 30, all: null };
const EMPTY_SAVES = new Map<string, number>();

/** Upstream `useStoreStats`: the dashboard's figures for a period, and the same figures one period earlier. */
export function useStoreStats(address: string | null, period: StorePeriod) {
  const signer = useSellerSigner();
  const days = WINDOW[period];
  const now = useMemo(() => Math.floor(Date.now() / DAY_MS) * DAY_MS + DAY_MS - 1, []);
  const from = days ? now - days * DAY_MS : undefined;

  const sales = useQuery({ queryKey: ["store-sales", address, period], enabled: !!address, queryFn: () => fetchSellerSales({ seller: address!, from }) });
  const summary = useQuery({ queryKey: ["store-summary", address, period], enabled: !!address, retry: false, queryFn: () => fetchSalesSummary({ seller: address!, from }) });
  const previous = useQuery({
    queryKey: ["store-summary-previous", address, period],
    enabled: !!address && days != null,
    retry: false,
    queryFn: () => fetchSalesSummary({ seller: address!, from: now - 2 * days! * DAY_MS, to: now - days! * DAY_MS }),
  });
  const mints = useQuery({ queryKey: ["store-mints", address, period], enabled: !!address, queryFn: () => countSales({ seller: address!, from, type: "mint" }) });
  const catalogue = useQuery({
    queryKey: ["store-catalogue", address],
    enabled: !!address && !!signer,
    queryFn: () => fetchPublishableItems(address!, { signer: signer!, includeSoldOut: true }),
  });
  const addresses = useMemo(() => [...new Set((catalogue.data ?? []).map((item) => item.contractAddress))], [catalogue.data]);
  const saleState = useQuery({
    queryKey: ["store-sale-state", address, addresses],
    enabled: addresses.length > 0,
    queryFn: async () => {
      const settled = await Promise.allSettled(addresses.map((ca) => fetchCollectionSaleState(ca)));
      const states: Record<string, CollectionSaleState> = {};
      const unreadable = new Set<string>();
      settled.forEach((result, i) => {
        const ca = addresses[i]!;
        if (result.status === "rejected") {
          unreadable.add(ca.toLowerCase());
          return;
        }
        for (const [itemId, state] of Object.entries(result.value)) states[`${ca}-${itemId}`] = state;
      });
      return { states, unreadable };
    },
  });
  const saves = useQuery({
    queryKey: ["store-saves", addresses, (catalogue.data ?? []).length],
    enabled: (catalogue.data ?? []).length > 0,
    queryFn: async () => {
      const keys = (catalogue.data ?? []).map((item) => `${item.contractAddress.toLowerCase()}-${item.blockchainItemId}`);
      const settled = await Promise.allSettled(keys.map(async (key) => [key, (await fetchFavoriteStats(key)).count] as const));
      const counts = new Map<string, number>();
      for (const outcome of settled) if (outcome.status === "fulfilled") counts.set(outcome.value[0], outcome.value[1]);
      return counts;
    },
  });

  const stats = useMemo<StoreStats | null>(() => {
    if (!catalogue.data) return null;
    return buildStoreStats({
      rows: sales.data?.rows ?? [],
      total: sales.data?.total ?? 0,
      truncated: !!sales.data?.truncated,
      mints: mints.data ?? 0,
      summary: summary.data ?? null,
      catalogue: catalogue.data,
      saleState: saleState.data?.states ?? {},
      unreadable: saleState.data?.unreadable,
      days,
      now,
    });
  }, [catalogue.data, sales.data, mints.data, summary.data, saleState.data, days, now]);

  const saleable = useMemo(() => toSaleableCollections(catalogue.data ?? [], saleState.data?.states), [catalogue.data, saleState.data]);

  const trend = useMemo(() => {
    const rows = sales.data?.rows ?? [];
    const against = previous.data;
    return {
      collectors: collectorsOf(rows),
      buyers: topBuyers(rows),
      quietDays: daysSinceLastSale(rows, now),
      sold: against && summary.data ? deltaOf(summary.data.total, against.total) : null,
      earnings: against && summary.data ? deltaOfWei(weiOf(summary.data.earnedWei), weiOf(against.earnedWei)) : null,
    };
  }, [sales.data, summary.data, previous.data, now]);

  return {
    stats,
    trend,
    saves: saves.data ?? EMPTY_SAVES,
    saleable,
    catalogue: catalogue.data ?? null,
    saleState: saleState.data?.states ?? {},
    needsSigner: !signer,
    isLoading: catalogue.isLoading || sales.isLoading,
    error: catalogue.error ?? sales.error ?? null,
    refetch: () => {
      void catalogue.refetch();
      void sales.refetch();
      void summary.refetch();
      void saleState.refetch();
    },
  };
}
