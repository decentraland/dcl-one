// from: shop@3a322cf9:app/src/components/NavBar/NavBar.tsx (the Shop subnav under the site nav: its section
//       tabs, the search box, Buy Credits, favorites, activity and the cart with its count; the bars deepen
//       once the page scrolls), app/src/components/NavBar/NavBar.styles.ts;
//       shop@2d3bf62:app/src/components/ShopFooter/ShopFooter.tsx;
//       v2 ui3/src/marketplace/frames/MarketplaceChrome.tsx, frames/shop-tabs.ts, new-shop/NewShopTabs.tsx (superseded)
// Plus marketplace@3bf3662f:webapp/src/components/Navigation/Navigation.tsx, Navigation.css and
// components/PageLayout/PageLayout.tsx (the announcement strip, the slim footer) for the pages only
// upstream Marketplace has.

import { useMemo, type ReactNode } from "react";
import { Link, useLocation } from "react-router";

import ShopChrome, { ShopNavigation, type ShopNavTab, type ShopNavSet, type ShopSection } from "@dcl/one-ui/apps/shop/ShopChrome";
import { shopCartDrawer } from "../../cart/drawer";
import { useCart } from "../../cart/store";
import CartDrawer from "../../checkout/components/CartDrawer";
import { SHOP_BASE } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { LAND_NETWORKS } from "@shop/land/contracts";
import SearchDropdown from "../components/SearchDropdown";
import Icon from "../components/Icon";
import { isIapMode } from "../flags";
import { useCampaign } from "../hooks";
import { t } from "@shop/discovery/intl";
import { localized, toContentfulLocale } from "../lib/contentful";
import { MarketAnnouncementBar, MarketFooter } from "./MarketChrome";
import { useMarketplace } from "../../market/access";
import { ShopHeldCredits } from "./ShopCreditsChip";
import ShopFooter from "./ShopFooter";
import "./shoptheme.css";
import "./shopchrome.css";

const COLLECTIBLES = /^\/shop\/(items|item|token|collection|names)(\/|$)/;
const TOKEN_ACTION = /^\/shop\/(token|item)\/[^/]+\/[^/]+\/(transfer|bid|bids|sell|cancel|manage|rent)\/?$/;
const TOKEN = /^\/shop\/token\/([^/]+)\/[^/]+(\/|$)/;
const MARKETPLACE_ONLY = /^\/shop\/(lands|rankings|accounts?)(\/|$)/;

export function isLandTokenPath(pathname: string): boolean {
  const c = TOKEN.exec(pathname)?.[1]?.toLowerCase();
  if (!c) return false;
  return Object.values(LAND_NETWORKS).some((n) => n.land.toLowerCase() === c || n.estate.toLowerCase() === c);
}

export function isLandPath(pathname: string): boolean {
  return /^\/shop\/lands(\/|$)/.test(pathname) || isLandTokenPath(pathname);
}

export function navSetFor(pathname: string): ShopNavSet {
  return MARKETPLACE_ONLY.test(pathname) || TOKEN_ACTION.test(pathname) || isLandTokenPath(pathname) ? "marketplace" : "shop";
}

export function sectionFor(pathname: string, signedIn = true): ShopSection | undefined {
  if (!signedIn && TOKEN_ACTION.test(pathname)) return undefined;
  if (/^\/shop\/token\/[^/]+\/[^/]+\/transfer\/?$/.test(pathname)) return "my-items";
  if (isLandPath(pathname)) return "lands";
  if (TOKEN_ACTION.test(pathname)) return undefined;
  if (/^\/shop\/rankings(\/|$)/.test(pathname)) return "overview";
  if (COLLECTIBLES.test(pathname)) return "collectibles";
  if (/^\/shop\/overview(\/|$)/.test(pathname) || pathname === SHOP_BASE) return "overview";
  if (/^\/shop\/event(\/|$)/.test(pathname)) return "event";
  if (/^\/shop\/my-items(\/|$)/.test(pathname)) return "my-items";
  if (/^\/shop\/activity(\/|$)/.test(pathname)) return "activity";
  return undefined;
}

