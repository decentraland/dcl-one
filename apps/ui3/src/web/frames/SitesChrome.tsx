import type { FormEvent, ReactNode } from "react";
import { useContext, useEffect, useId, useRef, useState } from "react";
import DclTopBar, { type DclTopBarNavId } from "./DclTopBar";
import SitesCreditsChip from "./SitesCreditsChip";
import { useChromeAuth } from "./chrome-auth";
import { chromeLinkProps, useChromeNav, type ChromeNavigate } from "./chrome-nav";
import "../../atoms/maskicon.css";
import wordmark from "../../atoms/assets/decentraland-wordmark.svg?url";
import { siteHost } from "../../data/site";
import { docsUrl } from "../../data/docs";
import "./siteschrome.css";
import SiteLayout from "./SiteLayout";
import SiteSectionNavigation from "./SiteSectionNavigation";
import { ProfileNavigationContext } from "./ProfileNavigation";

export type SitesNavId =
  | "play"
  | "marketplace"
  | "docs"
  | "legal"
  | "create"
  | "explore"
  | "whatson"
  | "shop"
  | "learn"
  /** No section is active: decentraland.org marks none on its download pages (its nav links are absolute URLs). */
  | "none";

const ACTIVE_MAP: Record<SitesNavId, DclTopBarNavId | ""> = {
  play: "",
  marketplace: "shop",
  docs: "learn",
  legal: "",
  create: "create",
  explore: "whatson",
  whatson: "whatson",
  shop: "shop",
  learn: "learn",
  none: "",
};

type FooterLink = { label: string; href: string };
type FooterCol = { title: string; links: FooterLink[] };

// from: sites@17b3535a:src/components/LandingFooter/footerConfig.ts (labels and order; dcl-one's own routes where it has them)
const FOOTER_COLS: FooterCol[] = [
  {
    title: "Getting Started",
    links: [
      { label: "About Decentraland", href: docsUrl("introduction/about-decentraland") },
      { label: "Download Decentraland", href: "/download" },
      { label: "System Requirements", href: docsUrl("in-world/settings-and-performance") },
      { label: "FAQs", href: docsUrl("faqs/decentraland-101") },
      { label: "Contact Support", href: "/help" },
    ],
  },
  {
    title: "Resources",
    links: [
      { label: "Shop", href: "/shop" },
      { label: "Creator Hub", href: "/create" },
      { label: "Docs", href: docsUrl() },
      { label: "Blog", href: "/blog" },
      { label: "Vote", href: "https://decentraland.org/dao" },
    ],
  },
];

const LEGAL: FooterLink[] = [
  { label: "Privacy Policy", href: "/privacy" },
  { label: "Terms of Use", href: "/terms" },
  { label: "Content Policy", href: "/content-policy" },
  { label: "Code of Ethics", href: "/ethics" },
];

type Language = { code: string; label: string; flag: string };

const DEFAULT_LANG: Language = { code: "en", label: "English", flag: "1f1ec-1f1e7" };
const LANGUAGES: Language[] = [
  DEFAULT_LANG,
  { code: "es", label: "Espa\u{F1}ol", flag: "1f1ea-1f1f8" },
  { code: "zh", label: "\u{4E2D}\u{6587}", flag: "1f1e8-1f1f3" },
];

const LANG_STORAGE_KEY = "dcl:lang";

function flagUrl(flag: string): string {
  return `/assets/twemoji-${flag}.svg`;
}

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

// from: sites@17b3535a:src/components/LandingFooter/SocialIcons.tsx (the icons, their boxes and order)
type Social = { name: string; href: string; path: string; box?: [width: number, height: number] };

