// from: shop@94584a5c:app/src/lib/contentful.ts; parseItemIds and Campaign.items from shop@2d3bf62

export type ContentfulLocale = "en-US" | "es";
const EN: ContentfulLocale = "en-US";
const FETCHED_LOCALES: ContentfulLocale[] = ["en-US", "es"];

export type LocalizedField<T> = Partial<Record<ContentfulLocale, T>>;
export type SysLink<T extends "Entry" | "Asset"> = { sys: { type: "Link"; linkType: T; id: string } };
export type ContentfulAsset = {
  sys: { id: string };
  fields: { title: LocalizedField<string>; description: LocalizedField<string>; file: LocalizedField<{ url: string }> };
};
type Entry = { sys: { id: string; contentType?: { sys: { id: string } } }; fields: Record<string, Record<string, unknown>> };

export type BannerFields = {
  desktopTitle: LocalizedField<string>;
  mobileTitle: LocalizedField<string>;
  desktopText: LocalizedField<unknown>;
  mobileText: LocalizedField<unknown>;
  showButton: LocalizedField<boolean>;
  buttonLink?: LocalizedField<string>;
  buttonsText?: LocalizedField<string>;
  fullSizeBackground: LocalizedField<SysLink<"Asset">>;
  mobileBackground: LocalizedField<SysLink<"Asset">>;
  [key: string]: unknown;
};

export type CampaignBanner = BannerFields & { id: string };

export type Campaign = {
  name: string | null;
  tabName: LocalizedField<string> | null;
  mainTag: string | null;
  tags: string[];
  collections: string[];
  items: string[];
  banners: Record<string, CampaignBanner>;
  assets: Record<string, ContentfulAsset>;
};

type CmsConfig = { url: string; spaceId: string; environment: string; adminEntryId: string };
type CmsWindow = Window & typeof globalThis & { __SHOP_CMS__?: Partial<CmsConfig> };

const BANNER_CONTENT_TYPE = "banner";
const CAMPAIGN_CONTENT_TYPE = "marketingCampaign";
const IMAGE_HOST = "cms-images.decentraland.org";

function cmsConfig(): CmsConfig | null {
  const w = typeof window !== "undefined" ? (window as CmsWindow).__SHOP_CMS__ : undefined;
  const env = typeof import.meta !== "undefined" ? import.meta.env : undefined;
  const cfg = {
    url: w?.url ?? (env?.VITE_SHOP_CMS_URL as string | undefined) ?? "",
    spaceId: w?.spaceId ?? (env?.VITE_SHOP_CMS_SPACE_ID as string | undefined) ?? "",
    environment: w?.environment ?? (env?.VITE_SHOP_CMS_ENVIRONMENT as string | undefined) ?? "",
    adminEntryId: w?.adminEntryId ?? (env?.VITE_SHOP_CMS_ADMIN_ENTRY as string | undefined) ?? "",
  };
  return cfg.url && cfg.spaceId && cfg.environment && cfg.adminEntryId ? cfg : null;
}

export function toContentfulLocale(locale: string): ContentfulLocale {
  return locale === "es" ? "es" : EN;
}

export function localized<T>(field: LocalizedField<T> | undefined | null, locale: ContentfulLocale): T | undefined {
  if (!field) return undefined;
  return field[locale] ?? field[EN];
}

export function assetUrl(assets: Record<string, ContentfulAsset>, link: SysLink<"Asset"> | undefined): string {
  if (!link) return "";
  const url = assets[link.sys.id]?.fields.file[EN]?.url;
  if (!url) return "";
  return /^https?:\/\//.test(url) ? url : `https:${url}`;
}

export function optimizeAssetUrl(url: string): string {
  try {
    const parsed = new URL(url.startsWith("//") ? `https:${url}` : url);
    parsed.hostname = IMAGE_HOST;
    parsed.protocol = "https:";
    parsed.searchParams.set("q", "80");
    if (/\.(jpe?g|png)$/i.test(parsed.pathname)) parsed.searchParams.set("fm", "webp");
    return parsed.toString();
  } catch {
    return url;
  }
}

export function parseCollectionIds(value: string | undefined): string[] {
  if (!value) return [];
  const seen = new Set<string>();
  for (const part of value.split(",")) {
    const address = part.trim().toLowerCase();
    if (/^0x[0-9a-f]{40}$/.test(address)) seen.add(address);
  }
  return [...seen];
}

function isSysLink(value: unknown): value is SysLink<"Entry" | "Asset"> {
  const sys = (value as { sys?: { type?: unknown; id?: unknown } } | null)?.sys;
  return !!sys && sys.type === "Link" && typeof sys.id === "string";
}

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(url, { signal });
  if (!res.ok) throw new Error(`contentful ${res.status}`);
  return (await res.json()) as T;
}

