// The browser host's options for the Shop's reads (discovery/api, detail/api): the document's client and the
// creator-sales switch as this page holds it. The fetchers take them as arguments and read neither themselves.

import { shopClient } from "../api/host";
import { createShopClient } from "@shop/api/shop";
import type { ShopApi } from "@shop/api/client";
import type { Opts } from "@shop/discovery/api";
import { shopFlags } from "./flags";

export function shopReads(signal?: AbortSignal): Opts {
  return { client: shopClient(), creatorSales: () => shopFlags().creatorSales, signal };
}

/**
 * shopReads for reads sent before this page's flags are known (a client navigation's loader): each request leaves at
 * once, and its answer is mapped once `flags` has settled, so with the creator-sales switch the page holds.
 */
export function shopReadsAfter(flags: Promise<unknown>): (signal: AbortSignal) => Opts {
  const api = shopClient().api;
  const settled = flags.catch(() => {});
  const client = createShopClient({ ...api, request: async <T>(...args: Parameters<ShopApi["request"]>) => (await Promise.all([api.request<T>(...args), settled]))[0] });
  return (signal) => ({ ...shopReads(signal), client });
}