const SOCIALS: Social[] = [
  {
    name: "Discord",
    href: "https://dcl.gg/discord",
    box: [28, 22],
    path: "M23.7 1.84A23.25 23.25 0 0 0 17.96 0c-.25.45-.54 1.05-.74 1.53a21.5 21.5 0 0 0-6.45 0C10.57 1.05 10.27.45 10.02 0A23.2 23.2 0 0 0 4.27 1.85C.62 7.34-.38 12.7.12 18a23.37 23.37 0 0 0 7.13 3.6 17.4 17.4 0 0 0 1.53-2.49 15.17 15.17 0 0 1-2.41-1.16c.2-.15.4-.3.59-.46a16.63 16.63 0 0 0 14.1 0c.19.16.39.31.58.46-.77.45-1.57.84-2.41 1.16.44.87.96 1.7 1.53 2.49a23.3 23.3 0 0 0 7.14-3.61c.58-6.15-1-11.47-4.1-16.15ZM9.35 14.74c-1.4 0-2.54-1.28-2.54-2.85s1.12-2.86 2.54-2.86 2.56 1.29 2.54 2.86c0 1.57-1.12 2.85-2.54 2.85Zm9.38 0c-1.4 0-2.54-1.28-2.54-2.85s1.12-2.86 2.54-2.86 2.56 1.29 2.54 2.86c0 1.57-1.12 2.85-2.54 2.85Z",
  },
  {
    name: "GitHub",
    href: "https://github.com/decentraland",
    path: "M12 .3a12 12 0 0 0-3.8 23.38c.6.12.82-.26.82-.58v-2.02c-3.34.73-4.04-1.61-4.04-1.61-.55-1.39-1.34-1.76-1.34-1.76-1.08-.74.08-.73.08-.73 1.2.09 1.84 1.24 1.84 1.24 1.07 1.83 2.8 1.3 3.49 1 .1-.78.42-1.3.76-1.6-2.67-.31-5.47-1.34-5.47-5.93 0-1.31.47-2.38 1.24-3.22-.13-.3-.54-1.52.12-3.18 0 0 1-.33 3.3 1.23a11.5 11.5 0 0 1 6.02 0c2.28-1.56 3.29-1.23 3.29-1.23.66 1.66.25 2.88.12 3.18a4.65 4.65 0 0 1 1.24 3.22c0 4.61-2.81 5.62-5.48 5.92.43.37.81 1.1.81 2.22v3.29c0 .32.22.7.82.58A12 12 0 0 0 12 .3Z",
  },
  {
    name: "X",
    href: "https://x.com/decentraland",
    path: "M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231 5.45-6.231Zm-1.161 17.52h1.833L7.084 4.126H5.117L17.083 19.77Z",
  },
  {
    name: "Instagram",
    href: "https://instagram.com/decentraland_foundation/",
    path: "M12 2.163c3.204 0 3.584.012 4.85.07 3.252.148 4.771 1.691 4.919 4.919.058 1.265.069 1.645.069 4.849 0 3.205-.012 3.584-.069 4.849-.149 3.225-1.664 4.771-4.919 4.919-1.266.058-1.644.07-4.85.07-3.204 0-3.584-.012-4.849-.07-3.26-.149-4.771-1.699-4.919-4.92-.058-1.265-.07-1.644-.07-4.849 0-3.204.013-3.583.07-4.849.149-3.227 1.664-4.771 4.919-4.919 1.266-.057 1.645-.069 4.849-.069ZM12 0C8.741 0 8.333.014 7.053.072 2.695.272.273 2.69.073 7.052.014 8.333 0 8.741 0 12c0 3.259.014 3.668.072 4.948.2 4.358 2.618 6.78 6.98 6.98C8.333 23.986 8.741 24 12 24c3.259 0 3.668-.014 4.948-.072 4.354-.2 6.782-2.618 6.979-6.98.059-1.28.073-1.689.073-4.948 0-3.259-.014-3.667-.072-4.947-.196-4.354-2.617-6.78-6.979-6.98C15.668.014 15.259 0 12 0Zm0 5.838a6.162 6.162 0 1 0 0 12.324 6.162 6.162 0 0 0 0-12.324ZM12 16a4 4 0 1 1 0-8 4 4 0 0 1 0 8Zm6.406-11.845a1.44 1.44 0 1 0 0 2.881 1.44 1.44 0 0 0 0-2.881Z",
  },
  {
    name: "YouTube",
    href: "https://youtube.com/@decentraland_foundation",
    path: "M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.546 12 3.546 12 3.546s-7.505 0-9.377.504A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.504 9.376.504 9.376.504s7.505 0 9.377-.504a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814ZM9.545 15.568V8.432L15.818 12l-6.273 3.568Z",
  },
  {
    name: "TikTok",
    href: "https://tiktok.com/@decentraland_fdn",
    path: "M12.525.02c1.31-.02 2.61-.01 3.91-.02.08 1.53.63 3.09 1.75 4.17 1.12 1.11 2.7 1.62 4.24 1.79v4.03c-1.44-.05-2.89-.35-4.2-.97-.57-.26-1.1-.59-1.62-.93-.01 2.92.01 5.84-.02 8.75-.08 1.4-.54 2.79-1.35 3.94-1.31 1.92-3.58 3.17-5.91 3.21-1.43.08-2.86-.31-4.08-1.03-2.02-1.19-3.44-3.37-3.65-5.71-.02-.5-.03-1-.01-1.49.18-1.9 1.12-3.72 2.58-4.96 1.66-1.44 3.98-2.13 6.15-1.72.02 1.48-.04 2.96-.04 4.44-.99-.32-2.15-.23-3.02.37-.63.41-1.11 1.04-1.36 1.75-.21.51-.15 1.07-.14 1.61.24 1.64 1.82 3.02 3.5 2.87 1.12-.01 2.19-.66 2.77-1.61.19-.33.4-.67.41-1.06.1-1.79.06-3.57.07-5.36.01-4.03-.01-8.05.02-12.07Z",
  },
  {
    name: "LinkedIn",
    href: "https://linkedin.com/company/decentralandorg",
    path: "M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286ZM5.337 7.433a2.062 2.062 0 1 1 0-4.123 2.062 2.062 0 0 1 0 4.123ZM6.863 20.452H3.804V9h3.059v11.452ZM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003Z",
  },
];

