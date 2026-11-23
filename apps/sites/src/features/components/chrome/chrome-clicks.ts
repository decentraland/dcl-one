import { useMemo, type MouseEvent } from "react";
import { useLocation } from "react-router";

import { isOutsideLegacySites, useAnalyticsSid, useAnalyticsUserId } from "@/core/lib/telemetry/page-view";
import { sendLegacyDeferred, useLegacyTrackScope, type LegacySession } from "@/core/lib/telemetry/legacy-click";
import { legacyAnalyticsReady, track, type TrackContext } from "@/core/lib/telemetry/track";

// The legacy site's navbar and footer `Click` events (decentraland/sites `LandingNavbar.tsx` `trackNavbar` and
// `LandingFooter.tsx` `trackFooter`). Warehouse models group these rows by `place`, `action`, `section`,
// `platform` and `link`, so every name and value below is the legacy one, byte for byte.

export type NavbarSection = "home" | "discover" | "shop" | "create" | "learn" | "credits" | "user_menu";

export type NavbarAction =
  | "logo"
  | "sign_in"
  | "events"
  | "places"
  | "shop_all"
  | "wearables"
  | "emotes"
  | "names"
  | "land"
  | "merch"
  | "create_in_decentraland"
  | "publish_wearables_emotes"
  | "publish_land"
  | "learn"
  | "credits"
  | "view_profile"
  | "my_assets"
  | "account_settings"
  | "marketplace_authorizations";

/** A navbar link's legacy `section` and `href`: the legacy menu's entry, not this site's destination. */
export type NavbarLink = { section: NavbarSection; href: string };

export type NavbarClick = { action: NavbarAction; link?: NavbarLink };

const LEGACY_PROFILE: NavbarClick = {
  action: "view_profile",
  link: { section: "user_menu", href: "https://decentraland.org/profile" },
};

// This site's navbar destinations (`decentralandPublicNavSections`, SitesTopBar's account actions and credits
// chip) mapped to the legacy entry the same control was (`navbarConfig.ts`, `LandingNavbar.tsx`). A destination
// with no legacy entry is absent and sends nothing.
const NAVBAR_LINKS: Readonly<Record<string, NavbarClick>> = {
  "/": { action: "logo", link: { section: "home", href: "/" } },
  "/events": { action: "events", link: { section: "discover", href: "/events" } },
  "/places": { action: "places", link: { section: "discover", href: "/places" } },
  "/shop/overview": { action: "shop_all", link: { section: "shop", href: "https://decentraland.org/shop" } },
  "/shop/items?category=wearable": {
    action: "wearables",
    link: { section: "shop", href: "https://decentraland.org/shop/items?category=wearable" },
  },
  "/shop/items?category=emote": {
    action: "emotes",
    link: { section: "shop", href: "https://decentraland.org/shop/items?category=emote" },
  },
  "/shop/names": { action: "names", link: { section: "shop", href: "https://decentraland.org/shop/items?category=names" } },
  "/shop/lands": { action: "land", link: { section: "shop", href: "https://decentraland.org/marketplace/lands" } },
  "https://store.decentraland.org/": { action: "merch", link: { section: "shop", href: "https://store.decentraland.org/" } },
  "/create": {
    action: "create_in_decentraland",
    link: { section: "create", href: "https://decentraland.org/create/" },
  },
  "/create/collections": {
    action: "publish_wearables_emotes",
    link: { section: "create", href: "https://decentraland.org/create/collections" },
  },
  "/download/creator-hub": {
    action: "publish_land",
    link: { section: "create", href: "https://decentraland.org/builder/land" },
  },
  "/blog": { action: "learn", link: { section: "learn", href: "https://decentraland.org/blog/" } },
  "/shop/credits": { action: "credits", link: { section: "credits", href: "/shop/credits" } },
};

const ACCOUNT_LINKS: Readonly<Record<string, NavbarClick>> = {
  "/shop/my-items": {
    action: "my_assets",
    link: { section: "user_menu", href: "https://decentraland.org/marketplace/account" },
  },
  "/account": {
    action: "account_settings",
    link: { section: "user_menu", href: "https://decentraland.org/account" },
  },
  "/shop/settings": {
    action: "marketplace_authorizations",
    link: { section: "user_menu", href: "https://decentraland.org/marketplace/settings" },
  },
};

export type NavbarLinkContext = {
  /** Signed in: the account menu is what links to the account pages. */
  signedIn: boolean;
  /** The account menu's own profile link. */
  profileHref?: string;
  /** Signed out: Sign in rendered as a link to this href. */
  signInHref?: string;
};

/** The legacy click a navbar link stands for, or null for a link the legacy navbar did not have. */
export function navbarLinkClick(href: string, { signedIn, profileHref, signInHref }: NavbarLinkContext): NavbarClick | null {
  if (!signedIn && signInHref !== undefined && href === signInHref) return { action: "sign_in" };
  if (signedIn && profileHref !== undefined && href === profileHref) return LEGACY_PROFILE;
  if (signedIn && Object.hasOwn(ACCOUNT_LINKS, href)) return ACCOUNT_LINKS[href]!;
  return Object.hasOwn(NAVBAR_LINKS, href) ? NAVBAR_LINKS[href]! : null;
}

