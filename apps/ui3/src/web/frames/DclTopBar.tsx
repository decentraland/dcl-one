import {
  sitesNavigationLinks,
  sitesNavigationOrder,
  type SitesNavigationId,
  type SitesNavigationLink,
} from "../../../../../library/ui/apps/sites/chrome/navigation";
import type { ReactNode } from "react";
import { useEffect, useId, useRef, useState } from "react";
import { NavMenuItem, NavMenuTab, NavNotificationsButton, NavProfileButton } from "@dcl/one-ui/components";
import logo from "../../atoms/assets/decentraland-logo.svg?url";
import wordmark from "../../atoms/assets/decentraland-wordmark.svg?url";
import { catalystBase, getJSON } from "../../data/catalyst/client";
import { sameOriginContentUrl } from "../../data/catalyst/profile";
import {
  notificationBody,
  notificationLink,
  notificationTitle,
  parseNotificationsLoose,
  relativeTime,
  type Notification,
} from "../../data/catalyst/notificationsView";
import { useChromeAuth } from "./chrome-auth";
import { useChromeNav, type ChromeNavigate } from "./chrome-nav";
import ChromeLink from "./ChromeLink";
import AccountMenu from "./AccountMenu";
import { docsUrl } from "../../data/docs";
import "../../atoms/maskicon.css";
import "./dcltopbar.css";
import { useSiteLayout } from "./SiteLayout";
import { avatarBackground, displayName } from "../places/avatarColor";

type DclTopBarVariant = "default" | "sites";

type NavLink = SitesNavigationLink;
const LINK_DEFS = sitesNavigationLinks(docsUrl());
export type DclTopBarNavId = SitesNavigationId;

// from: decentraland-ui2@3.23.8:dist/components/Navbar/icons.js (BellIcon): the filled bell of DCL's navbar.
const BellMark = ({ size = 24 }: { size?: number }) => (
  <svg viewBox="0 0 24 24" width={size} height={size} fill="none" aria-hidden="true">
    <path
      d="M12 21.75C13.1 21.75 14 20.85 14 19.75H10C10 20.85 10.89 21.75 12 21.75ZM18 15.75V10.75C18 7.68 16.36 5.11 13.5 4.43V3.75C13.5 2.92 12.83 2.25 12 2.25C11.17 2.25 10.5 2.92 10.5 3.75V4.43C7.63 5.11 6 7.67 6 10.75V15.75L4 17.75V18.75H20V17.75L18 15.75Z"
      fill="currentColor"
    />
  </svg>
);

const DCL_LINKS: NavLink[] = sitesNavigationOrder
  .map((id) => LINK_DEFS[id]);

type NotifState =
  | { phase: "idle" | "loading" }
  | { phase: "error" }
  | { phase: "ready"; rows: Notification[] };

export type DclTopBarProps = {
  variant?: DclTopBarVariant;
  active?: DclTopBarNavId | "";
  signedIn?: boolean;
  account?: string;
  transparent?: boolean;
  /** As decentraland.org's landing nav: transparent on desktop, and only the logo, wordmark and Sign in while signed out. */
  minimal?: boolean;
  onSignIn?: () => void;
  onSignOut?: () => void;
  signInHref?: string;
  fetchNotifications?: (signal?: AbortSignal) => Promise<unknown>;
  onNavigate?: ChromeNavigate;
  mobileNavigation?: (closeMenu: () => void) => ReactNode;
  accountControl?: ReactNode;
  /** Balance chips for a signed-in account, before the bell (the Shop's credits). */
  balances?: ReactNode;
};

const accountLinks = (account: string) => [
  { label: "My profile", href: `/profile/${encodeURIComponent(account.toLowerCase())}` },
  { label: "My assets", href: "/shop/my-items" },
];