/** The footer's ChevronDown (sites: LandingFooter/SocialIcons.tsx) in its DropdownChevron box. */
function FootChevron() {
  return (
    <span className="st__chev" aria-hidden="true">
      <svg viewBox="0 0 24 24" fill="currentColor" width="20" height="20">
        <path d="M7.41 8.59 12 13.17l4.59-4.58L18 10l-6 6-6-6 1.41-1.41Z" />
      </svg>
    </span>
  );
}

function Connect({ className }: { className: string }) {
  return (
    <div className={className}>
      <p className="st__foottitle">Connect</p>
      <div className="st__socials">
        {SOCIALS.map(({ name, href, path, box: [w, h] = [24, 24] }) => (
          <a key={name} href={href} className="st__social" aria-label={name} target="_blank" rel="noopener noreferrer">
            <svg viewBox={`0 0 ${w} ${h}`} fill="currentColor" width={w} height={h} aria-hidden="true"><path d={path} /></svg>
          </a>
        ))}
      </div>
    </div>
  );
}

function footerLinkProps(href: string, go: ChromeNavigate | undefined) {
  return /^https?:/.test(href) ? { href, target: "_blank", rel: "noopener noreferrer" } : chromeLinkProps(href, go);
}

type SitesChromeProps = {
  children?: ReactNode;
  active?: SitesNavId;
  signedIn?: boolean;
  mana?: string;
  account?: string;
  onSignIn?: () => void;
  overlayNav?: boolean;
  headerSurface?: "solid" | "overlay";
  /** The landing page's nav: transparent over the hero, and only the logo, wordmark and Sign in while signed out. */
  minimalNav?: boolean;
  hideNav?: boolean;
  secondaryNavigation?: { title: string; active?: string; items: readonly { id: string; label: string; href: string }[] };
};

type NewsletterState =
  | { phase: "idle" }
  | { phase: "sending" }
  | { phase: "ok" }
  | { phase: "error"; message: string };

