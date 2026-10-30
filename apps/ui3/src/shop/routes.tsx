// from: shop@94584a5c:app/src/App.tsx (the route table, AliasRedirect, RenamedPathRedirect)
import { lazy, Suspense, useMemo, type ComponentType, type ReactNode } from "react";
import { Navigate, useLocation, useParams, useRoutes, type RouteObject } from "react-router";

import Spinner from "../atoms/Spinner";
import { ESTATE_CONTRACT, LAND_CONTRACT, landTokenId } from "../data/catalyst/landListing";

export const SHOP_BASE = "/shop";

export type ShopPageId =
  | "overview"
  | "items"
  | "event"
  | "itemDetail"
  | "tokenDetail"
  | "collection"
  | "creator"
  | "outfitStudioManage"
  | "outfitStudioNew"
  | "outfitStudioEdit"
  | "outfitDetail"
  | "storeSettings"
  | "myItems"
  | "myStore"
  | "myFavorites"
  | "favoriteList"
  | "activity"
  | "cart"
  | "credits"
  | "success"
  | "lands"
  | "names"
  | "account"
  | "settings"
  | "rankings"
  | "tokenSell"
  | "tokenCancel"
  | "tokenTransfer"
  | "tokenBid"
  | "tokenBids"
  | "tokenManage"
  | "tokenRent"
  | "tokenBuyStatus"
  | "itemBid"
  | "itemBuyStatus"
  | "notFound";

export type ShopPage = {
  id: ShopPageId;
  path: string;
  module: string;
  exportName: string;
  title: string;
};

const page = (id: ShopPageId, path: string, module: string, title: string, exportName?: string): ShopPage => ({
  id,
  path,
  module,
  title,
  exportName: exportName ?? module.replace(/^.*\//, "").replace(/\.tsx$/, ""),
});

export const SHOP_PAGES: readonly ShopPage[] = [
  page("overview", "overview", "./pages/Overview.tsx", "Overview"),
  page("items", "items", "./pages/Items.tsx", "Browse Collectibles"),
  page("event", "event", "./pages/Event.tsx", "Browse Collectibles"),
  page("itemDetail", "item/:contractAddress/:itemId", "./pages/ItemDetail.tsx", "Item", "ItemDetailRoute"),
  page("tokenDetail", "token/:contractAddress/:tokenId", "./pages/ItemDetail.tsx", "Item", "ItemDetailRoute"),
  page("collection", "collection/:contractAddress", "./pages/Collection.tsx", "Collection"),
  page("creator", "items/creator/:address", "./pages/Creator.tsx", "Creator"),
  page("outfitStudioManage", "outfits/manage", "./pages/OutfitStudio.tsx", "Outfit Studio"),
  page("outfitStudioNew", "outfits/new", "./pages/OutfitStudio.tsx", "Outfit Studio"),
  page("outfitStudioEdit", "outfits/:id/edit", "./pages/OutfitStudio.tsx", "Outfit Studio"),
  page("outfitDetail", "items/outfits/:id", "./pages/OutfitDetail.tsx", "Outfit"),
  page("storeSettings", "store-settings", "./pages/StoreSettings.tsx", "Store Settings"),
  page("myItems", "my-items", "./pages/MyItems.tsx", "My Items"),
  page("myStore", "my-store", "./pages/MyStore.tsx", "My Store"),
  page("myFavorites", "my-favorites", "./pages/MyFavorites.tsx", "My Favorites"),
  page("favoriteList", "my-favorites/:listId", "./pages/FavoriteList.tsx", "Favorites list"),
  page("activity", "activity", "./pages/Activity.tsx", "Activity"),
  page("cart", "cart", "./pages/Cart.tsx", "Cart"),
  page("credits", "credits", "./pages/Credits.tsx", "Buy Credits"),
  page("success", "success", "./pages/Success.tsx", "Purchase Complete"),
  page("lands", "lands", "./pages/Lands.tsx", "Land"),
  page("names", "names", "./pages/Names.tsx", "Browse Collectibles"),
  page("account", "accounts/:address", "./pages/Account.tsx", "Account"),
  page("settings", "settings", "./pages/Settings.tsx", "Settings"),
  page("rankings", "rankings", "./pages/Rankings.tsx", "Rankings"),
  page("tokenSell", "token/:contractAddress/:tokenId/sell", "./pages/TokenSell.tsx", "Sell"),
  page("tokenCancel", "token/:contractAddress/:tokenId/cancel", "./components/RemoveListingModal.tsx", "Cancel listing"),
  page("tokenTransfer", "token/:contractAddress/:tokenId/transfer", "./pages/TokenTransfer.tsx", "Transfer"),
  page("tokenBid", "token/:contractAddress/:tokenId/bid", "./pages/TokenBid.tsx", "Place a bid"),
  page("tokenBids", "token/:contractAddress/:tokenId/bids", "./workflows/AcceptBid.tsx", "Bids"),
  page("tokenManage", "token/:contractAddress/:tokenId/manage", "./pages/TokenManage.tsx", "Manage"),
  page("tokenRent", "token/:contractAddress/:tokenId/rent", "./pages/TokenRent.tsx", "Rent"),
  page("tokenBuyStatus", "token/:contractAddress/:tokenId/buy/status", "./pages/BuyStatus.tsx", "Purchase status"),
  page("itemBid", "item/:contractAddress/:itemId/bid", "./pages/TokenBid.tsx", "Place a bid"),
  page("itemBuyStatus", "item/:contractAddress/:itemId/buy/status", "./pages/BuyStatus.tsx", "Purchase status"),
  page("notFound", "*", "./pages/NotFound.tsx", "Page Not Found"),
];

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

export type ShopRouteOptions = {
  contracts?: { land: string; estate: string };
  iapMode?: () => boolean;
  fallback?: ReactNode;
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
    if (seg.startsWith(":")) params[seg.slice(1)] = decodeURIComponent(value);
    else if (seg !== value) return null;
  }
  return got.length === want.length ? params : null;
}

