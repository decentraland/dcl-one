import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useLocation } from "react-router";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import {
  SitesNavbar,
  type Loadable,
  type NavMenu,
} from "@dcl/one-ui/apps/sites/chrome/SitesNavbar";
import type { ChromeLinkClick } from "@dcl/one-ui/apps/sites/chrome/follow";
import {
  GeneralSecondaryBarSettingsOutline,
  GeneralSecondaryBarWearables,
  MenusPassport,
  ShopCart,
} from "@dcl/one-ui/foundations/icons";
import {
  notificationBody,
  notificationLink,
  notificationMillis,
  notificationTitle,
  parseNotificationsLoose,
  relativeTime,
  type Notification,
} from "@ui/data/catalyst/notificationsView";
import { docsUrl } from "@ui/data/docs";
import { useChromeAuth } from "@ui/web/frames/chrome-auth";
import {
  ethereumChainId,
  settlementChainId,
} from "@ui/shop/checkout/chain/config";
import { readManaBalanceWei } from "@ui/shop/checkout/lib/mana";
import { formatMana } from "@ui/shop/checkout/lib/mana-format";
import { useBalance } from "@ui/shop/credits/hooks";

import { useAuth } from "@/data/lib/auth/index";
import { NOTIFICATIONS_PREFIX } from "@/data/lib/catalyst/bases";
import { catalystBase } from "@/data/lib/catalyst/client";
import { useLocale } from "@/features/locale/LocaleProvider";

import type { TrackContext } from "@/core/lib/telemetry/track";
import { openSignIn } from "@/features/components/auth/signin-store";
import { trackNavbarClick } from "@/features/components/chrome/chrome-telemetry";

import { installSitesShopClient } from "../shop/host";

// Source shows the newest 30; dcl-one reads 50 and keeps the same cap.
const NOTIFICATIONS_SHOWN = 30;

const loadable = <T,>(query: {
  isError: boolean;
  data: T | undefined;
}): Loadable<T> =>
  query.isError
    ? { status: "error" }
    : query.data === undefined
      ? { status: "loading" }
      : { status: "ready", value: query.data };

/** Whole credits with thousands separators, as source (1 credit = $0.10). */
const formatCredits = (credits: number) =>
  Math.floor(credits).toLocaleString("en-US");

/** Which top-level section the current page belongs to, as source's isSectionActive. */
function activeSection(pathname: string): string | null {
  if (/^\/(places|events)(\/|$)/.test(pathname)) return "discover";
  if (/^\/shop(\/|$)/.test(pathname)) return "shop";
  if (/^\/create(\/|$)/.test(pathname)) return "create";
  return null;
}

export type SitesHeaderVariant = "full" | "landing" | "compact";

/**
 * The Sites header, wired: dcl-one's own data through TanStack Query (credits from the shop API, MANA
 * on Ethereum and Polygon over RPC, notifications from the social API, the profile), auth, sign-in and
 * telemetry. Signing out also drops every cached query, so nothing of the account outlives it.
 * Only the page chrome (sites-page-chrome.tsx) renders it; the page picks the variant:
 * - `full`: the menus, then the account, or Sign In when signed out;
 * - `landing`: the signed-out home page's logo, wordmark and Sign In, then Jump In once scrolled;
 * - `compact`: the signed-out download page's logo and Sign In.
 * `session` is the page's telemetry session, for the navbar's link clicks.
 */
