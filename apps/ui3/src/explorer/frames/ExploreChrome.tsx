import type { ReactNode } from "react";
import { createContext, useContext, useEffect, useLayoutEffect, useRef, useState } from "react";
import { asset } from "../../asset";
import ContextMenu from "../../components/ContextMenu";
import ProfileWidget from "../components/ProfileWidget";
import CompactNavigation from "../components/CompactNavigation";
import { Close } from "../../atoms/icons";
import { Avatar } from "../../atoms/primitives";
import "./explorechrome.css";

const ChromeNestedContext = createContext(false);

let openChromes = 0;

export type TabId =
  | "events"
  | "places"
  | "communities"
  | "marketplace"
  | "create"
  | "map"
  | "backpack"
  | "gallery"
  | "settings";

type ExploreTab = {
  id: TabId;
  label: string;
  hint: string;
  badge?: boolean;
  count?: number;
  to: string;
};

export const EXPLORE_TABS: ExploreTab[] = [
  { id: "events", label: "Events", hint: "X", badge: true, to: "Explorer/Pages/Events" },
  { id: "places", label: "Places", hint: "Z", to: "Explorer/Pages/Places" },
  { id: "communities", label: "Communities", hint: "O", to: "Explorer/Pages/Communities" },
  { id: "marketplace", label: "Marketplace", hint: "", to: "Explorer/Pages/Marketplace" },
  { id: "create", label: "Create", hint: "", to: "Explorer/Pages/Create" },
  { id: "map", label: "Map", hint: "M", to: "Explorer/Pages/Map" },
  { id: "backpack", label: "Backpack", hint: "I", to: "Explorer/Pages/Backpack" },
  { id: "gallery", label: "Gallery", hint: "K", to: "Explorer/Pages/Reel" },
  { id: "settings", label: "Settings", hint: "P", to: "Explorer/Pages/Settings" },
];

const TOP_BAR_TABS = EXPLORE_TABS.filter(tab => tab.id !== "settings");

const ICONS: Record<string, ReactNode> = {
  events: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="4.5" width="18" height="16" rx="2.5" />
      <path d="M3 9h18M8 2.5v4M16 2.5v4" />
    </svg>
  ),
  places: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 21s7-6.2 7-11a7 7 0 1 0-14 0c0 4.8 7 11 7 11Z" />
      <circle cx="12" cy="10" r="2.6" />
    </svg>
  ),
  communities: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="9" cy="8" r="3.2" />
      <path d="M3 20c0-3.3 2.7-5.5 6-5.5s6 2.2 6 5.5" />
      <path d="M16 5.2a3.2 3.2 0 0 1 0 5.6M17 14.7c2.4.5 4 2.4 4 5.3" />
    </svg>
  ),
  marketplace: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 10h18l-2-6H5l-2 6Zm1 0v10h16V10M9 20v-6h6v6" />
    </svg>
  ),
  create: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 3 4 7.5v9L12 21l8-4.5v-9L12 3Z" />
      <path d="m4 7.5 8 4.5 8-4.5M12 12v9" />
    </svg>
  ),
  map: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M9 4 3.5 6.2v13.3L9 17.3l6 2.4 5.5-2.2V4.2L15 6.4 9 4Z" />
      <path d="M9 4v13.3M15 6.4v13.3" />
    </svg>
  ),
  backpack: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M5 9.5A4.5 4.5 0 0 1 9.5 5h5A4.5 4.5 0 0 1 19 9.5V20a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V9.5Z" />
      <path d="M9 5a3 3 0 0 1 6 0M8 13h8" />
    </svg>
  ),
  gallery: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="6" width="18" height="14" rx="2.5" />
      <path d="M8 6 9.5 3.5h5L16 6" />
      <circle cx="12" cy="13" r="3.4" />
    </svg>
  ),
  settings: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="3.2" />
      <path d="M12 2.5v3M12 18.5v3M21.5 12h-3M5.5 12h-3M18.7 5.3l-2.1 2.1M7.4 16.6l-2.1 2.1M18.7 18.7l-2.1-2.1M7.4 7.4 5.3 5.3" />
    </svg>
  ),
};

type ExploreChromeProps = {
  tabs?: ExploreTab[];
  active?: TabId;
  children?: ReactNode;
  onTab?: (id: TabId) => void;
  user?: string;
  onClose?: () => void;
  avatarSrc?: string | null;
  tag?: string;
  wallet?: string;
  address?: string;
  isGuest?: boolean;
  profileOpen?: boolean;
  onProfileToggle?: () => void;
  onLobbyOpen?: () => void;
  onSignOut?: () => void;
};