function resolveTarget(alias: ShopAlias, params: Record<string, string>, search: string, hash: string, opts: ShopRouteOptions): string | null {
  if (alias.kind === "alias") return aliasTarget(alias.to, search, hash);
  if (alias.kind === "renamed") return renamedTarget(alias.to, alias.from.endsWith("/*") ? params["*"] : undefined, search, hash);
  if (alias.from === "browse") return browseTarget(search, hash);
  const contracts = opts.contracts ?? { land: LAND_CONTRACT, estate: ESTATE_CONTRACT };
  if (alias.from.startsWith("parcels")) {
    const x = Number(params.x);
    const y = Number(params.y);
    if (!Number.isSafeInteger(x) || !Number.isSafeInteger(y)) return null;
    return `/shop/token/${contracts.land.toLowerCase()}/${landTokenId(x, y)}${search}${hash}`;
  }
  if (!/^[0-9]+$/.test(params.estateId ?? "")) return null;
  return `/shop/token/${contracts.estate.toLowerCase()}/${params.estateId}${search}${hash}`;
}

export function resolveShopAlias(pathname: string, search = "", hash = "", opts: ShopRouteOptions = {}): string | null {
  if (pathname !== SHOP_BASE && !pathname.startsWith(`${SHOP_BASE}/`)) return null;
  const rest = pathname.slice(SHOP_BASE.length).replace(/^\/+|\/+$/g, "");
  for (const alias of SHOP_ALIASES) {
    const params = matchPath(alias.from, rest);
    if (params) return resolveTarget(alias, params, search, hash, opts);
  }
  return null;
}

export function shopPath(id: Exclude<ShopPageId, "notFound">, params: Record<string, string | number> = {}): string {
  const found = SHOP_PAGES.find((p) => p.id === id)!;
  const path = found.path.replace(/:([A-Za-z]+)/g, (_, name: string) => {
    const value = params[name];
    if (value === undefined) throw new TypeError(`missing :${name} for ${id}`);
    return encodeURIComponent(String(value));
  });
  return `${SHOP_BASE}/${path}`;
}

type PageModule = { default?: ComponentType; [name: string]: unknown };

const MODULES = import.meta.glob<PageModule>([
  "./pages/*.tsx",
  "./workflows/*.tsx",
  "./components/RemoveListingModal.tsx",
  "!./**/*.stories.tsx",
  "!./**/*.test.tsx",
]);

function ShopPlaceholder({ title }: { title: string }) {
  return (
    <section className="shop-placeholder" role="status" aria-label={title}>
      <h1>{title}</h1>
      <p>This page is moving into the Shop.</p>
    </section>
  );
}

const lazyCache = new Map<string, ComponentType>();

export function shopPageComponent(page: ShopPage, modules: Record<string, () => Promise<PageModule>> = MODULES): ComponentType {
  const key = `${page.module}#${page.exportName}#${page.title}`;
  const cached = modules === MODULES ? lazyCache.get(key) : undefined;
  if (cached) return cached;
  const load = modules[page.module];
  const Component = load
    ? lazy(async () => {
        const mod = await load();
        const found = (mod.default ?? mod[page.exportName]) as ComponentType | undefined;
        return { default: found ?? (() => <ShopPlaceholder title={page.title} />) };
      })
    : () => <ShopPlaceholder title={page.title} />;
  if (modules === MODULES) lazyCache.set(key, Component);
  return Component;
}

function AliasRoute({ alias, opts }: { alias: ShopAlias; opts: ShopRouteOptions }) {
  const params = useParams();
  const { search, hash } = useLocation();
  const target = resolveTarget(alias, params as Record<string, string>, search, hash, opts);
  return target ? <Navigate to={target} replace /> : <ShopPlaceholder title="Not found" />;
}

export function shopRouteObjects(opts: ShopRouteOptions = {}, modules?: Record<string, () => Promise<PageModule>>): RouteObject[] {
  const fallback = opts.fallback ?? (
    <div className="page-loading" aria-busy="true">
      <Spinner size={24} />
    </div>
  );
  const routes: RouteObject[] = [];
  for (const alias of SHOP_ALIASES) {
    const element = <AliasRoute alias={alias} opts={opts} />;
    routes.push(alias.from === "" ? { index: true, element } : { path: alias.from, element });
  }
  for (const p of SHOP_PAGES) {
    const Page = shopPageComponent(p, modules);
    let element: ReactNode = (
      <Suspense fallback={fallback}>
        <Page />
      </Suspense>
    );
    if (p.id === "credits" && opts.iapMode) element = <IapGate iapMode={opts.iapMode}>{element}</IapGate>;
    routes.push({ path: p.path, element });
  }
  return routes;
}

function IapGate({ iapMode, children }: { iapMode: () => boolean; children: ReactNode }) {
  return iapMode() ? <Navigate to="/shop/overview" replace /> : <>{children}</>;
}

export function ShopRoutes(props: ShopRouteOptions) {
  const { contracts, iapMode, fallback } = props;
  const routes = useMemo(() => shopRouteObjects({ contracts, iapMode, fallback }), [contracts, iapMode, fallback]);
  return useRoutes(routes);
}