export default function SitesNavbarConnected({
  variant,
  overlay,
  onJumpIn,
  session,
}: {
  variant: SitesHeaderVariant;
  overlay?: boolean;
  onJumpIn?: () => void;
  session: TrackContext;
}) {
  const { t, text } = useLocale();
  // The shared chrome auth (ChromeAuthBridge): seeded from the session cookie, so the server render
  // already shows this navbar; its name and face come from the profile; it owns sign out.
  const chrome = useChromeAuth();
  const account = chrome.signedIn ? chrome.account : "";
  // Signed requests wait for the client session.
  const { isConnected, fetch: signedFetch } = useAuth();
  const live = isConnected && !!account;
  const queryClient = useQueryClient();
  const { pathname, search } = useLocation();

  // The shop client signs with this app's identity; it must be in place before the credits read.
  useState(() => installSitesShopClient());
  const credits = useBalance(live ? account : null);

  const [userOpen, setUserOpen] = useState(false);
  const mana = useQuery({
    queryKey: ["private", account.toLowerCase(), "sites-navbar", "mana"],
    enabled: live && userOpen,
    staleTime: 30_000,
    // Each chain read on its own and any failure shown as such: the shared both-chains reader turns
    // a failed chain into 0, which would claim an empty wallet.
    queryFn: async () => {
      const [ethereum, polygon] = await Promise.all([
        readManaBalanceWei(account, ethereumChainId()),
        readManaBalanceWei(account, settlementChainId()),
      ]);
      return { ethereum: formatMana(ethereum), polygon: formatMana(polygon) };
    },
  });

  const notificationsKey = [
    "private",
    account.toLowerCase(),
    "sites-navbar",
    "notifications",
  ];
  const notifications = useQuery({
    queryKey: notificationsKey,
    enabled: live,
    staleTime: 30_000,
    queryFn: async ({ signal }) => {
      const response = await signedFetch(
        `${catalystBase()}${NOTIFICATIONS_PREFIX}/notifications?limit=50`,
        {
          headers: { accept: "application/json" },
          signal,
        },
      );
      if (!response.ok)
        throw new Error(`notifications returned ${response.status}`);
      return parseNotificationsLoose(await response.json());
    },
  });

  // While the panel is open, the unread rows it SHOWS (the first NOTIFICATIONS_SHOWN) are marked read,
  // as source does, including rows of a list that arrives after it opened; rows beyond the cap stay
  // unread. The list is reloaded only after that write, so an earlier read cannot resurrect them.
  // Nothing is requested before the client session is live.
  const [panelOpen, setPanelOpen] = useState(false);
  // Rows being or already marked read, and rows whose write failed during this opening of the panel.
  // A failed write releases its rows (the reload brings them back unread) and they are retried the
  // next time the panel opens, not again while it stays open, so a failing server cannot loop.
  const marked = useRef(new Set<string>());
  const failed = useRef(new Set<string>());
  useEffect(() => {
    marked.current = new Set();
    failed.current = new Set();
  }, [account]);
  useEffect(() => {
    if (!panelOpen || !live || !notifications.data) return;
    const shownUnread = notifications.data
      .slice(0, NOTIFICATIONS_SHOWN)
      .filter(
        (row) =>
          !row.read &&
          !marked.current.has(row.id) &&
          !failed.current.has(row.id),
      )
      .map((row) => row.id);
    if (!shownUnread.length) return;
    for (const id of shownUnread) marked.current.add(id);
    queryClient.setQueryData<Notification[]>(notificationsKey, (rows) =>
      rows?.map((row) =>
        shownUnread.includes(row.id) ? { ...row, read: true } : row,
      ),
    );
    const release = () => {
      for (const id of shownUnread) {
        marked.current.delete(id);
        failed.current.add(id);
      }
    };
    void signedFetch(
      `${catalystBase()}${NOTIFICATIONS_PREFIX}/notifications/read`,
      {
        method: "PUT",
        headers: {
          "content-type": "application/json",
          accept: "application/json",
        },
        body: JSON.stringify({ notificationIds: shownUnread }),
      },
    )
      .then((response) => {
        if (!response.ok) release();
      }, release)
      .finally(
        () =>
          void queryClient.invalidateQueries({ queryKey: notificationsKey }),
      );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [panelOpen, live, notifications.data, signedFetch, queryClient, account]);

  const openNotifications = useCallback(() => {
    failed.current = new Set();
    setPanelOpen(true);
    if (live) void notifications.refetch();
  }, [live, notifications]);

  const signOut = useCallback(() => {
    void queryClient.cancelQueries();
    queryClient.clear();
    chrome.onSignOut?.();
  }, [chrome, queryClient]);

  const section = activeSection(pathname);
  const menus: NavMenu[] = useMemo(
    () => [
      {
        id: "discover",
        label: t("component.landing.navbar.discover"),
        active: section === "discover",
        items: [
          {
            id: "events",
            label: t("component.landing.navbar.events"),
            href: "/events",
          },
          {
            id: "places",
            label: t("component.landing.navbar.places"),
            href: "/places",
          },
        ],
      },
      {
        id: "shop",
        label: t("component.landing.navbar.shop"),
        active: section === "shop",
        items: [
          {
            id: "shop_all",
            label: t("component.landing.navbar.shop_all"),
            href: "/shop/items",
          },
          {
            id: "wearables",
            label: t("component.landing.navbar.wearables"),
            href: "/shop/items?category=wearable",
          },
          {
            id: "emotes",
            label: t("component.landing.navbar.emotes"),
            href: "/shop/items?category=emote",
          },
          {
            id: "names",
            label: t("component.landing.navbar.names"),
            href: "/shop/names",
          },
          {
            id: "land",
            label: t("component.landing.navbar.land"),
            href: "/shop/lands",
          },
          {
            id: "merch",
            label: t("component.landing.navbar.merch"),
            href: "https://store.decentraland.org/",
            external: true,
          },
        ],
      },
      {
        id: "create",
        label: t("component.landing.navbar.create"),
        active: section === "create",
        items: [
          {
            id: "create_in_decentraland",
            label: t("component.landing.navbar.create_in_decentraland"),
            href: "/create",
          },
          {
            id: "publish_wearables_emotes",
            label: t("component.landing.navbar.publish_wearables_emotes"),
            href: "/create/wearables",
          },
          {
            id: "publish_land",
            label: t("component.landing.navbar.publish_land"),
            href: "/create/land",
          },
          {
            id: "creator_documentation",
            label: t("component.landing.navbar.creator_documentation"),
            href: docsUrl("creator"),
          },
        ],
      },
    ],
    [t, section],
  );

  const now = Date.now();
  const rows = notifications.data?.slice(0, NOTIFICATIONS_SHOWN).map((row) => {
    const millis = notificationMillis(row.timestamp);
    return {
      id: row.id,
      title: notificationTitle(row),
      body: notificationBody(row) || undefined,
      time: millis === null ? "" : relativeTime(millis, now),
      href: notificationLink(row) ?? undefined,
      unread: !row.read,
    };
  });

  const signIn = {
    label: t("component.landing.navbar.sign_in"),
    onClick: () => {
      trackNavbarClick({ action: "sign_in" }, session);
      openSignIn({ redirectTo: `${pathname}${search}` });
    },
  };
  const home = {
    href: "/",
    label: text("component.landing.navbar.home_label"),
  };
  const onLinkClick = (link: ChromeLinkClick) =>
    trackNavbarClick(link, session);

  if (variant === "compact") {
    // As source's download header: neither its logo nor its Sign In reports a click.
    return (
      <SitesNavbar
        variant="compact"
        home={home}
        signIn={{
          label: signIn.label,
          onClick: () => openSignIn({ redirectTo: `${pathname}${search}` }),
        }}
      />
    );
  }
  if (variant === "landing") {
    return (
      <SitesNavbar
        variant="landing"
        home={home}
        signIn={signIn}
        jumpIn={
          onJumpIn
            ? {
                label: t("component.landing.navbar.jump_in"),
                onClick: () => {
                  trackNavbarClick({ action: "jump_in" }, session);
                  onJumpIn();
                },
              }
            : undefined
        }
        onLinkClick={onLinkClick}
      />
    );
  }
  const shared = {
    overlay,
    home,
    menus,
    learn: {
      id: "learn",
      label: t("component.landing.navbar.learn"),
      href: "/blog",
    },
    labels: {
      openMenu: text("component.landing.navbar.open_menu"),
      closeMenu: text("component.landing.navbar.close_menu"),
      notifications: text("component.landing.navbar.notifications"),
      userMenu: text("component.landing.navbar.user_menu"),
      copyAddress: text("component.landing.navbar.copy_address"),
      addressCopied: t("component.landing.navbar.address_copied"),
      walletAddress: t("component.landing.navbar.wallet_address"),
      logout: t("component.landing.navbar.logout"),
      switchAccount: t("component.landing.navbar.switch_account"),
      loading: t("component.landing.navbar.notifications_loading"),
      notificationsTitle: t("component.landing.navbar.notifications_title"),
      notificationsEmpty: t("component.landing.navbar.notifications_empty_new"),
      notificationsError: t("component.landing.navbar.notifications_error"),
      manaError: t("component.landing.navbar.mana_error"),
      unread: t("component.landing.navbar.notification_unread"),
    },
    onLinkClick,
  };
  if (!account) {
    return <SitesNavbar {...shared} signIn={signIn} />;
  }
  return (
    <SitesNavbar
      {...shared}
      credits={{
        href: "/shop/credits",
        state: loadable({
          isError: credits.isError,
          data: credits.data ? formatCredits(credits.data.credits) : undefined,
        }),
        label: (count) =>
          text("component.landing.navbar.credits_balance", { count }),
      }}
      notifications={{
        unread: (notifications.data ?? []).filter((row) => !row.read).length,
        state: loadable({ isError: notifications.isError, data: rows }),
        onOpen: openNotifications,
        onClose: () => setPanelOpen(false),
      }}
      user={{
        name: chrome.name || `${account.slice(0, 6)}...${account.slice(-4)}`,
        address: account,
        ...(chrome.avatarUrl ? { avatarUrl: chrome.avatarUrl } : {}),
        links: [
          {
            id: "view_profile",
            label: t("component.landing.navbar.view_profile"),
            href: `/profile/${account}`,
            icon: MenusPassport,
          },
          {
            id: "my_assets",
            label: t("component.landing.navbar.my_assets"),
            href: "/shop/my-items",
            icon: GeneralSecondaryBarWearables,
          },
          {
            id: "account_settings",
            label: t("component.landing.navbar.account_settings"),
            href: "/shop/settings",
            icon: GeneralSecondaryBarSettingsOutline,
          },
          {
            id: "marketplace_authorizations",
            label: t("component.landing.navbar.marketplace_authorizations"),
            href: "/shop/authorizations",
            icon: ShopCart,
          },
        ],
        mana: loadable(mana),
        onOpen: () => setUserOpen(true),
        // Framed by the Explorer, the host owns the session: offer switching account instead.
        ...(chrome.onSwitchAccount
          ? { onSwitchAccount: chrome.onSwitchAccount }
          : { onSignOut: signOut }),
      }}
    />
  );
}