export default function SitesChrome({
  children,
  active = "whatson",
  signedIn,
  account,
  onSignIn,
  overlayNav = false,
  headerSurface = "solid",
  minimalNav = false,
  hideNav = false,
  secondaryNavigation,
}: SitesChromeProps) {
  const pageId = useId();
  const profileNav = useContext(ProfileNavigationContext);
  const [openCol, setOpenCol] = useState<string | null>(null);
  const auth = useChromeAuth();
  const isIn = signedIn ?? auth.signedIn;
  const acct = account ?? auth.account;
  const signIn = onSignIn ?? auth.onSignIn;
  const { navigate: go } = useChromeNav();

  const [email, setEmail] = useState("");
  const [news, setNews] = useState<NewsletterState>({ phase: "idle" });
  const submitNewsletter = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const value = email.trim();
    if (!EMAIL_RE.test(value)) {
      setNews({ phase: "error", message: "Enter a valid email address." });
      return;
    }
    setNews({ phase: "sending" });
    fetch("/v1/newsletter", {
      method: "POST",
      headers: { "content-type": "application/json", accept: "application/json" },
      body: JSON.stringify({ email: value, source: siteHost() }),
    })
      .then((res) => {
        if (!res.ok) throw new Error(`newsletter returned ${res.status}`);
        setNews({ phase: "ok" });
        setEmail("");
      })
      .catch(() => {
        setNews({ phase: "error", message: "Something went wrong. Please try again." });
      });
  };

  const [lang, setLang] = useState<Language>(DEFAULT_LANG);
  const [langOpen, setLangOpen] = useState(false);
  const langRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    try {
      const saved = window.localStorage.getItem(LANG_STORAGE_KEY);
      const found = LANGUAGES.find((l) => l.code === saved);
      if (found) {
        setLang(found);
        document.documentElement.lang = found.code;
      }
    } catch {
    }
  }, []);

  useEffect(() => {
    if (!langOpen) return;
    const onDown = (e: MouseEvent) => {
      if (langRef.current && !langRef.current.contains(e.target as Node)) setLangOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setLangOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [langOpen]);

  const pickLang = (l: Language) => {
    setLang(l);
    setLangOpen(false);
    try {
      window.localStorage.setItem(LANG_STORAGE_KEY, l.code);
      document.cookie = `lang=${l.code}; path=/; max-age=31536000; samesite=lax`;
    } catch {
    }
    document.documentElement.lang = l.code;
  };

  return (
    <SiteLayout overlayNavigation={headerSurface === "overlay"} navigation={!hideNav ? <DclTopBar
      variant="sites" transparent={headerSurface === "overlay"} active={ACTIVE_MAP[active]} signedIn={isIn} account={acct}
      onSignIn={signIn} fetchNotifications={auth.fetchNotifications} minimal={minimalNav}
      balances={isIn && acct ? <SitesCreditsChip account={acct} /> : undefined}
    /> : null} className={"site-layout--sites" + (overlayNav ? " site-layout--overlay" : "")} sections={profileNav ? closeMenu => <SiteSectionNavigation
      title="Profile" label="Profile sections" items={profileNav.tabs} active={profileNav.active}
      onSelect={profileNav.onTab} closeMenu={closeMenu}
    /> : secondaryNavigation ? closeMenu => <SiteSectionNavigation
      title={secondaryNavigation.title} items={secondaryNavigation.items} active={secondaryNavigation.active} closeMenu={closeMenu}
    /> : undefined}>
    <div className="st ui2" data-label="Decentraland">
      <a className="st__skip" href={"#" + pageId}>Skip to content</a>

      <div className="st__page" id={pageId} tabIndex={-1}>{children}</div>

      <footer className="st__footer">
        <div className="st__footmain">
          <div className="st__footleft">
            <img className="st__wordmark" src={wordmark} alt="Decentraland" width={384} height={48} />
            <div className="st__news">
              <p className="st__newstitle">Get the weekly highlights in your inbox</p>
              <form className="st__newsform" onSubmit={submitNewsletter} noValidate>
                <input
                  className="st__newsinput"
                  type="email"
                  placeholder="Enter Your Email"
                  aria-label="Email"
                  value={email}
                  onChange={(e) => {
                    setEmail(e.target.value);
                    if (news.phase !== "idle") setNews({ phase: "idle" });
                  }}
                  disabled={news.phase === "sending"}
                />
                <button type="submit" className="st__newsbtn" disabled={news.phase === "sending"}>
                  {news.phase === "sending" ? "Sending\u{2026}" : "Subscribe"}
                </button>
              </form>
              {news.phase === "ok" && (
                <p className="st__newsnote st__newsnote--ok" role="status">Subscribed!</p>
              )}
              {news.phase === "error" && (
                <p className="st__newsnote st__newsnote--err" role="alert">{news.message}</p>
              )}
            </div>
            <Connect className="st__connect st__connect--desktop" />
          </div>

          <div className="st__footcols">
            {FOOTER_COLS.map((col) => (
              <div key={col.title} className="st__footcol">
                <p className="st__foottitle">{col.title}</p>
                {col.links.map((link) => (
                  <a key={link.label} {...footerLinkProps(link.href, go)} className="st__footlink">
                    {link.label}
                  </a>
                ))}
              </div>
            ))}
          </div>

          <div className="st__footmenu">
            <p className="st__footmenulabel">Menu</p>
            {FOOTER_COLS.map((col) => (
              <div key={col.title}>
                <button
                  type="button"
                  className="st__footdrop"
                  aria-expanded={openCol === col.title}
                  onClick={() => setOpenCol((o) => (o === col.title ? null : col.title))}
                >
                  {col.title}
                  <FootChevron />
                </button>
                <div className="st__footdropcontent" data-open={openCol === col.title || undefined}>
                  {col.links.map((link) => (
                    <a key={link.label} {...footerLinkProps(link.href, go)} className="st__footmlink">
                      {link.label}
                    </a>
                  ))}
                </div>
              </div>
            ))}
          </div>
          <Connect className="st__connect st__connect--mobile" />
        </div>

        <div className="st__footbar">
          <div className="st__footbarleft">
            <div className="st__langwrap" ref={langRef}>
              <button
                type="button"
                className="st__lang"
                aria-label={`Change language: ${lang.label}`}
                aria-haspopup="listbox"
                aria-expanded={langOpen}
                onClick={() => setLangOpen((o) => !o)}
              >
                <img className="st__flag" src={flagUrl(lang.flag)} alt="" width="16" height="16" />
                {lang.label}
                <FootChevron />
              </button>
              {langOpen && (
                <div className="st__langmenu" role="listbox" aria-label="Language">
                  {LANGUAGES.map((l) => (
                    <button
                      key={l.code}
                      type="button"
                      role="option"
                      aria-selected={l.code === lang.code}
                      className={"st__langopt" + (l.code === lang.code ? " is-active" : "")}
                      onClick={() => pickLang(l)}
                    >
                      <img className="st__flag" src={flagUrl(l.flag)} alt="" width="16" height="16" />
                      {l.label}
                    </button>
                  ))}
                </div>
              )}
            </div>
            <div className="st__legal">
              {LEGAL.map((l) => (
                <a key={l.label} href={l.href} className="st__legallink">
                  {l.label}
                </a>
              ))}
            </div>
          </div>
          <div className="st__footbarright">
            <span className="st__copy">&copy; {new Date().getFullYear()} Decentraland</span>
          </div>
        </div>
      </footer>
    </div>
    </SiteLayout>
  );
}

export function SitesChromeMaybe({
  chrome = true,
  children,
  ...rest
}: SitesChromeProps & { chrome?: boolean }) {
  if (!chrome) return <>{children}</>;
  return <SitesChrome {...rest}>{children}</SitesChrome>;
}