export function shopTabs({ iap, eventLabel, set = "shop" }: { iap: boolean; eventLabel: string | null; set?: ShopNavSet }): ShopNavTab[] {
  const tabs: ShopNavTab[] = [];
  if (set === "shop" || !iap) tabs.push({ id: "overview", label: t("nav.overview"), href: `${SHOP_BASE}/overview` });
  if (eventLabel && (set === "shop" || !iap)) tabs.push({ id: "event", label: eventLabel, href: `${SHOP_BASE}/event` });
  tabs.push({ id: "collectibles", label: t("nav.collectibles"), href: `${SHOP_BASE}/items` });
  if (set === "marketplace") {
    if (!iap) {
      tabs.push(
        { id: "lands", label: t("nav.lands"), href: `${SHOP_BASE}/lands` },
        { id: "names", label: t("nav.names"), href: `${SHOP_BASE}/names` },
        { id: "my-items", label: t("nav.myAssets"), href: `${SHOP_BASE}/my-items` },
        { id: "activity", label: t("nav.activity"), href: `${SHOP_BASE}/activity`, mobileOnly: true },
      );
    }
    return tabs;
  }
  if (!iap) tabs.push({ id: "my-items", label: t("nav.myAssets"), href: `${SHOP_BASE}/my-items` });
  return tabs;
}

function ShopSubnav({ tabs, active, iap, signedIn, set }: { tabs: ShopNavTab[]; active: ShopSection | undefined; iap: boolean; signedIn: boolean; set: ShopNavSet }) {
  const count = useCart((s) => s.lines.reduce((n, l) => n + l.quantity, 0));
  const { pathname } = useLocation();
  const hidesSearch = /^\/shop\/my-items(\/|$)/.test(pathname);
  const favActive = /^\/shop\/my-favorites(\/|$)/.test(pathname);
  const market = set === "marketplace";
  const creditsCta = !iap && !market && !isLandPath(pathname);
  return (
    <ShopNavigation tabs={tabs} active={active} iap={iap} set={set} label={t("nav.sections")} Link={Link}>
      {hidesSearch || market ? null : <SearchDropdown />}
      {signedIn ? <ShopHeldCredits /> : null}
      {!creditsCta ? null : (
        <Link className="shop-subnav__credits shop-buy-ring" to={`${SHOP_BASE}/credits`}>
          <Icon name="credits" size={20} />
          {t("nav.getCredits")}
        </Link>
      )}
      <Link className="shop-subnav__icon" data-kind="fav" data-active={favActive || undefined} to={`${SHOP_BASE}/my-favorites`} aria-label={t("nav.myFavorites")}>
        <Icon name={favActive ? "heart-solid" : "heart"} size={28} />
      </Link>
      {signedIn && !market ? (
        <Link className="shop-subnav__icon" data-kind="activity" to={`${SHOP_BASE}/activity`} aria-label={t("nav.activity")} data-testid="subnav-activity">
          <Icon name="history" size={28} />
        </Link>
      ) : null}
      {market ? null : (
        // As upstream's NavBar: the cart icon opens the cart drawer over the page (no banner); the
        // drawer's Go to cart is the way to the cart page.
        <button
          type="button"
          className="shop-subnav__icon"
          data-kind="cart"
          onClick={() => shopCartDrawer.open()}
          aria-label={count > 0 ? t("nav.cartCount", { count }) : t("nav.cart")}
          aria-haspopup="dialog"
          data-testid="subnav-cart"
        >
          <Icon name={count > 0 ? "cart-solid" : "cart"} size={28} />
          {count > 0 ? (
            <span className="shop-subnav__badge" data-testid="subnav-cart-badge">
              {count}
            </span>
          ) : null}
        </button>
      )}
      {/* Mounted for every nav set: a card adds to the cart (and opens it) on a marketplace page too. */}
      <CartDrawer />
    </ShopNavigation>
  );
}

export default function ShopChromeHost({ children, topbar, footer }: { children?: ReactNode; topbar?: ReactNode; footer?: ReactNode | false }) {
  const { pathname } = useLocation();
  const wallet = useShopWallet();
  const iap = isIapMode();
  const { campaign } = useCampaign();
  const eventLabel = campaign?.mainTag ? (localized(campaign.tabName, toContentfulLocale("en")) ?? campaign.name ?? null) : null;
  // The Marketplace's tabs (LAND, ...) only for a visitor who has it (market/access.tsx): anyone else stays on the
  // Shop's, and its pages send them to the overview anyway.
  const marketplace = useMarketplace();
  const set = marketplace ? navSetFor(pathname) : "shop";
  const tabs = useMemo(() => shopTabs({ iap, eventLabel, set }), [iap, eventLabel, set]);
  const signedIn = wallet.session.kind !== "signedOut";

  return (
    <ShopChrome
      topbar={topbar}
      tools={
        <>
          {set === "marketplace" && !iap ? <MarketAnnouncementBar /> : null}
          <ShopSubnav tabs={tabs} active={sectionFor(pathname, signedIn)} iap={iap} signedIn={signedIn} set={set} />
        </>
      }
      footer={footer === undefined ? set === "marketplace" ? <MarketFooter /> : <ShopFooter /> : footer}
    >
      {children}
    </ShopChrome>
  );
}
