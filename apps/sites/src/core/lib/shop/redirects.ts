// Where every old Marketplace GET page goes in the Shop (SHOP.md "Redirect old Marketplace GET pages"):
// v2's own /marketplace/* pages and /shop?tab= tabs, and the classic
// Marketplace UI's paths (decentraland.org/marketplace/<path>, served here under /marketplace). Each
// target keeps the asset's identity (chain contract + item or token id), the rest of the query (return
// destinations such as `redirectTo`, tracking such as `from`/`src`, the iOS app's `view=mobile-iap`)
// and the hash. Only GET pages redirect: a write is never redirected and a signed API request never
// reaches these routes. Pure, so the SSR loaders and the Explorer's native runtime share it.

import { landIdentity, landRedirectTarget } from "@ui/shop/land/redirects";
import { isNameRegistrar, landCategoryOf } from "@ui/shop/land/contracts";
import { aliasTarget, resolveShopAlias, SHOP_BASE } from "@ui/shop/routes";

const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
const NUMBER = /^[0-9]+$/;

function withQuery(path: string, query: URLSearchParams, hash = ""): string {
  const s = query.toString();
  return `${path}${s ? `${path.includes("?") ? "&" : "?"}${s}` : ""}${hash}`;
}

function without(query: URLSearchParams, keys: readonly string[]): URLSearchParams {
  const out = new URLSearchParams();
  for (const [k, v] of query) if (!keys.includes(k)) out.append(k, v);
  return out;
}

/** A LAND, estate or NAME registrar contract: its ids are tokens, never collection items. */
export function isTokenRegistry(contract: string): boolean {
  return !!landCategoryOf(contract) || isNameRegistrar(contract);
}

/** v2's asset refs: `<contract>-<id>` (or `:`), as /marketplace/:id, ?item=, ?nft=, ?id= wrote them. */
export function parseAssetRef(ref: string | null | undefined): { contract: string; id: string } | null {
  const m = /^(0x[0-9a-fA-F]{40})[-:]([0-9]+)$/.exec((ref ?? "").trim());
  return m ? { contract: m[1]!.toLowerCase(), id: m[2]! } : null;
}

/** The Shop page of an asset: an item page for a collection item, a token page for a registry token. */
export function assetPage(contract: string, id: string, as: "item" | "token" = isTokenRegistry(contract) ? "token" : "item"): string {
  const c = contract.toLowerCase();
  return as === "token" || isTokenRegistry(c) ? `${SHOP_BASE}/token/${c}/${id}` : `${SHOP_BASE}/item/${c}/${id}`;
}

// ---------------------------------------------------------------------------------------------------
// Browse filters: v2's /shop and the classic Marketplace's /browse, as the Shop's Items page reads them
// ---------------------------------------------------------------------------------------------------

const SORTS: Record<string, string> = {
  newest: "newest",
  recently_listed: "newest",
  cheapest: "price-asc",
  most_expensive: "price-desc",
  name: "name",
};

const SUBCATEGORIES: Record<string, string> = {
  head: "Head",
  eyebrows: "Eyebrows",
  eyes: "Eyes",
  facial_hair: "Facial Hair",
  hair: "Hair",
  mouth: "Mouth",
  upper_body: "Upper Body",
  lower_body: "Lower Body",
  feet: "Feet",
  hands: "Handwear",
  accessories: "Accessories",
  earring: "Earring",
  eyewear: "Eyewear",
  hat: "Hat",
  helmet: "Helmet",
  mask: "Mask",
  tiara: "Tiara",
  top_head: "Top Head",
  skin: "Skins",
  dance: "Dance",
  stunt: "Stunt",
  greetings: "Greetings",
  fun: "Fun",
  poses: "Poses",
  reactions: "Reactions",
  horror: "Horror",
  miscellaneous: "Miscellaneous",
};

/** The classic Marketplace's browse `section` as the Items page's category and sub-category. */
export function sectionCategory(section: string | null): { category?: string; subCategory?: string } {
  if (!section) return {};
  const m = /^(wearables|emotes)(?:_(.+))?$/.exec(section);
  if (!m) return {};
  const category = m[1] === "wearables" ? "wearable" : "emote";
  const sub = m[2] ? SUBCATEGORIES[m[2]] : undefined;
  return sub ? { category, subCategory: sub } : { category };
}

