import type { CSSProperties, ReactNode } from "react";
import { useState } from "react";
import { useChromeAuth } from "../../web/frames/chrome-auth";
import DclTopBar from "../../web/frames/DclTopBar";
import ChromeLink from "../../web/frames/ChromeLink";
import AccountMenu from "../../web/frames/AccountMenu";
import CreatorHubSyncChip from "../components/CreatorHubSyncChip";
import "./creatorhubchrome.css";
import SiteLayout from "../../web/frames/SiteLayout";
import SiteSectionNavigation from "../../web/frames/SiteSectionNavigation";
import { truncateAddress } from "../../data/format";

export type CreatorHubNavId =
  | "home"
  | "scenes"
  | "templates"
  | "preview"
  | "publish"
  | "admissions"
  | "collections"
  | "curate"
  | "manage"
  | "storage"
  | "land"
  | "names"
  | "metrics"
  | "sources"
  | "learn";

type NavItem = {
  id: CreatorHubNavId;
  group: string;
  label: string;
  href: string;
  icon: ReactNode;
  hint?: string;
  secondary?: boolean;
  external?: boolean;
};

const CREATORHUB_NAV: NavItem[] = [
  {
    id: "home",
    group: "Create",
    label: "Home",
    href: "/create",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <path d="M3 9.2 10 3.5l7 5.7V17a1 1 0 0 1-1 1h-3v-5H7v5H4a1 1 0 0 1-1-1V9.2Z" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinejoin="round" />
      </svg>
    ),
  },
  {
    id: "templates",
    group: "Create",
    label: "Templates",
    href: "/create/templates",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <rect x="3" y="3" width="6" height="6" rx="1.3" stroke="currentColor" strokeWidth="1.5" fill="none" />
        <rect x="11" y="3" width="6" height="6" rx="1.3" stroke="currentColor" strokeWidth="1.5" fill="none" />
        <rect x="3" y="11" width="6" height="6" rx="1.3" stroke="currentColor" strokeWidth="1.5" fill="none" />
        <rect x="11" y="11" width="6" height="6" rx="1.3" stroke="currentColor" strokeWidth="1.5" fill="none" />
      </svg>
    ),
  },
  {
    id: "scenes",
    group: "Create",
    label: "Scenes",
    href: "/create/scenes",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <path d="M10 2.5 17.5 7 10 11.5 2.5 7 10 2.5Z" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinejoin="round" />
        <path d="M2.5 13 10 17.5 17.5 13" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinejoin="round" />
      </svg>
    ),
  },
  {
    id: "publish",
    group: "Create",
    label: "Publishing",
    href: "/create/publish",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <path d="M10 13V4M6.5 7.5 10 4l3.5 3.5" stroke="currentColor" strokeWidth="1.6" fill="none" strokeLinecap="round" strokeLinejoin="round" />
        <path d="M4 14.5v1A1.5 1.5 0 0 0 5.5 17h9a1.5 1.5 0 0 0 1.5-1.5v-1" stroke="currentColor" strokeWidth="1.6" fill="none" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    ),
  },
  {
    id: "admissions",
    group: "Create",
    label: "Admissions",
    href: "/create/admissions",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <rect x="4" y="9" width="12" height="8" rx="1.5" stroke="currentColor" strokeWidth="1.5" fill="none" />
        <path d="M7 9V6.5a3 3 0 0 1 6 0V9" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" />
      </svg>
    ),
  },
  {
    id: "collections",
    group: "Create",
    label: "Collections",
    href: "/create/wearables",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <path d="M3 3h6l8 8-6 6-8-8V3Z" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinejoin="round" />
        <circle cx="6.4" cy="6.4" r="1.1" fill="currentColor" />
      </svg>
    ),
  },
  {
    id: "curate",
    group: "Manage",
    label: "Curate",
    href: "/create/curate",
    secondary: true,
    hint: "Committee",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <path d="M10 2.5 16 4.7v4.3c0 3.4-2.4 6.5-6 8-3.6-1.5-6-4.6-6-8V4.7L10 2.5Z" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinejoin="round" />
        <path d="m7.3 9.8 1.9 1.9 3.5-3.6" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    ),
  },
  {
    id: "manage",
    group: "Manage",
    label: "Worlds",
    href: "/create/worlds",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <path d="M3 5h14M3 10h14M3 15h9" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      </svg>
    ),
  },
  {
    id: "land",
    group: "Manage",
    label: "Land",
    href: "/create/land",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <rect x="3" y="3" width="14" height="14" rx="1.3" stroke="currentColor" strokeWidth="1.5" fill="none" />
        <path d="M7.7 3v14M12.3 3v14M3 7.7h14M3 12.3h14" stroke="currentColor" strokeWidth="1.5" fill="none" />
      </svg>
    ),
  },
  {
    id: "names",
    group: "Manage",
    label: "Names",
    href: "/create/names",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <path d="M10.3 3H16a1 1 0 0 1 1 1v5.7a1 1 0 0 1-.3.7l-6 6a1 1 0 0 1-1.4 0L3.6 11.7a1 1 0 0 1 0-1.4l6-6a1 1 0 0 1 .7-.3Z" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinejoin="round" />
        <circle cx="13.3" cy="6.7" r="1.1" fill="currentColor" />
      </svg>
    ),
  },
  {
    id: "metrics",
    group: "Manage",
    label: "Metrics",
    href: "/create/metrics",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <path d="M3 16.5h14" stroke="currentColor" strokeWidth="1.6" fill="none" strokeLinecap="round" />
        <path d="M6 16.5V12M10 16.5V8.5M14 16.5V5" stroke="currentColor" strokeWidth="1.6" fill="none" strokeLinecap="round" />
      </svg>
    ),
  },
  {
    id: "storage",
    group: "Manage",
    label: "Storage",
    href: "/create/storage?quota=1",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <rect x="3" y="5.5" width="14" height="9" rx="1.5" stroke="currentColor" strokeWidth="1.5" fill="none" />
        <path d="M6 11.5h5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        <circle cx="14" cy="11.5" r="0.9" fill="currentColor" />
      </svg>
    ),
  },
  {
    id: "sources",
    group: "Manage",
    label: "Data sources",
    href: "/create/data-sources",
    secondary: true,
    hint: "What is connected",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <ellipse cx="10" cy="5.2" rx="6" ry="2.4" stroke="currentColor" strokeWidth="1.5" fill="none" />
        <path d="M4 5.2v9.6c0 1.3 2.7 2.4 6 2.4s6-1.1 6-2.4V5.2" stroke="currentColor" strokeWidth="1.5" fill="none" />
        <path d="M4 10c0 1.3 2.7 2.4 6 2.4s6-1.1 6-2.4" stroke="currentColor" strokeWidth="1.5" fill="none" />
      </svg>
    ),
  },
  {
    id: "learn",
    group: "Learn",
    label: "Tutorials",
    href: "/create/learn",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <path d="M10 5.6C8.5 4.4 6.2 4.1 3.5 4.6v10c2.7-.5 5-.2 6.5 1 1.5-1.2 3.8-1.5 6.5-1v-10c-2.7-.5-5-.2-6.5 1Z" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinejoin="round" />
        <path d="M10 5.6v10" stroke="currentColor" strokeWidth="1.5" />
      </svg>
    ),
  },
];