async function fetchEntryAllLocales(cfg: CmsConfig, id: string, signal?: AbortSignal): Promise<Entry> {
  const base = `${cfg.url}/spaces/${cfg.spaceId}/environments/${cfg.environment}`;
  const perLocale = await Promise.all(
    FETCHED_LOCALES.map(async (locale) => ({
      locale,
      entry: await getJson<{ sys: Entry["sys"]; fields: Record<string, unknown> }>(`${base}/entries/${id}/?locale=${encodeURIComponent(locale)}`, signal),
    })),
  );
  const fields: Record<string, Record<string, unknown>> = {};
  for (const { locale, entry } of perLocale) {
    for (const [key, value] of Object.entries(entry.fields ?? {})) fields[key] = { ...fields[key], [locale]: value };
  }
  for (const field of Object.values(fields)) {
    const english = field[EN];
    if (english === undefined) continue;
    for (const locale of FETCHED_LOCALES) field[locale] ??= english;
  }
  return { sys: perLocale[0]!.entry.sys, fields };
}

async function fetchAsset(cfg: CmsConfig, id: string, signal?: AbortSignal): Promise<ContentfulAsset> {
  const base = `${cfg.url}/spaces/${cfg.spaceId}/environments/${cfg.environment}`;
  const asset = await getJson<{ sys: { id: string }; fields: { title: string; description: string; file: { url: string } } }>(`${base}/assets/${id}/`, signal);
  const file = asset.fields?.file;
  if (!file?.url) throw new Error(`contentful asset ${id} has no file`);
  return {
    sys: asset.sys,
    fields: {
      title: { [EN]: asset.fields.title },
      description: { [EN]: asset.fields.description },
      file: { [EN]: { ...file, url: optimizeAssetUrl(file.url) } },
    },
  };
}

function linkedIds(fieldSets: Record<string, unknown>[], linkType: "Entry" | "Asset"): string[] {
  const ids = new Set<string>();
  const collect = (value: unknown) => {
    if (Array.isArray(value)) {
      value.forEach(collect);
      return;
    }
    if (isSysLink(value) && value.sys.linkType === linkType) ids.add(value.sys.id);
  };
  for (const fields of fieldSets) {
    for (const localizedField of Object.values(fields ?? {})) Object.values((localizedField ?? {}) as Record<string, unknown>).forEach(collect);
  }
  return [...ids];
}

function settled<T>(results: PromiseSettledResult<T>[]): T[] {
  return results.flatMap((r) => (r.status === "fulfilled" ? [r.value] : []));
}

export function parseItemIds(value: string | undefined): string[] {
  if (!value) return [];
  const seen = new Set<string>();
  for (const part of value.split(",")) {
    const trimmed = part.trim();
    if (!trimmed) continue;
    const match = /\/contracts\/(0x[0-9a-fA-F]{40})\/items\/(\d+)/.exec(trimmed) ?? /^(0x[0-9a-fA-F]{40})-(\d+)$/.exec(trimmed);
    if (!match) continue;
    seen.add(`${match[1]!.toLowerCase()}-${match[2]!.replace(/^0+(?=\d)/, "")}`);
  }
  return [...seen];
}

function dedupeTags(tags: (string | undefined)[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const tag of tags) {
    const trimmed = tag?.trim();
    if (!trimmed || seen.has(trimmed.toLowerCase())) continue;
    seen.add(trimmed.toLowerCase());
    out.push(trimmed);
  }
  return out;
}

export async function fetchCampaign(signal?: AbortSignal): Promise<Campaign | null> {
  const cfg = cmsConfig();
  if (!cfg) return null;
  const admin = await fetchEntryAllLocales(cfg, cfg.adminEntryId, signal);
  const entries = settled(await Promise.allSettled(linkedIds([admin.fields], "Entry").map((id) => fetchEntryAllLocales(cfg, id, signal))));
  const entryById = new Map(entries.map((e) => [e.sys.id, e]));
  const assetIds = linkedIds([admin.fields, ...entries.map((e) => e.fields)], "Asset");
  const assets: Record<string, ContentfulAsset> = {};
  for (const asset of settled(await Promise.allSettled(assetIds.map((id) => fetchAsset(cfg, id, signal))))) assets[asset.sys.id] = asset;

  const banners: Record<string, CampaignBanner> = {};
  for (const [key, field] of Object.entries(admin.fields)) {
    const link = field?.[EN];
    if (!isSysLink(link)) continue;
    const entry = entryById.get(link.sys.id);
    if (entry?.sys.contentType?.sys.id !== BANNER_CONTENT_TYPE) continue;
    banners[key] = { ...(entry.fields as unknown as BannerFields), id: link.sys.id };
  }

  const campaign = entries.find((e) => e.sys.contentType?.sys.id === CAMPAIGN_CONTENT_TYPE)?.fields;
  const mainTag = (campaign?.mainTag?.[EN] as string | undefined)?.trim() || null;
  const additional = (campaign?.additionalTags?.[EN] as string[] | undefined) ?? [];
  return {
    name: (campaign?.name?.[EN] as string | undefined) ?? null,
    tabName: (campaign?.marketplaceTabName as LocalizedField<string> | undefined) ?? null,
    mainTag,
    tags: dedupeTags([mainTag ?? undefined, ...additional]),
    collections: parseCollectionIds(campaign?.collectionIds?.[EN] as string | undefined),
    items: parseItemIds(campaign?.itemIds?.[EN] as string | undefined),
    banners,
    assets,
  };
}
