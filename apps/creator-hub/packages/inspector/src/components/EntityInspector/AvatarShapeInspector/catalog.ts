import { fetchLatestCatalog, getBuiltInEmote, getContentsUrl } from '../../../lib/logic/catalog';
import { CATALYST_BASE_URL, MARKETPLACE_API_URL } from '../../../lib/logic/config';
import type { WearableCategory } from '../../../lib/logic/wearables';
import { isWearableCategory } from '../../../lib/logic/wearables';

export type WearableSource = 'scene' | 'base' | 'catalog';

export type WearableItem = {
  /** What goes in `AvatarShape.wearables`: a URN, or the path of a scene GLB. */
  id: string;
  name: string;
  category: WearableCategory;
  source: WearableSource;
  thumbnail?: string;
  rarity?: string;
  /**
   * A scene model rigged to the avatar whose file name gives no slot. Choosing it renames the file
   * to `name_category.glb`, the form explorers read the slot from.
   */
  needsSlotName?: boolean;
  /** A rigged model whose meshes suggest no slot: it is offered for every slot. */
  fitsAnySlot?: boolean;
};

export type AnimationSource = 'builtin' | 'scene' | 'catalog';

export type AnimationItem = {
  /** What goes in `AvatarShape.expressionTriggerId`: a built-in name, a scene path or a URN. */
  id: string;
  name: string;
  source: AnimationSource;
  thumbnail?: string;
  rarity?: string;
};

const BASE_AVATARS_COLLECTION = 'urn:decentraland:off-chain:base-avatars';
const REQUEST_TIMEOUT_MS = 15_000;
export const CATALOG_PAGE_SIZE = 24;

type BaseWearable = {
  id: string;
  name: string;
  thumbnail?: string;
  i18n?: { code: string; text: string }[];
  data?: { category?: string };
};

type CatalogItem = {
  urn?: string;
  name: string;
  thumbnail?: string;
  rarity?: string;
  data?: { wearable?: { category?: string }; emote?: unknown };
};

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
  const timeout = AbortSignal.timeout(REQUEST_TIMEOUT_MS);
  const response = await fetch(url, {
    signal: signal ? AbortSignal.any([signal, timeout]) : timeout,
  });
  if (!response.ok) throw new Error(`${new URL(url).host} replied ${response.status}`);
  return (await response.json()) as T;
}

export function toBaseItem(wearable: BaseWearable): WearableItem | undefined {
  const category = wearable.data?.category;
  if (!isWearableCategory(category)) return undefined;
  const name = wearable.i18n?.find($ => $.code === 'en')?.text || wearable.name;
  return {
    id: wearable.id,
    name,
    category,
    source: 'base',
    thumbnail: wearable.thumbnail || undefined,
  };
}

export function toCatalogItem(item: CatalogItem): WearableItem | undefined {
  const category = item.data?.wearable?.category;
  if (!item.urn || !isWearableCategory(category)) return undefined;
  return {
    id: item.urn,
    name: item.name,
    category,
    source: 'catalog',
    thumbnail: item.thumbnail,
    rarity: item.rarity,
  };
}

let baseWearables: Promise<WearableItem[]> | undefined;

/** The free base-avatar wearables every avatar can wear, fetched once per session. */
export function fetchBaseWearables(): Promise<WearableItem[]> {
  if (!baseWearables) {
    const url = `${CATALYST_BASE_URL}/lambdas/collections/wearables?collectionId=${BASE_AVATARS_COLLECTION}`;
    baseWearables = getJson<{ wearables: BaseWearable[] }>(url)
      .then(({ wearables }) =>
        wearables.map(toBaseItem).filter((item): item is WearableItem => !!item),
      )
      .catch(error => {
        // A failed request is not cached, so the next dialog tries again.
        baseWearables = undefined;
        throw error;
      });
  }
  return baseWearables;
}

/** Published wearables of one category whose name matches `search`. */
export async function searchCatalogWearables(
  category: WearableCategory,
  search: string,
  signal?: AbortSignal,
): Promise<WearableItem[]> {
  const params = new URLSearchParams({
    category: 'wearable',
    wearableCategory: category,
    first: String(CATALOG_PAGE_SIZE),
  });
  const term = search.trim();
  if (term) params.set('search', term);
  const { data } = await getJson<{ data: CatalogItem[] }>(
    `${MARKETPLACE_API_URL}/v1/catalog?${params}`,
    signal,
  );
  return data.map(toCatalogItem).filter((item): item is WearableItem => !!item);
}

