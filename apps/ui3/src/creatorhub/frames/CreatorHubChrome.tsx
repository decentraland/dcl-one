import type { CSSProperties, ReactNode } from "react";
import { useState } from "react";
import { useChromeAuth } from "../../web/frames/chrome-auth";
import DclTopBar from "../../web/frames/DclTopBar";
import ChromeLink from "../../web/frames/ChromeLink";
import AccountMenu from "../../web/frames/AccountMenu";
import "./creatorhubchrome.css";
import SiteLayout from "../../web/frames/SiteLayout";
import SiteSectionNavigation from "../../web/frames/SiteSectionNavigation";
import { truncateAddress } from "../../data/format";

export type CreatorHubNavId = "collections" | "curate";

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
    id: "collections",
    group: "Create",
    label: "Collections",
    href: "/create/collections",
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
    href: "/create/curation",
    secondary: true,
    hint: "Committee",
    icon: (
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <path d="M10 2.5 16 4.7v4.3c0 3.4-2.4 6.5-6 8-3.6-1.5-6-4.6-6-8V4.7L10 2.5Z" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinejoin="round" />
        <path d="m7.3 9.8 1.9 1.9 3.5-3.6" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" />
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
  active = "collections",
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
  settingsHref = "/download/creator-hub",
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