type NavGroup = { group: string; items: { item: NavItem; i: number }[] };

function buildNavGroups(nav: NavItem[]): NavGroup[] {
  return nav.reduce<NavGroup[]>((groups, item, i) => {
    const last = groups[groups.length - 1];
    if (last && last.group === item.group) last.items.push({ item, i });
    else groups.push({ group: item.group, items: [{ item, i }] });
    return groups;
  }, []);
}

const DownloadIcon = () => (
  <svg viewBox="0 0 20 20" width="18" height="18" aria-hidden="true">
    <path d="M9 3h2v7h3l-4 4-4-4h3V3z" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinejoin="round" />
    <path d="M4 16.5h12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

const avatarStyle: CSSProperties & { "--sz": string; "--hue": number } = {
  "--sz": "40px",
  "--hue": 212,
};

function AccountAvatar({ src }: { src?: string }) {
  const [broken, setBroken] = useState(false);
  return (
    <span className="ch__avatar u-avatar" style={avatarStyle}>
      {src && !broken ? (
        <img
          className="u-avatar__img"
          src={src}
          alt=""
          width="40"
          height="40"
          loading="lazy"
          onError={() => setBroken(true)}
        />
      ) : null}
    </span>
  );
}

type CreatorHubChromeProps = {
  active?: CreatorHubNavId | null;
  committee?: boolean;
  onTab?: (id: CreatorHubNavId) => void;
  children?: ReactNode;
  signedIn?: boolean;
  account?: string;
  name?: string;
  avatarUrl?: string;
  onSettings?: () => void;
  onAccount?: () => void;
  onSignIn?: () => void;
  onSignOut?: () => void;
  accountHref?: string;
  settingsHref?: string;
};

export default function CreatorHubChrome({
  active = "home",
  committee,
  onTab,
  children,
  signedIn,
  account,
  name,
  avatarUrl,
  onSettings,
  onAccount,
  onSignIn,
  onSignOut,
  accountHref = "/marketplace/account",
  settingsHref = "/create/settings",
}: CreatorHubChromeProps) {
  const auth = useChromeAuth();
  const isCommittee = committee ?? auth.committee;
  const nav = isCommittee ? CREATORHUB_NAV : CREATORHUB_NAV.filter((item) => item.id !== "curate");
  const navGroups = buildNavGroups(nav);
  const isIn = signedIn ?? auth.signedIn;
  const acct = account ?? auth.account;
  const displayName = name ?? auth.name;
  const face = avatarUrl ?? auth.avatarUrl;
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
  const accountLinks = [
    { label: "My assets", href: accountHref },
    { label: "Download", href: settingsHref },
  ];
  const accountInner = (
    <>
      <AccountAvatar key={face || "hue"} src={face} />
      <span className="ch__accmeta">
        <span className="ch__accname">{displayName || "My Account"}</span>
        {acct ? <span className="ch__accaddr" title={acct}>{truncateAddress(acct)}</span> : null}
      </span>
    </>
  );

  const accountLabel = `${displayName || "My Account"}${acct ? " " + truncateAddress(acct) : ""}`;

  const showSettings = !!onSettings || !!settingsHref;
  const showAccount = isIn && (onAccount || accountHref);

  const renderNavigation = (closeMenu: () => void) => (
    <>
      {navGroups.map(group => <SiteSectionNavigation
        key={group.group} title={group.group === "Create" ? null : group.group}
        label={group.group === "Create" ? "Creator Hub sections" : "Creator Hub " + group.group.toLowerCase()}
        items={group.items.map(({ item }) => ({ id: item.id, href: item.href, label: item.label, icon: item.icon }))}
        active={active} onSelect={onTab} closeMenu={closeMenu}
      />)}
      <CreatorHubSyncChip />
      {showSettings ? onSettings ? (
        <button className="side-nav-item" type="button" onClick={() => { closeMenu(); onSettings(); }}><DownloadIcon />Download</button>
      ) : <ChromeLink className="side-nav-item" href={settingsHref} onClick={closeMenu}><DownloadIcon />Download</ChromeLink> : null}
    </>
  );
  return (
    <SiteLayout sections={renderNavigation} navigation={
      <DclTopBar active="create" signedIn={isIn} account={acct} onSignIn={doSignIn} onSignOut={doSignOut} signInHref={accountHref}
        accountControl={showAccount ? onAccount ? (
          <button type="button" className="ch__account" aria-label={accountLabel} onClick={onAccount}>{accountInner}</button>
        ) : <AccountMenu className="ch__accountwrap" triggerClassName="ch__account" triggerLabel={accountLabel}
          name={displayName} account={acct} links={accountLinks} onSwitchAccount={switchAccount} onSignOut={doSignOut}
          align="right" placement="below">{accountInner}</AccountMenu> : undefined}
      />
    }>
      <div className="ch ui2" data-label="Creator Hub">
        <ChromeLink className="ch__skip" href="#ch-main">Skip to content</ChromeLink>
        <main id="ch-main" className="ch__main">{children}</main>
      </div>
    </SiteLayout>
  );
}

export function CreatorHubChromeMaybe({
  chrome = true,
  children,
  ...rest
}: CreatorHubChromeProps & { chrome?: boolean }) {
  if (!chrome) return <>{children}</>;
  return <CreatorHubChrome {...rest}>{children}</CreatorHubChrome>;
}
