// from: shop@2d3bf62:app/src/components/NavBar/NavBar.tsx (the Shop subnav under the site nav: its section
//       tabs, the search box, Buy Credits, favorites, activity and the cart with its count),
//       app/src/components/NavBar/NavBar.styles.ts, app/src/components/ShopFooter/ShopFooter.tsx;
//       v2 ui3/src/marketplace/frames/MarketplaceChrome.tsx, frames/shop-tabs.ts, new-shop/NewShopTabs.tsx (superseded)
// Plus marketplace@3bf3662f:webapp/src/components/Navigation/Navigation.tsx, Navigation.css and
// components/PageLayout/PageLayout.tsx (the announcement strip, the slim footer) for the pages only
// upstream Marketplace has.

import { useEffect, useMemo, useRef, type ReactNode } from "react";
import { Link, useLocation } from "react-router";

import ChromeShell from "../../../components/ChromeShell";
import DclTopBar from "../../../web/frames/DclTopBar";
import { useCart } from "../../cart/store";
import { SHOP_BASE } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { LAND_NETWORKS } from "../../land/contracts";
import SearchDropdown from "../components/SearchDropdown";
import Icon from "../components/Icon";
import { isIapMode } from "../flags";
import { useCampaign } from "../hooks";
import { t } from "../intl";
import { localized, toContentfulLocale } from "../lib/contentful";
import { MarketAnnouncementBar, MarketFooter } from "./MarketChrome";
import ShopFooter from "./ShopFooter";
import "./shoptheme.css";
import "./shopchrome.css";

export type ShopSection = "overview" | "event" | "collectibles" | "names" | "lands" | "my-items" | "activity";

export type ShopNavSet = "shop" | "marketplace";

type Tab = { id: ShopSection; label: string; href: string; mobileOnly?: boolean };

const COLLECTIBLES = /^\/shop\/(items|item|token|collection|names)(\/|$)/;
const TOKEN_ACTION = /^\/shop\/(token|item)\/[^/]+\/[^/]+\/(transfer|bid|bids|sell|cancel|manage|rent)\/?$/;
const TOKEN = /^\/shop\/token\/([^/]+)\/[^/]+(\/|$)/;
const MARKETPLACE_ONLY = /^\/shop\/(lands|rankings|settings|accounts?)(\/|$)/;

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

export function shopTabs({ iap, eventLabel, set = "shop" }: { iap: boolean; eventLabel: string | null; set?: ShopNavSet }): Tab[] {
  const tabs: Tab[] = [];
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

function ShopSubnav({ tabs, active, iap, signedIn, set }: { tabs: Tab[]; active: ShopSection | undefined; iap: boolean; signedIn: boolean; set: ShopNavSet }) {
  const count = useCart((s) => s.lines.reduce((n, l) => n + l.quantity, 0));
  const { pathname } = useLocation();
  const hidesSearch = /^\/shop\/my-items(\/|$)/.test(pathname);
  const favActive = /^\/shop\/my-favorites(\/|$)/.test(pathname);
  const inCart = /^\/shop\/cart(\/|$)/.test(pathname);
  const market = set === "marketplace";
  const creditsCta = !iap && !market && !isLandPath(pathname);
  const tabsRef = useRef<HTMLElement>(null);
  useEffect(() => {
    const nav = tabsRef.current;
    const current = nav?.querySelector<HTMLElement>('[aria-current="page"]');
    if (!nav || !current || nav.scrollWidth <= nav.clientWidth) return;
    const left = current.offsetLeft - nav.offsetLeft;
    if (left < nav.scrollLeft || left + current.offsetWidth > nav.scrollLeft + nav.clientWidth) {
      nav.scrollLeft = Math.max(0, left - (nav.clientWidth - current.offsetWidth) / 2);
    }
  }, [active, tabs]);
  return (
    <div className="shop-subnav" data-testid="shop-tools" data-iap={iap || undefined} data-set={set}>
      <nav ref={tabsRef} className="shop-subnav__tabs" aria-label={t("nav.sections")} data-testid="subnav-tabs">
        {tabs.map((tab) => (
          <Link
            key={tab.id}
            to={tab.href}
            className={tab.id === active ? "active" : undefined}
            aria-current={tab.id === active ? "page" : undefined}
            data-event={tab.id === "event" || undefined}
            data-mobile-only={tab.mobileOnly || undefined}
          >
            {tab.label}
          </Link>
        ))}
        {inCart ? (
          <Link key="cart" className="active" aria-current="page" to={`${SHOP_BASE}/cart`}>
            {t("nav.cart")}
          </Link>
        ) : null}
      </nav>
      <hr className="shop-subnav__divider" aria-hidden="true" />
      {hidesSearch || market ? null : <SearchDropdown />}
      {!creditsCta ? null : (
        <Link className="shop-subnav__credits shop-buy-ring" to={`${SHOP_BASE}/credits`} data-testid="nav-credits">
          <Icon name="credits" size={20} />
          {t("nav.getCredits")}
        </Link>
      )}
      <Link className="shop-subnav__icon" data-kind="fav" data-active={favActive || undefined} to={`${SHOP_BASE}/my-favorites`} aria-label={t("nav.myFavorites")}>
        <Icon name={favActive ? "heart-solid" : "heart"} size={28} />
      </Link>
      {signedIn && !market ? (
        <Link className="shop-subnav__icon" data-kind="activity" to={`${SHOP_BASE}/activity`} aria-label={t("nav.activity")} data-testid="nav-activity">
          <Icon name="history" size={28} />
        </Link>
      ) : null}
      {market ? null : (
        <Link
          className="shop-subnav__icon"
          data-kind="cart"
          to={`${SHOP_BASE}/cart`}
          aria-label={count > 0 ? t("nav.cartCount", { count }) : t("nav.cart")}
          data-testid="nav-cart"
        >
          <Icon name={count > 0 ? "cart-solid" : "cart"} size={28} />
          {count > 0 ? (
            <span className="shop-subnav__badge" data-testid="nav-cart-badge">
              {count}
            </span>
          ) : null}
        </Link>
      )}
    </div>
  );
}

export default function ShopChrome({ children, topbar, footer }: { children?: ReactNode; topbar?: ReactNode; footer?: ReactNode | false }) {
  const { pathname } = useLocation();
  const wallet = useShopWallet();
  const iap = isIapMode();
  const { campaign } = useCampaign();
  const eventLabel = campaign?.mainTag ? (localized(campaign.tabName, toContentfulLocale("en")) ?? campaign.name ?? null) : null;
  const set = navSetFor(pathname);
  const tabs = useMemo(() => shopTabs({ iap, eventLabel, set }), [iap, eventLabel, set]);
  const signedIn = wallet.session.kind !== "signedOut";

  return (
    <ChromeShell
      className="shop-chrome"
      ariaLabel="Shop"
      topbar={topbar ?? <DclTopBar variant="sites" active="shop" signedIn={signedIn} account={wallet.account ?? undefined} onSignIn={wallet.host?.onSignIn} />}
      subnav={false}
      right={
        <>
          {set === "marketplace" && !iap ? <MarketAnnouncementBar /> : null}
          <ShopSubnav tabs={tabs} active={sectionFor(pathname, signedIn)} iap={iap} signedIn={signedIn} set={set} />
        </>
      }
      footer={footer === undefined ? set === "marketplace" ? <MarketFooter /> : <ShopFooter /> : footer}
    >
      {children}
    </ChromeShell>
  );
}