const resolved = new Map<string, Promise<WearableItem | undefined>>();

/**
 * Name, category and thumbnail of wearable URNs, for the rows of the inspector. Base avatars come
 * from their collection; other URNs from the catalog. A URN that neither knows resolves to
 * undefined, and the row shows the URN itself.
 */
export function resolveWearables(urns: string[]): Promise<(WearableItem | undefined)[]> {
  const missing = urns.filter($ => !resolved.has($));
  const baseUrns = missing.filter($ => $.startsWith(`${BASE_AVATARS_COLLECTION}:`));
  const otherUrns = missing.filter($ => !$.startsWith(`${BASE_AVATARS_COLLECTION}:`));

  for (const urn of baseUrns) {
    resolved.set(
      urn,
      fetchBaseWearables()
        .then(items => items.find($ => $.id.toLowerCase() === urn.toLowerCase()))
        .catch(() => {
          resolved.delete(urn);
          return undefined;
        }),
    );
  }

  if (otherUrns.length) {
    const params = new URLSearchParams({ first: String(otherUrns.length) });
    for (const urn of otherUrns) params.append('urn', urn);
    const request = getJson<{ data: CatalogItem[] }>(`${MARKETPLACE_API_URL}/v1/catalog?${params}`)
      .then(({ data }) => data.map(toCatalogItem).filter((item): item is WearableItem => !!item))
      .catch(() => {
        for (const urn of otherUrns) resolved.delete(urn);
        return [] as WearableItem[];
      });
    for (const urn of otherUrns) {
      resolved.set(
        urn,
        request.then(items => items.find($ => $.id.toLowerCase() === urn.toLowerCase())),
      );
    }
  }

  return Promise.all(urns.map($ => resolved.get($)!));
}

export function toCatalogEmote(item: CatalogItem): AnimationItem | undefined {
  if (!item.urn || !item.data?.emote) return undefined;
  return {
    id: item.urn,
    name: item.name,
    source: 'catalog',
    thumbnail: item.thumbnail,
    rarity: item.rarity,
  };
}

/** Published emotes whose name matches `search`. */
export async function searchCatalogEmotes(
  search: string,
  signal?: AbortSignal,
): Promise<AnimationItem[]> {
  const params = new URLSearchParams({ category: 'emote', first: String(CATALOG_PAGE_SIZE) });
  const term = search.trim();
  if (term) params.set('search', term);
  const { data } = await getJson<{ data: CatalogItem[] }>(
    `${MARKETPLACE_API_URL}/v1/catalog?${params}`,
    signal,
  );
  return data.map(toCatalogEmote).filter((item): item is AnimationItem => !!item);
}

const resolvedEmotes = new Map<string, Promise<AnimationItem | undefined>>();

/** The name and thumbnail of a published emote, for the animation field. */
export function resolveCatalogEmote(urn: string): Promise<AnimationItem | undefined> {
  let result = resolvedEmotes.get(urn);
  if (!result) {
    const params = new URLSearchParams({ first: '1', urn });
    result = getJson<{ data: CatalogItem[] }>(`${MARKETPLACE_API_URL}/v1/catalog?${params}`)
      .then(({ data }) =>
        data.map(toCatalogEmote).find($ => $?.id.toLowerCase() === urn.toLowerCase()),
      )
      .catch(() => {
        resolvedEmotes.delete(urn);
        return undefined;
      });
    resolvedEmotes.set(urn, result);
  }
  return result;
}

/**
 * The explorers' built-in animations, with the thumbnails the asset-pack catalog keeps for the
 * ones it lists. `names` is the documented list, so an animation the catalog lacks still shows.
 */
export async function fetchBuiltInAnimations(
  names: readonly { value: string; label: string }[],
): Promise<AnimationItem[]> {
  const thumbnails = new Map<string, string>();
  try {
    for (const pack of await fetchLatestCatalog()) {
      for (const asset of pack.assets) {
        const name = getBuiltInEmote(asset);
        const thumbnail = asset.contents['thumbnail.png'];
        if (name && thumbnail) thumbnails.set(name.toLowerCase(), getContentsUrl(thumbnail));
      }
    }
  } catch {
    // the animations still list, without pictures
  }
  return names.map(({ value, label }) => ({
    id: value,
    name: label,
    source: 'builtin' as const,
    thumbnail: thumbnails.get(value.toLowerCase()),
  }));
}
