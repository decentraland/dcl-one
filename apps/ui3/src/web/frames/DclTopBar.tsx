import type { CSSProperties, ReactNode } from "react";
import { useEffect, useId, useRef, useState } from "react";
import logo from "../../atoms/assets/decentraland-logo.svg?url";
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
import type { ChromeNavigate } from "./chrome-nav";
import ChromeLink from "./ChromeLink";
import AccountMenu from "./AccountMenu";
import { docsUrl } from "../../data/docs";
import "../../atoms/maskicon.css";
import "./dcltopbar.css";
import { useSiteLayout } from "./SiteLayout";

type DclTopBarVariant = "default" | "sites";

type NavMenuItem = { label: string; href: string };
type NavLink = { id: string; label: string; href: string; caret?: boolean; menu?: NavMenuItem[] };

const LEARN_MENU: NavMenuItem[] = [
  { label: "Docs", href: docsUrl() },
  { label: "Blog", href: "/blog" },
];

const PLAY_MENU: NavMenuItem[] = [
  { label: "Jump in", href: "/play/" },
  { label: "What's on", href: "/whats-on" },
];

const LINK_DEFS = {
  explore: { id: "explore", label: "Explore", href: "/" },
  whatson: { id: "whatson", label: "Play", href: "/play/", caret: true, menu: PLAY_MENU },
  social: { id: "social", label: "Social", href: "/social/" },
  shop: { id: "shop", label: "Shop", href: "/shop" },
  create: { id: "create", label: "Create", href: "/create" },
  learn: { id: "learn", label: "Learn", href: "/blog", caret: true, menu: LEARN_MENU },
  events: { id: "events", label: "Events", href: "/whats-on" },
} satisfies Record<string, NavLink>;

export type DclTopBarNavId = keyof typeof LINK_DEFS;

const BellMark = ({ size = 19 }: { size?: number }) => (
  <svg viewBox="0 0 24 24" width={size} height={size} fill="none" stroke="currentColor"
    strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M18 8a6 6 0 1 0-12 0c0 7-3 9-3 9h18s-3-2-3-9" />
    <path d="M13.7 21a2 2 0 0 1-3.4 0" />
  </svg>
);

const DCL_LINKS: NavLink[] = (["whatson", "shop", "create", "social", "learn"] as const)
  .map((id) => LINK_DEFS[id]);

type NotifState =
  | { phase: "idle" | "loading" }
  | { phase: "error" }
  | { phase: "ready"; rows: Notification[] };

type DclTopBarProps = {
  variant?: DclTopBarVariant;
  active?: DclTopBarNavId | "";
  signedIn?: boolean;
  account?: string;
  transparent?: boolean;
  onSignIn?: () => void;
  onSignOut?: () => void;
  signInHref?: string;
  fetchNotifications?: () => Promise<unknown>;
  onNavigate?: ChromeNavigate;
  mobileNavigation?: (closeMenu: () => void) => ReactNode;
  accountControl?: ReactNode;
};

const accountLinks = (account: string) => [
  { label: "My profile", href: `/profile/${encodeURIComponent(account.toLowerCase())}` },
  { label: "My assets", href: "/shop/my-items" },
];

export default function DclTopBar({
  active = "shop",
  signedIn = undefined,
  account = undefined,
  transparent = false,
  onSignIn = undefined,
  onSignOut = undefined,
  signInHref = "/shop/my-items",
  fetchNotifications = undefined,
  onNavigate = undefined,
  mobileNavigation,
  accountControl,
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
  const sharedProfile = isIn && auth.signedIn && acct.toLowerCase() === auth.account.toLowerCase();
  const sharedFaceUrl = sharedProfile ? auth.avatarUrl : undefined;
  useEffect(() => {
    setFaceUrl("");
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
        const face = (raw as { avatars?: Array<{ avatar?: { snapshots?: { face256?: unknown } } }> })
          ?.avatars?.[0]?.avatar?.snapshots?.face256;
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

  const renderNavLink = (link: NavLink) => {
    if (!link.menu?.length) {
      return (
        <ChromeLink
          key={link.id}
          href={link.href} onNavigate={onNavigate}
          className={"dtb__link" + (link.id === activeId ? " is-active" : "")}
          aria-current={link.id === activeId ? "page" : undefined}
        >
          {link.label}
        </ChromeLink>
      );
    }
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
        <ChromeLink
          href={link.href} onNavigate={onNavigate}
          className={"dtb__link" + (link.id === activeId ? " is-active" : "")}
          aria-current={link.id === activeId ? "page" : undefined}
          aria-haspopup="menu"
          aria-expanded={open}
        >
          {link.label}
        </ChromeLink>
        {open ? (
          <div className="dtb__ddmenu is-open" role="menu" aria-label={`${link.label} menu`}>
            {link.menu.map((item) => (
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
    <header className={"dtb" + (transparent && !layout ? " dtb--transparent" : "")} role="banner" aria-label="Decentraland">
      <ChromeLink className="dtb__brand" href="/" onNavigate={onNavigate} aria-label="Decentraland home">
        <img src={logo} width={40} height={40} alt="" />
      </ChromeLink>

      <button
        type="button"
        className="dtb__burger"
        ref={burgerRef}
        aria-controls={mobileMenuId}
        aria-label="Toggle navigation menu"
        aria-expanded={menuOpen}
        onClick={() => setMenuOpen((o) => !o)}
      />

      <nav className="dtb__links" aria-label="Decentraland sections">
        {links.map(renderNavLink)}
      </nav>

      <div className="dtb__right">
        {isIn ? (
          <div className="dtb__bellwrap" ref={bellRef}>
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
            <span className="dtb__avatar u-avatar" style={{ "--sz": "40px", "--hue": 268 } as CSSProperties}>
              {faceUrl && (
                <img
                  className="u-avatar__img"
                  src={faceUrl}
                  alt=""
                  width="40"
                  height="40"
                  loading="lazy"
                  onError={() => setFaceUrl("")}
                />
              )}
            </span>
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
