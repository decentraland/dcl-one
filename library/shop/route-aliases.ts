import { landNetwork, landTokenId } from "./land/contracts";
import { CHAIN } from "./model/chain";

export const SHOP_BASE = "/shop";

export type ShopAlias = {
  from: string;
  kind: "alias" | "renamed" | "resolver";
  to: string;
};

export const SHOP_ALIASES: readonly ShopAlias[] = [
  { from: "", kind: "alias", to: "/shop/overview" },
  { from: "market", kind: "alias", to: "/shop/items" },
  { from: "my-purchases", kind: "alias", to: "/shop/activity" },
  { from: "import", kind: "alias", to: "/shop/activity?section=listings" },
  { from: "authorizations", kind: "alias", to: "/shop/settings" },
  { from: "assets", kind: "renamed", to: "/shop/items" },
  { from: "assets/*", kind: "renamed", to: "/shop/items" },
  { from: "my-assets", kind: "renamed", to: "/shop/my-items" },
  { from: "browse", kind: "resolver", to: "/shop/items" },
  { from: "parcels/:x/:y", kind: "resolver", to: "/shop/token/{land}/{tokenId}" },
  { from: "parcels/:x/:y/detail", kind: "resolver", to: "/shop/token/{land}/{tokenId}" },
  { from: "estates/:estateId", kind: "resolver", to: "/shop/token/{estate}/{estateId}" },
  { from: "estates/:estateId/detail", kind: "resolver", to: "/shop/token/{estate}/{estateId}" },
];

type ShopAliasOptions = {
  contracts?: { land: string; estate: string };
};

export function aliasTarget(to: string, search: string, hash: string): string {
  const mark = to.indexOf("?");
  const path = mark === -1 ? to : to.slice(0, mark);
  const params = new URLSearchParams(search);
  for (const [key, value] of new URLSearchParams(mark === -1 ? "" : to.slice(mark + 1))) params.set(key, value);
  const query = params.toString();
  return `${path}${query ? `?${query}` : ""}${hash}`;
}

export function renamedTarget(to: string, rest: string | undefined, search: string, hash: string): string {
  return `${to}${rest ? `/${rest}` : ""}${search}${hash}`;
}

function browseTarget(search: string, hash: string): string {
  const section = new URLSearchParams(search).get("section");
  if (section === "land" || section === "lands") return `/shop/lands${hash}`;
  if (section === "ens" || section === "names") return `/shop/names${hash}`;
  return `/shop/items${search}${hash}`;
}

function matchPath(pattern: string, path: string): Record<string, string> | null {
  const want = pattern.split("/").filter(Boolean);
  const got = path.split("/").filter(Boolean);
  const params: Record<string, string> = {};
  for (let i = 0; i < want.length; i++) {
    const seg = want[i]!;
    if (seg === "*") {
      params["*"] = got.slice(i).join("/");
      return params;
    }
    const value = got[i];
    if (value === undefined) return null;
    if (seg.startsWith(":")) {
      // A malformed escape (`%E0`) names no page: it is no match, never a thrown URIError.
      try {
        params[seg.slice(1)] = decodeURIComponent(value);
      } catch {
        return null;
      }
    } else if (seg !== value) return null;
  }
  return got.length === want.length ? params : null;
}

export function resolveShopAliasTarget(alias: ShopAlias, params: Record<string, string>, search: string, hash: string, opts: ShopAliasOptions): string | null {
  if (alias.kind === "alias") return aliasTarget(alias.to, search, hash);
  if (alias.kind === "renamed") return renamedTarget(alias.to, alias.from.endsWith("/*") ? params["*"] : undefined, search, hash);
  if (alias.from === "browse") return browseTarget(search, hash);
  const contracts = opts.contracts ?? landNetwork(CHAIN.ETHEREUM);
  if (alias.from.startsWith("parcels")) {
    const x = Number(params.x);
    const y = Number(params.y);
    if (!Number.isSafeInteger(x) || !Number.isSafeInteger(y)) return null;
    return `/shop/token/${contracts.land.toLowerCase()}/${landTokenId(x, y)}${search}${hash}`;
  }
  if (!/^[0-9]+$/.test(params.estateId ?? "")) return null;
  return `/shop/token/${contracts.estate.toLowerCase()}/${params.estateId}${search}${hash}`;
}

export function resolveShopAlias(pathname: string, search = "", hash = "", opts: ShopAliasOptions = {}): string | null {
  if (pathname !== SHOP_BASE && !pathname.startsWith(`${SHOP_BASE}/`)) return null;
  const rest = pathname.slice(SHOP_BASE.length).replace(/^\/+|\/+$/g, "");
  for (const alias of SHOP_ALIASES) {
    const params = matchPath(alias.from, rest);
    if (params) return resolveShopAliasTarget(alias, params, search, hash, opts);
  }
  return null;
}