// Targets that load the link in this browsing context or one that contains it, so the page is replaced.
const SAME_CONTEXT_TARGETS: ReadonlyArray<string | null> = [null, "", "_self", "_parent", "_top"];

/**
 * The legacy `isSameTabNavigation`: whether a primary click on a link replaces the page. A link that opens
 * elsewhere (`_blank` or a named target, or ctrl/cmd/shift held) or downloads its file (a `download` attribute,
 * or alt/option held) leaves the page alive.
 */
export function isSameTabNavigation(
  event: Pick<MouseEvent<HTMLAnchorElement>, "currentTarget" | "ctrlKey" | "metaKey" | "shiftKey" | "altKey">,
): boolean {
  const target = event.currentTarget.getAttribute("target")?.toLowerCase() ?? null;
  return (
    SAME_CONTEXT_TARGETS.includes(target) &&
    !event.currentTarget.hasAttribute("download") &&
    !event.ctrlKey &&
    !event.metaKey &&
    !event.shiftKey &&
    !event.altKey
  );
}

/**
 * The legacy `trackNavbar`. Once the document's analytics is ready the click is sent at once with
 * `track_deferred: false`. Before that, a click that replaces the page is sent at once with `track_deferred: true`
 * (the legacy beacon), and one that keeps the page (a new tab, a middle click) waits for readiness (the legacy
 * queue). Unlike the other legacy clicks it carries no `utm_*`. A bot or an exempt session sends nothing.
 */
export function trackNavbarClick(
  { action, link }: NavbarClick,
  session: LegacySession,
  { leavesPage = true }: { leavesPage?: boolean } = {},
): void {
  try {
    sendLegacyDeferred(
      (audit, ctx) => track("Click", { place: "Landing Navbar", event: "click", action, ...link, ...audit }, ctx),
      session,
      // Only a cold click that replaces the page took the legacy beacon, stamped at its call; once ready, the
      // legacy queue sent it at once with its own delivery time.
      { immediate: leavesPage && !legacyAnalyticsReady() },
    );
  } catch {
    // Telemetry never breaks navigation.
  }
}

/**
 * The legacy footer's `trackFooter` called analytics directly, guarded by `isInitialized`: a click before analytics
 * is ready is dropped, and none carries the deferral fields.
 */
function sendFooterClick(send: (ctx: TrackContext) => void, session: LegacySession): void {
  try {
    if (!legacyAnalyticsReady()) return;
    sendLegacyDeferred((_audit, ctx) => send(ctx), session);
  } catch {
    // Telemetry never breaks navigation.
  }
}

/** The legacy footer's social icon click. */
export function trackFooterSocialClick(platform: string, session: LegacySession): void {
  sendFooterClick((ctx) => track("Click", { place: "Landing Footer Social", event: "click", platform }, ctx), session);
}

/** The legacy footer's link click, with the link's label in the shown language. */
export function trackFooterLinkClick(link: string, session: LegacySession): void {
  sendFooterClick((ctx) => track("Click", { place: "Landing Footer Link", event: "click", link }, ctx), session);
}

export type ChromeClicks = {
  /** A navbar link's `click` or `auxclick`, sent as its legacy entry when it has one. */
  navbarLink: (href: string, event: MouseEvent<HTMLAnchorElement>, context: NavbarLinkContext) => void;
  /** The legacy Sign in button. */
  signIn: () => void;
  footerSocial: (platform: string) => void;
  footerLink: (label: string) => void;
};

/**
 * The navbar and footer clicks for the page being viewed. Pages the legacy site never served (the Shop, the docs,
 * see `isOutsideLegacySites`) send none, as their own navbars sent none.
 */
export function useChromeClicks(): ChromeClicks {
  const sid = useAnalyticsSid();
  const userId = useAnalyticsUserId();
  const scope = useLegacyTrackScope();
  const { pathname } = useLocation();
  const off = isOutsideLegacySites(pathname);
  return useMemo(() => {
    const session = { sid, userId, scope };
    return {
      navbarLink: (href, event, context) => {
        if (off) return;
        const click = navbarLinkClick(href, context);
        if (!click) return;
        // A middle click opens a new tab and fires `auxclick`, not `click`; the primary-button check keeps an older
        // browser that fires `click` for it too to one event.
        if (event.type === "auxclick") {
          if (event.button === 1) trackNavbarClick(click, session, { leavesPage: false });
          return;
        }
        if (event.button === 0) trackNavbarClick(click, session, { leavesPage: isSameTabNavigation(event) });
      },
      signIn: () => {
        if (!off) trackNavbarClick({ action: "sign_in" }, session);
      },
      footerSocial: (platform) => {
        if (!off) trackFooterSocialClick(platform, session);
      },
      footerLink: (label) => {
        if (!off) trackFooterLinkClick(label, session);
      },
    };
  }, [off, scope, sid, userId]);
}