/** The Items page query for a v2 /shop or classic /browse query; unknown keys are kept as they are. */
export function itemsQuery(q: URLSearchParams): URLSearchParams {
  const out = without(q, ["tab", "section", "search", "rarity", "rarities", "sortBy", "page", "onlyOnSale", "onlySmart", "status", "assetType", "vendor", "category"]);
  const { category, subCategory } = sectionCategory(q.get("section"));
  const v2Category = q.get("category");
  const cat = category ?? (v2Category === "wearable" || v2Category === "emote" ? v2Category : undefined);
  if (cat) out.set("category", cat);
  if (subCategory) out.set("subCategory", subCategory);
  const search = (q.get("search") ?? "").trim();
  if (search && !out.has("q")) out.set("q", search);
  const rarities = [q.get("rarity"), ...(q.get("rarities") ?? "").split(/[_,]/)].map((r) => (r ?? "").trim().toLowerCase()).filter(Boolean);
  if (rarities.length) out.set("rarities", [...new Set(rarities)].join(","));
  const sort = SORTS[q.get("sortBy") ?? ""];
  if (sort) out.set("sort", sort);
  const onlyOnSale = q.get("onlyOnSale");
  const status = q.get("status");
  if (status === "not_for_sale") out.set("status", "not_for_sale");
  else if (status === "on_sale" || status === "only_listing" || status === "only_minting" || onlyOnSale === "true") out.set("status", "on_sale");
  else if (onlyOnSale === "false") out.set("status", "all");
  if (q.get("onlySmart") === "true") out.set("smart", "true");
  return out;
}

/** The classic Marketplace's /browse (and the iOS app's `/` in IAP mode). */
function browseTarget(q: URLSearchParams, hash: string): string {
  const section = q.get("section") ?? "";
  if (section === "land" || section === "parcels" || section === "estates") return landRedirectTarget("lands", q.toString()) + hash;
  if (section === "ens" || section === "names") return withQuery(`${SHOP_BASE}/names`, (() => {
    const out = without(q, ["section", "assetType", "vendor"]);
    out.set("section", "resale");
    return out;
  })(), hash);
  const contracts = (q.get("contracts") ?? "").split("_").filter((c) => ADDRESS.test(c));
  if (contracts.length === 1) return withQuery(`${SHOP_BASE}/collection/${contracts[0]!.toLowerCase()}`, itemsQuery(without(q, ["contracts"])), hash);
  return withQuery(`${SHOP_BASE}/items`, itemsQuery(q), hash);
}

// ---------------------------------------------------------------------------------------------------
// v2's /shop?tab= (the Shop tabs before the Shop pages)
// ---------------------------------------------------------------------------------------------------

const V2_TABS: Record<string, string> = {
  "my-assets": `${SHOP_BASE}/my-items`,
  "my-favorites": `${SHOP_BASE}/my-favorites`,
  "my-lists": `${SHOP_BASE}/my-favorites?tab=lists`,
  names: `${SHOP_BASE}/names`,
  credits: `${SHOP_BASE}/credits`,
  cart: `${SHOP_BASE}/cart`,
};

/** `/shop` itself: v2's `?tab=` to its page, else the overview (the upstream Shop's `/` alias). */
export function shopIndexTarget(search: string, hash = ""): string {
  const q = new URLSearchParams(search);
  const tab = q.get("tab");
  if (tab === "all-assets") return withQuery(`${SHOP_BASE}/items`, itemsQuery(q), hash);
  const page = tab ? V2_TABS[tab] : undefined;
  if (page) return aliasTarget(page, without(q, ["tab"]).toString(), hash);
  return resolveShopAlias(SHOP_BASE, withQuery("", without(q, ["tab"])), hash) ?? `${SHOP_BASE}/overview`;
}

// ---------------------------------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------------------------------