function LegacyDclTopBar({
  variant = "default",
  active = "shop",
  signedIn = undefined,
  account = undefined,
  transparent = false,
  minimal = false,
  onSignIn = undefined,
  onSignOut = undefined,
  signInHref = "/shop/my-items",
  fetchNotifications = undefined,
  onNavigate = undefined,
  mobileNavigation,
  accountControl,
  balances,
}: DclTopBarProps) {
  const layout = useSiteLayout();
  const renderMobileNavigation = mobileNavigation ?? layout?.sections;
  const auth = useChromeAuth();
  const isIn = signedIn ?? auth.signedIn;
  const acct = account ?? auth.account;
  const doSignIn = onSignIn ?? auth.onSignIn;
  const doSignOut = onSignOut ?? auth.onSignOut;
  const switchAccount =
    auth.onSwitchAccount ??
    (doSignOut && doSignIn
      ? () => {
          doSignOut();
          doSignIn();
        }
      : undefined);
  const fetchNotifs = fetchNotifications ?? auth.fetchNotifications;
  const links = DCL_LINKS;
  const activeId = active === "explore" || active === "events" ? "whatson" : active;
  const [menuOpen, setMenuOpen] = useState(false);
  const mobileMenuId = useId();
  const burgerRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!menuOpen) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setMenuOpen(false);
        burgerRef.current?.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [menuOpen]);
  const [openDropdown, setOpenDropdown] = useState("");
  const ddEscRef = useRef(false);

  const [faceUrl, setFaceUrl] = useState("");
  const [fetchedColor, setFetchedColor] = useState("");
  const sharedProfile = isIn && auth.signedIn && acct.toLowerCase() === auth.account.toLowerCase();
  const sharedFaceUrl = sharedProfile ? auth.avatarUrl : undefined;
  // As the reference's LandingNavbar: the disc takes the display name's colour once the profile loads.
  const avatarColor = sharedProfile ? auth.avatarColor : fetchedColor || undefined;
  useEffect(() => {
    setFaceUrl("");
    setFetchedColor("");
    if (!isIn || !acct) return;
    if (sharedProfile) {
      setFaceUrl(sharedFaceUrl ?? "");
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const addr = acct.trim().toLowerCase();
        const raw = await getJSON(`/lambdas/profile/${encodeURIComponent(addr)}`);
        const profile = (raw as { avatars?: Array<{ name?: string; hasClaimedName?: boolean; ethAddress?: string; avatar?: { snapshots?: { face256?: unknown } } }> })
          ?.avatars?.[0];
        if (!cancelled && profile) setFetchedColor(avatarBackground(displayName(profile.name, Boolean(profile.hasClaimedName), profile.ethAddress ?? addr)));
        const face = profile?.avatar?.snapshots?.face256;
        if (cancelled || typeof face !== "string" || !face) return;
        const url =
          /^https?:\/\//i.test(face)
            ? sameOriginContentUrl(face)
            : face.startsWith("data:")
              ? face
              : `${catalystBase()}/content/contents/${face}`;
        setFaceUrl(url);
      } catch {
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [isIn, acct, sharedProfile, sharedFaceUrl]);

  const [bellOpen, setBellOpen] = useState(false);
  const [notif, setNotif] = useState<NotifState>({ phase: "idle" });
  const bellRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setBellOpen(false);
    setNotif({ phase: "idle" });
  }, [isIn, acct]);

  useEffect(() => {
    if (!isIn || !bellOpen) return;
    let cancelled = false;
    if (!fetchNotifs) {
      setNotif({ phase: "error" });
      return;
    }
    setNotif({ phase: "loading" });
    void fetchNotifs()
      .then((raw) => {
        if (!cancelled) setNotif({ phase: "ready", rows: parseNotificationsLoose(raw) });
      })
      .catch(() => {
        if (!cancelled) setNotif({ phase: "error" });
      });
    return () => { cancelled = true; };
  }, [isIn, acct, bellOpen, fetchNotifs]);

  useEffect(() => {
    if (!bellOpen) return;
    const onDown = (e: MouseEvent) => {
      if (bellRef.current && !bellRef.current.contains(e.target as Node)) setBellOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setBellOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [bellOpen]);

  const openBell = () => {
    if (isIn) setBellOpen((open) => !open);
  };

  const now = Date.now();
  const bare = minimal && !isIn;
  // The sites bar draws its tab, menu item, bell and avatar with the shared library; the shop and account bars keep
  // their own look. `dtb__link` stays on the sites tab as the hook the parity harness and the Escape handler use.
  const sites = variant === "sites";

  const renderTab = (link: NavLink, menu?: { open: boolean }) => {
    const isActive = link.id === activeId;
    const className = "dtb__link" + (isActive ? " is-active" : "");
    if (sites) {
      return (
        <NavMenuTab
          as={ChromeLink} key={link.id} href={link.href} onNavigate={onNavigate} className={className}
          label={link.label} active={isActive} hasMenu={Boolean(menu)} expanded={menu?.open}
        />
      );
    }
    return (
      <ChromeLink
        key={link.id}
        href={link.href} onNavigate={onNavigate}
        className={className}
        aria-current={isActive ? "page" : undefined}
        aria-haspopup={menu ? "menu" : undefined}
        aria-expanded={menu ? menu.open : undefined}
      >
        {link.label}
      </ChromeLink>
    );
  };

  const renderNavLink = (link: NavLink) => {
    if (!link.menu?.length) return renderTab(link);
    const open = openDropdown === link.id;
    return (
      <div
        key={link.id}
        className="dtb__dd"
        onMouseEnter={() => setOpenDropdown(link.id)}
        onMouseLeave={() => setOpenDropdown((cur) => (cur === link.id ? "" : cur))}
        onFocus={() => {
          if (!ddEscRef.current) setOpenDropdown(link.id);
        }}
        onBlur={(e) => {
          if (!e.currentTarget.contains(e.relatedTarget as Node)) {
            setOpenDropdown((cur) => (cur === link.id ? "" : cur));
          }
        }}
        onKeyDown={(e) => {
          if (e.key !== "Escape") return;
          setOpenDropdown((cur) => (cur === link.id ? "" : cur));
          ddEscRef.current = true;
          e.currentTarget.querySelector<HTMLAnchorElement>("a.dtb__link")?.focus();
          ddEscRef.current = false;
        }}
      >
        {renderTab(link, { open })}
        {open ? (
          <div className="dtb__ddmenu is-open" role="menu" aria-label={`${link.label} menu`}>
            {link.menu.map((item) => sites ? (
              <NavMenuItem as={ChromeLink} key={item.label} href={item.href} onNavigate={onNavigate} label={item.label} />
            ) : (
              <ChromeLink key={item.label} href={item.href} onNavigate={onNavigate} className="dtb__dditem" role="menuitem">
                {item.label}
              </ChromeLink>
            ))}
          </div>
        ) : null}
      </div>
    );
  };

  return (
    <header
      className={"dtb" + (sites ? " dtb--sites" : "") + (transparent && !layout ? " dtb--transparent" : "") + (minimal ? " dtb--minimal" : "") + (bare ? " dtb--bare" : "")}
      role="banner" aria-label="Decentraland"
    >
      <ChromeLink className="dtb__brand" href="/" onNavigate={onNavigate} aria-label="Decentraland home">
        <img src={logo} width={40} height={40} alt="" />
      </ChromeLink>
      {bare ? <img className="dtb__wordmark" src={wordmark} width={160} height={20} alt="Decentraland" /> : null}

      {bare ? null : <button
        type="button"
        className="dtb__burger"
        ref={burgerRef}
        aria-controls={mobileMenuId}
        aria-label="Toggle navigation menu"
        aria-expanded={menuOpen}
        onClick={() => setMenuOpen((o) => !o)}
      />}

      {bare ? null : <nav className="dtb__links" aria-label="Decentraland sections">
        {links.map(renderNavLink)}
      </nav>}

      <div className="dtb__right">
        {isIn ? balances : null}
        {isIn ? (
          <div className="dtb__bellwrap" ref={bellRef}>
            {sites ? <NavNotificationsButton state={bellOpen ? "open" : "closed"} onClick={openBell} /> : (
              <button
                type="button"
                className="dtb__bell"
                aria-label="Notifications"
                aria-haspopup="dialog"
                aria-expanded={bellOpen}
                onClick={openBell}
              >
                <BellMark />
              </button>
            )}
            {bellOpen && (
              <div className="dtb__notif" role="dialog" aria-label="Notifications">
                <div className="dtb__notifhead">Notifications</div>
                {notif.phase === "loading" && <div className="dtb__notifmsg">Loading&#x2026;</div>}
                {notif.phase === "error" && (
                  <div className="dtb__notifmsg">Couldn&rsquo;t load notifications.</div>
                )}
                {notif.phase === "ready" && notif.rows.length === 0 && (
                  <div className="dtb__notifmsg">No notifications yet</div>
                )}
                {notif.phase === "ready" && notif.rows.length > 0 && (
                  <ul className="dtb__notiflist">
                    {notif.rows.slice(0, 10).map((n) => {
                      const link = notificationLink(n);
                      const body = (
                        <>
                          <span className="dtb__notifdot" aria-hidden="true" />
                          <span className="dtb__notifbody">
                            <span className="dtb__notiftitle">{notificationTitle(n)}</span>
                            {notificationBody(n) && (
                              <span className="dtb__notifdesc">{notificationBody(n)}</span>
                            )}
                          </span>
                          <span className="dtb__notiftime">{relativeTime(n.timestamp, now)}</span>
                        </>
                      );
                      return (
                        <li key={n.id} className={"dtb__notifitem" + (n.read ? "" : " is-unread")}>
                          {link ? (
                            <ChromeLink className="dtb__notiflink" href={link} onNavigate={onNavigate}>
                              {body}
                            </ChromeLink>
                          ) : (
                            body
                          )}
                        </li>
                      );
                    })}
                  </ul>
                )}
              </div>
            )}
          </div>
        ) : null}
        {accountControl ?? (isIn ? (
          <AccountMenu
            triggerClassName="dtb__usermenu"
            triggerLabel="My account"
            name={auth.name}
            account={acct}
            links={accountLinks(acct)}
            onSwitchAccount={switchAccount}
            onSignOut={doSignOut}
            onNavigate={onNavigate}
            align="right"
          >
            {sites ? (
              <NavProfileButton
                imageUrl={faceUrl || undefined} color={avatarColor} fallbackSrc={logo} onImageError={() => setFaceUrl("")}
              />
            ) : <span className="dtb__avatar u-avatar" style={avatarColor ? { backgroundColor: avatarColor } : undefined}>
              {faceUrl ? (
                <img
                  className="u-avatar__img"
                  src={faceUrl}
                  alt=""
                  width="40"
                  height="40"
                  loading="lazy"
                  onError={() => setFaceUrl("")}
                />
              ) : (
                // As ui2's navbar: an account without a face picture shows the Decentraland mark on its disc.
                <img className="dtb__avatar-logo" src={logo} alt="" width="40" height="40" data-testid="topbar-avatar-logo" />
              )}
            </span>}
          </AccountMenu>
        ) : doSignIn ? (
          <button type="button" className="dtb__signin" onClick={() => doSignIn()}>Sign in</button>
        ) : (
          <ChromeLink className="dtb__signin" href={signInHref} onNavigate={onNavigate} style={{ textDecoration: "none" }}>Sign in</ChromeLink>
        ))}
      </div>

      <div id={mobileMenuId} className={"dtb__menu" + (menuOpen ? " is-open" : "") + (renderMobileNavigation ? " dtb__menu--context" : "")}>
        {menuOpen ? (
          <>
            <nav className="dtb__globalmenu" aria-label="Decentraland sections">
              {links.map((link) => {
                const top = (
                  <ChromeLink
                    key={link.id}
                    href={link.href} onNavigate={onNavigate}
                    className={"dtb__menulink" + (link.id === activeId ? " is-active" : "") + (link.caret ? " dtb__menulink--caret" : "")}
                    aria-current={link.id === activeId ? "page" : undefined}
                    onClick={() => setMenuOpen(false)}
                  >
                    {link.label}
                  </ChromeLink>
                );
                return link.menu?.length ? (
                  <span key={link.id} className="dtb__menugroup">
                    {top}
                    {link.menu.map((item) => (
                      <ChromeLink
                        key={item.label}
                        href={item.href} onNavigate={onNavigate}
                        className="dtb__menulink dtb__menulink--sub"
                        onClick={() => setMenuOpen(false)}
                      >
                        {item.label}
                      </ChromeLink>
                    ))}
                  </span>
                ) : top;
              })}
            </nav>
            {renderMobileNavigation ? <div className="site-layout__mobile-sections">{renderMobileNavigation(() => setMenuOpen(false))}</div> : null}
          </>
        ) : null}
      </div>
    </header>
  );
}

export default function DclTopBar(props: DclTopBarProps) {
  const { TopBar } = useChromeNav();
  return TopBar && TopBar !== DclTopBar ? <TopBar {...props} /> : <LegacyDclTopBar {...props} />;
}
