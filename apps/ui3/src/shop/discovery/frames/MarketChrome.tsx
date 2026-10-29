// from: marketplace@3bf3662f:webapp/src/components/AnnouncementBar/{AnnouncementBar.tsx,AnnouncementBar.module.css}
//       (the "Meet the Decentraland Shop" strip over upstream Marketplace's pages), webapp/src/images/announcement-bar-*.svg,
//       webapp/src/modules/translation/locales/en.json (announcement_bar); ui@8a63476:src/components/Footer/{Footer.tsx,Footer.css}
//       (the one-row Marketplace footer), src/assets/{discord,reddit,github,twitter}.svg
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.

import { useState } from "react";
import { Link } from "react-router";

import { SHOP_BASE } from "../../routes";
import "./marketchrome.css";

const ANNOUNCEMENT_BAR_KEY = "shop-announcement-bar";

function dismissed(): boolean {
  try {
    return typeof window !== "undefined" && window.localStorage.getItem(ANNOUNCEMENT_BAR_KEY) !== null;
  } catch {
    return false;
  }
}

export function MarketAnnouncementBar() {
  const [hidden, setHidden] = useState(dismissed);
  if (hidden) return null;
  const dismiss = () => {
    try {
      window.localStorage.setItem(ANNOUNCEMENT_BAR_KEY, "1");
    } catch {
    }
    setHidden(true);
  };
  return (
    <aside className="shop-mbar" data-testid="market-announcement">
      <p className="shop-mbar__message">
        <span className="shop-mbar__highlight">Meet the Decentraland Shop</span> - New Layout, New Outfits, and Meet the creators.
      </p>
      <Link className="shop-mbar__cta" to={SHOP_BASE}>
        <span className="shop-mbar__ctalabel">Try it out</span>
        <span className="shop-mbar__ctaicon" aria-hidden="true" />
      </Link>
      <button type="button" className="shop-mbar__close" onClick={dismiss} aria-label="Dismiss announcement" />
    </aside>
  );
}

const LINKS = [
  { label: "Home", href: "/" },
  { label: "Privacy Policy", href: "/privacy" },
  { label: "Terms of Use", href: "/terms" },
  { label: "Content Policy", href: "/content-policy" },
  { label: "Code of Ethics", href: "/ethics" },
];

const SOCIAL = [
  { name: "Discord", kind: "discord", href: "https://dcl.gg/discord" },
  { name: "Reddit", kind: "reddit", href: "https://reddit.com/r/decentraland" },
  { name: "GitHub", kind: "github", href: "https://github.com/decentraland" },
  { name: "X", kind: "twitter", href: "https://x.com/decentraland" },
];

export function MarketFooter() {
  return (
    <footer className="shop-mfooter" data-testid="market-footer">
      <div className="shop-mfooter__main">
        <span className="shop-mfooter__lang">
          <span aria-hidden="true">🇬🇧</span>
          English
          <span className="shop-mfooter__caret" aria-hidden="true" />
        </span>
        <nav className="shop-mfooter__links" aria-label="Legal">
          {LINKS.map((l) => (
            <a key={l.label} href={l.href}>
              {l.label}
            </a>
          ))}
        </nav>
      </div>
      <div className="shop-mfooter__secondary">
        <div className="shop-mfooter__social">
          {SOCIAL.map((s) => (
            <a key={s.kind} href={s.href} target="_blank" rel="noopener noreferrer" aria-label={s.name}>
              <i className="shop-mfooter__icon" data-kind={s.kind} />
            </a>
          ))}
        </div>
        <span className="shop-mfooter__copy">&copy; {new Date().getFullYear()} Decentraland</span>
      </div>
    </footer>
  );
}