export default function ExploreChrome({
  tabs,
  active,
  children,
  onTab,
  user = "Guest",
  onClose = () => {},
  avatarSrc,
  tag,
  wallet,
  address,
  isGuest,
  profileOpen = false,
  onProfileToggle,
  onLobbyOpen,
  onSignOut,
}: ExploreChromeProps) {
  const nested = useContext(ChromeNestedContext);
  const [animate] = useState(() => !nested && openChromes === 0);
  useEffect(() => {
    if (nested) return undefined;
    openChromes++;
    return () => {
      openChromes--;
    };
  }, [nested]);
  if (nested) return <>{children}</>;
  return (
    <ChromeNestedContext.Provider value={true}>
    <div className={"xc" + (active ? "" : " xc--hud") + (animate ? " xc--enter" : "")} role="dialog" aria-label="Explore">
      <a className="xc__skip" href="#xc-page">Skip to content</a>
      <header className="xc__nav">
        <button type="button" className="xc__brand" aria-label={onLobbyOpen ? "Open lobby" : "Back to world"} title={onLobbyOpen ? "Open lobby" : "Back to world (Esc)"} onClick={onLobbyOpen ?? onClose}>
          <img src={asset("assets/dcl-logo.webp")} alt="" />
          <span>Decentraland</span>
        </button>
        <button type="button" className="xc__world" aria-label="Decentraland: back to world" title="Back to world (Esc)" onClick={onClose}>
          <img src={asset("assets/dcl-logo.webp")} alt="" />
          <span>Back to<strong>world</strong></span>
        </button>

        <ExploreTabs active={active} onTab={onTab} tabs={tabs} />

        <div className="xc__right">
          <button
            type="button"
            className="xc__user"
            aria-label={`${user}: ${onLobbyOpen ? "open lobby" : "open profile"}`}
            aria-haspopup={onLobbyOpen ? undefined : "dialog"}
            aria-expanded={onLobbyOpen ? undefined : profileOpen}
            onClick={onLobbyOpen ?? onProfileToggle}
          >
            <Avatar size={28} name={user} src={avatarSrc || undefined} className="xc__avatar" />
            <span className="xc__uname">{user}</span>
          </button>
          <button
            type="button"
            className="xc__close"
            aria-label="Back to world"
            title="Back to world (Esc)"
            onClick={onClose}
          >
            <Close strokeWidth={2.2} />
          </button>
        </div>
      </header>

      <div className="xc__profileflyout">
        <ProfileWidget
          open={profileOpen}
          name={user}
          tag={tag}
          wallet={wallet}
          address={address}
          avatarSrc={avatarSrc}
          isGuest={isGuest}
          onClose={onProfileToggle}
          onSignOut={onSignOut}
          anchor="topbar"
        />
      </div>

      <main className="xc__body" id="xc-page" tabIndex={-1}>{children}</main>
    </div>
    </ChromeNestedContext.Provider>
  );
}

export const PUBLIC_WORLD_TABS: ReadonlySet<TabId> = new Set<TabId>(["events", "places", "communities", "marketplace", "create"]);
export const PREVIEW_TABS: ExploreTab[] = TOP_BAR_TABS.filter((t) => !PUBLIC_WORLD_TABS.has(t.id));

function ExploreTabs({ active, onTab, tabs = TOP_BAR_TABS }: Pick<ExploreChromeProps, "active" | "onTab" | "tabs">) {
  const nav = useRef<HTMLElement>(null);
  const measure = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const [count, setCount] = useState(tabs.length);
  const [open, setOpen] = useState(false);
  useLayoutEffect(() => {
    const resize = () => {
      if (!nav.current || !measure.current) return;
      const widths = Array.from(measure.current.children, child => child.getBoundingClientRect().width);
      const available = nav.current.clientWidth;
      if (available <= 0) return;
      let used = 0;
      let fit = 0;
      for (const width of widths) {
        if (used + width + (fit < widths.length - 1 ? 56 : 0) > available) break;
        used += width;
        fit++;
      }
      setCount(fit);
    };
    const observer = typeof ResizeObserver === "function" ? new ResizeObserver(resize) : null;
    if (nav.current) observer?.observe(nav.current);
    if (measure.current) observer?.observe(measure.current);
    resize();
    return () => observer?.disconnect();
  }, []);
  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => { if (event.target instanceof Node && !nav.current?.contains(event.target)) setOpen(false); };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, [open]);
  const close = () => { setOpen(false); trigger.current?.focus(); };
  const content = (t: ExploreTab) => <><span className="xc__ticon">{ICONS[t.id]}{t.badge && <span className="xc__dot" />}</span><span className="xc__tlabel">{t.label}{(t.count != null || t.hint) && <em className="xc__hint">{t.count != null ? `[${t.count}]` : `[${t.hint}]`}</em>}</span></>;
  return <nav className="xc__tabs" aria-label="Explore sections" ref={nav}>
    <CompactNavigation className="xc__compact" name="Switch Explorer section" label={tabs.find(t => t.id === active)?.label ?? "Explore"}
      icon={active ? ICONS[active] : undefined}
      items={tabs.map(t => ({ kind: "button", label: <span aria-current={t.id === active ? "page" : undefined}>{t.label}{t.id === active ? " ✓" : ""}</span>, icon: ICONS[t.id], to: t.to, onClick: () => onTab?.(t.id) }))} />
    <div className="xc__measure" ref={measure} aria-hidden="true" inert>{tabs.map(t => <span className="xc__tab" key={t.id}>{content(t)}</span>)}</div>
    {tabs.slice(0, count).map(t => <button key={t.id} type="button" className={"xc__tab" + (t.id === active ? " is-active" : "")} aria-current={t.id === active ? "page" : undefined} data-sb-linkto={t.to} onClick={() => onTab?.(t.id)}>{content(t)}</button>)}
    {count < tabs.length && <div className="xc__overflow">
      <button ref={trigger} type="button" className={"xc__tab xc__more" + (tabs.slice(count).some(t => t.id === active) ? " is-active" : "")} aria-label={"More sections" + (tabs.slice(count).some(t => t.id === active) ? ` (${tabs.find(t => t.id === active)?.label})` : "")} aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(value => !value)}>&bull;&bull;&bull;</button>
      {open && <ContextMenu autoFocus onClose={close} items={tabs.slice(count).map(t => ({ kind: "button", label: <span aria-current={t.id === active ? "page" : undefined}>{t.label}{t.id === active ? " \u2713" : ""}</span>, to: t.to, icon: ICONS[t.id], onClick: () => { onTab?.(t.id); close(); } }))} />}
    </div>}
  </nav>;
}