/** The classic Marketplace's /account `section`, as the Shop page that shows it. */
function accountSectionTarget(section: string | null): string {
  switch (section) {
    case "on_sale":
    case "sales":
    case "recently_sold":
    case "collections":
      return `${SHOP_BASE}/my-store`;
    case "store_settings":
      return `${SHOP_BASE}/store-settings`;
    case "lists":
      return `${SHOP_BASE}/my-favorites?tab=lists`;
    case null:
    case "":
      return `${SHOP_BASE}/my-items`;
    default: {
      if (section.startsWith("wearables")) return `${SHOP_BASE}/my-items?section=wearables`;
      if (section.startsWith("emotes")) return `${SHOP_BASE}/my-items?section=emotes`;
      return `${SHOP_BASE}/my-items?section=${encodeURIComponent(section)}`;
    }
  }
}

const V2_ACCOUNT_TABS: Record<string, string> = {
  "on-sale": "on_sale",
  "on-rent": "on_rent",
  collections: "collections",
  bids: "bids",
};

/** /marketplace/account: v2's `?tab=&address=` or the classic `?section=`. */
function accountTarget(q: URLSearchParams, hash: string): string {
  const address = q.get("address")?.trim();
  const tab = q.get("tab");
  const section = q.get("section") ?? (tab ? (V2_ACCOUNT_TABS[tab] ?? null) : null);
  const rest = without(q, ["address", "tab", "section", "vendor", "assetType", "page"]);
  if (address && ADDRESS.test(address)) return withQuery(`${SHOP_BASE}/accounts/${address.toLowerCase()}`, rest, hash);
  return aliasTarget(accountSectionTarget(section), rest.toString(), hash);
}

// ---------------------------------------------------------------------------------------------------
// v2's action pages, which named the asset in the query
// ---------------------------------------------------------------------------------------------------

const IDENTITY_KEYS = ["id", "nft", "item", "contractAddress", "contract", "tokenId", "itemId", "landId", "x", "y", "estateId"];

function tokenFromQuery(q: URLSearchParams): { contract: string; tokenId: string } | null {
  const nft = parseAssetRef(q.get("nft"));
  if (nft) return { contract: nft.contract, tokenId: nft.id };
  const land = landIdentity(q);
  return land ? { contract: land.contract, tokenId: land.tokenId } : null;
}

/**
 * A v2 token action (`/marketplace/{sell,transfer,cancel,accept-bid,manage}`): the token's own page, else
 * `fallback`. The listing (`order`) and bid (`bid`) a link named are kept either way: the Shop pages open
 * that order's cancellation and that bid's review from them.
 */
function tokenAction(q: URLSearchParams, hash: string, action: string, fallback: string): string {
  const token = tokenFromQuery(q);
  const rest = without(q, [...IDENTITY_KEYS, "step", "owner", "wallet", "address", "creator", "ownership"]);
  if (token) return withQuery(`${SHOP_BASE}/token/${token.contract}/${token.tokenId}/${action}`, rest, hash);
  return aliasTarget(fallback, rest.toString(), hash);
}

function assetFromQuery(q: URLSearchParams, keys: readonly string[]): { contract: string; id: string } | null {
  for (const key of keys) {
    const ref = parseAssetRef(q.get(key));
    if (ref) return ref;
  }
  const contract = (q.get("contractAddress") ?? q.get("contract") ?? "").toLowerCase();
  const id = q.get("itemId") ?? q.get("tokenId") ?? "";
  return ADDRESS.test(contract) && NUMBER.test(id) ? { contract, id } : null;
}

// ---------------------------------------------------------------------------------------------------
// The classic Marketplace's asset paths
// ---------------------------------------------------------------------------------------------------

const TOKEN_ACTIONS = new Set(["sell", "bid", "cancel", "transfer", "manage", "bids"]);

