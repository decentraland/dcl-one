import { queryOptions } from "@tanstack/react-query";

import { itemIdFromUrn } from "@shop/detail/lib/urn";
import { CHAIN } from "@shop/model/chain";

import { assetPage } from "@/core/lib/shop/redirects";
import { fetchCatalog, type CatalogItem } from "../catalyst/marketplace";
import { loadWorldPlace, resolveLegacyPlace } from "../catalyst/places/destinations";

const STALE_MS = 60_000;
const MAX_ITEMS = 100;
const NETWORKS = {
  ethereum: { network: "ETHEREUM", chainId: CHAIN.ETHEREUM },
  matic: { network: "MATIC", chainId: CHAIN.POLYGON },
} as const;

type Network = (typeof NETWORKS)[keyof typeof NETWORKS];
export type ReelWearable = { id: string; label: string; image?: string; href?: string };

export function wearableRef(urn: string): { key: string; id: string; chain: Network } | undefined {
  const match = /^urn:decentraland:(ethereum|mainnet|matic):collections-v2:(0x[0-9a-f]{40}):(\d{1,78})(?::\d{1,78})?$/i.exec(
    urn,
  );
  if (!match) return undefined;
  const chain = match[1]!.toLowerCase() === "matic" ? NETWORKS.matic : NETWORKS.ethereum;
  const id = itemIdFromUrn(`collections-v2:${match[2]}:${match[3]}`);
  if (!id) return undefined;
  return { key: `${chain.chainId}:${id}`, id, chain };
}

export function reelWearables(urns: readonly string[], items: Record<string, ReelWearable>): ReelWearable[] {
  const byId = new Map<string, ReelWearable>();
  let unresolved = false;
  for (const urn of urns) {
    const key = wearableRef(urn)?.key;
    const item = key ? items[key] : undefined;
    if (item) byId.set(item.id, item);
    else if (key) unresolved = true;
  }
  if (unresolved) byId.set("unavailable", { id: "unavailable", label: "Some collectible details are unavailable." });
  return [...byId.values()];
}

function wearableItem(item: CatalogItem, wanted: ReadonlySet<string>, chain: Network): ReelWearable | undefined {
  const id = `${chain.chainId}:${item.id}`;
  if (item.chainId !== chain.chainId || item.category !== "wearable" || !item.name || !wanted.has(id)) return;
  const ref = wearableRef(
    `urn:decentraland:${chain.network === "MATIC" ? "matic" : "ethereum"}:collections-v2:${item.contractAddress}:${item.itemId}`,
  );
  if (ref?.key !== id) return;
  return {
    id,
    label: item.name,
    image: item.thumbnail ?? undefined,
    href: assetPage(item.contractAddress, item.itemId, "item"),
  };
}

export function reelWearablesQuery(urns: readonly string[]) {
  const references = new Map<string, NonNullable<ReturnType<typeof wearableRef>>>();
  for (const urn of urns) {
    const ref = wearableRef(urn);
    if (ref && references.size < MAX_ITEMS) references.set(ref.key, ref);
  }
  const keys = [...references.keys()].sort();
  return queryOptions({
    queryKey: ["reels", "wearables", keys] as const,
    staleTime: STALE_MS,
    enabled: keys.length > 0,
    queryFn: async ({ signal }): Promise<Record<string, ReelWearable>> => {
      const groups = await Promise.allSettled(
        Object.values(NETWORKS).map(async (chain) => {
          const ids = keys.flatMap((key) => {
            const ref = references.get(key);
            return ref?.chain === chain ? [ref.id] : [];
          });
          if (!ids.length) return [];
          const wanted = new Set(ids.map((id) => `${chain.chainId}:${id}`));
          const { data } = await fetchCatalog({ ids, first: ids.length, network: chain.network }, { signal });
          return data.flatMap((item) => wearableItem(item, wanted, chain) ?? []);
        }),
      );
      if (signal.aborted) throw signal.reason;
      if (groups.every((group) => group.status === "rejected" || group.value.length === 0)) {
        const failed = groups.find((group) => group.status === "rejected");
        if (failed) throw failed.reason;
      }
      return Object.fromEntries(
        groups.flatMap((group) => (group.status === "fulfilled" ? group.value : [])).map((item) => [item.id, item]),
      );
    },
  });
}

export function reelPlaceQuery(position?: string, realm?: string) {
  return queryOptions({
    queryKey: ["reels", "place", position, realm] as const,
    staleTime: STALE_MS,
    enabled: Boolean(position || realm),
    queryFn: async ({ signal }) => {
      if (realm) {
        const world = await loadWorldPlace(realm, { signal });
        return world ? `/places/world/${encodeURIComponent(realm)}` : null;
      }
      const place = position ? await resolveLegacyPlace(position, { signal }) : null;
      return place ? `/places/${encodeURIComponent(place)}` : null;
    },
  });
}