function contractsPath(segments: string[], q: URLSearchParams, hash: string): string | null {
  const [, contract, kind, id, action, sub] = segments;
  if (!contract || !ADDRESS.test(contract) || (kind !== "tokens" && kind !== "items") || !id || !NUMBER.test(id)) return null;
  const base = `${SHOP_BASE}/${kind === "tokens" ? "token" : "item"}/${contract.toLowerCase()}/${id}`;
  if (!action) return withQuery(base, q, hash);
  if (action === "buy") return withQuery(sub === "status" ? `${base}/buy/status` : base, q, hash);
  if (kind === "tokens" && TOKEN_ACTIONS.has(action) && !sub) return withQuery(`${base}/${action}`, q, hash);
  if (kind === "items" && action === "bid" && !sub) return withQuery(`${base}/bid`, q, hash);
  return null;
}

/** A same-site return path, or null (never another origin: `//host` and `/\host` are refused). */
function safeReturn(raw: string | null): string | null {
  if (!raw || !raw.startsWith("/") || raw.startsWith("//") || raw.startsWith("/\\")) return null;
  return raw;
}

/**
 * The Shop page for an old /marketplace GET page (`pathname` includes the /marketplace prefix, in any
 * case, as the classic Marketplace's router matched it); null when `pathname` is not under /marketplace.
 */
export function marketplaceTarget(pathname: string, search = "", hash = ""): string | null {
  const m = /^\/marketplace(?=\/|$)/i.exec(pathname);
  if (!m) return null;
  const rest = pathname.slice(m[0].length).replace(/\/+$/, "");
  let segments: string[];
  try {
    segments = rest.split("/").filter(Boolean).map((s) => decodeURIComponent(s));
  } catch {
    return `${SHOP_BASE}/not-found`;
  }
  const q = new URLSearchParams(search);
  const [first = "", second] = segments;
  const one = segments.length === 1;

  if (segments.length === 0) {
    if (q.get("view") === "mobile-iap") return browseTarget(q, hash);
    return aliasTarget(`${SHOP_BASE}/overview`, search, hash);
  }
  if (first === "contracts") return contractsPath(segments, q, hash) ?? withQuery(`${SHOP_BASE}/${rest.slice(1)}`, q, hash);
  if (first === "parcels" || first === "estates") return resolveShopAlias(`${SHOP_BASE}${rest}`, search, hash) ?? withQuery(`${SHOP_BASE}/${rest.slice(1)}`, q, hash);
  if (first === "collections" && segments.length === 2 && ADDRESS.test(second!)) return withQuery(`${SHOP_BASE}/collection/${second!.toLowerCase()}`, q, hash);
  if (first === "accounts" && segments.length === 2 && ADDRESS.test(second!)) return withQuery(`${SHOP_BASE}/accounts/${second!.toLowerCase()}`, without(q, ["vendor", "assetType", "page"]), hash);
  if (first === "lists" && segments.length === 2) return withQuery(`${SHOP_BASE}/my-favorites/${encodeURIComponent(second!)}`, q, hash);
  if (first === "names" && segments.length === 2) {
    if (second === "claim") return withQuery(`${SHOP_BASE}/names`, q, hash);
    if (second === "browse") return browseTarget(new URLSearchParams([...without(q, ["section"]), ["section", "ens"]]), hash);
  }

  if (one) {
    switch (first) {
      // The classic Marketplace's pages.
      case "lands":
      case "land":
        return landRedirectTarget("lands", search) + hash;
      case "browse":
        return q.has("section") || q.get("view") === "mobile-iap" ? browseTarget(q, hash) : withQuery(`${SHOP_BASE}/items`, itemsQuery(q), hash);
      case "campaign":
        return withQuery(`${SHOP_BASE}/event`, q, hash);
      case "account":
        return accountTarget(q, hash);
      case "lists": {
        const listId = q.get("listId")?.trim();
        const out = without(q, ["listId", "address", "section", "view", "page", "assetType"]);
        if (listId) return withQuery(`${SHOP_BASE}/my-favorites/${encodeURIComponent(listId)}`, out, hash);
        return aliasTarget(`${SHOP_BASE}/my-favorites?tab=lists`, out.toString(), hash);
      }
      case "sign-in": {
        // The classic sign-in page returned to a path of its own app (`/account`), or a full one.
        const back = safeReturn(q.get("redirectTo"));
        if (!back) return `${SHOP_BASE}/my-items`;
        const url = new URL(back, "https://shop.invalid");
        if (/^\/shop(?=\/|$)/i.test(url.pathname)) return back;
        const path = /^\/marketplace(?=\/|$)/i.test(url.pathname) ? url.pathname : `/marketplace${url.pathname}`;
        return marketplaceTarget(path, url.search, url.hash) ?? `${SHOP_BASE}/my-items`;
      }
      case "success":
        return withQuery(`${SHOP_BASE}/success`, q, hash);
      case "settings":
        return withQuery(q.get("tab") === "store" ? `${SHOP_BASE}/store-settings` : `${SHOP_BASE}/settings`, without(q, ["tab", "address"]), hash);
      case "activity":
        return withQuery(`${SHOP_BASE}/activity`, q, hash);

      // v2's own pages.
      case "shop":
        return shopIndexTarget(search, hash);
      case "mint":
      case "buy": {
        const item = parseAssetRef(q.get("item"));
        const nft = parseAssetRef(q.get("nft"));
        const out = without(q, ["item", "nft"]);
        if (nft) return withQuery(assetPage(nft.contract, nft.id, "token"), out, hash);
        if (item) return withQuery(assetPage(item.contract, item.id), out, hash);
        return withQuery(`${SHOP_BASE}/items`, out, hash);
      }
      case "bid": {
        const ref = assetFromQuery(q, ["id", "item", "nft"]);
        const out = without(q, [...IDENTITY_KEYS, "step"]);
        if (ref) return withQuery(`${assetPage(ref.contract, ref.id)}/bid`, out, hash);
        return withQuery(`${SHOP_BASE}/items`, out, hash);
      }
      case "sell":
        return tokenAction(q, hash, "sell", `${SHOP_BASE}/my-items`);
      case "transfer":
        return tokenAction(q, hash, "transfer", `${SHOP_BASE}/my-items`);
      case "cancel":
        return tokenAction(q, hash, "cancel", `${SHOP_BASE}/my-store`);
      case "accept-bid":
        return tokenAction(q, hash, "bids", `${SHOP_BASE}/my-items?section=bids`);
      case "manage":
        return tokenAction(q, hash, "manage", `${SHOP_BASE}/my-items`);
      case "rent-land":
        return landRedirectTarget("rent", search) + hash;
      case "cart":
        return withQuery(`${SHOP_BASE}/cart`, q, hash);
      case "checkout": {
        const express = parseAssetRef(q.get("express"));
        const out = without(q, ["express", "state", "tour", "pay", "step"]);
        if (express) return withQuery(assetPage(express.contract, express.id), out, hash);
        return withQuery(`${SHOP_BASE}/cart`, out, hash);
      }
      case "credits":
      case "packs":
        return withQuery(`${SHOP_BASE}/credits`, without(q, ["step", "granted", "wallet"]), hash);
      case "names":
      case "claim-name":
        return withQuery(`${SHOP_BASE}/names`, without(q, ["step", "owner"]), hash);
      case "collection": {
        const contract = q.get("contract") ?? q.get("contractAddress");
        const out = itemsQuery(without(q, ["contract", "contractAddress"]));
        if (contract && ADDRESS.test(contract)) return withQuery(`${SHOP_BASE}/collection/${contract.toLowerCase()}`, out, hash);
        return withQuery(`${SHOP_BASE}/items`, out, hash);
      }
    }
    // v2's /marketplace/:id, the item (or registry token) its ref names.
    const ref = parseAssetRef(first);
    if (ref) return withQuery(assetPage(ref.contract, ref.id), q, hash);
  }

  // Anything else keeps its path under /shop, where it is a Shop page or the Shop's not-found page.
  return withQuery(`${SHOP_BASE}/${segments.map(encodeURIComponent).join("/")}`, q, hash);
}

/** Where an old page (a /marketplace path or /shop itself) goes; null when the path is neither. */
export function legacyShopTarget(pathname: string, search = "", hash = ""): string | null {
  if (/^\/shop\/?$/i.test(pathname)) return shopIndexTarget(search, hash);
  return marketplaceTarget(pathname, search, hash);
}
