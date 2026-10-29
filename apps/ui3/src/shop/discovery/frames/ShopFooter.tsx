// from: shop@2d3bf62:app/src/components/ShopFooter/ShopFooter.tsx

import { useState, type FormEvent, type ReactNode } from "react";

import "./shopfooter.css";

type FooterLink = { label: string; url: string };

const GETTING_STARTED: FooterLink[] = [
  { label: "What is Decentraland", url: "https://docs.decentraland.org/introduction/about-decentraland" },
  { label: "Download", url: "/download" },
  { label: "System Requirements", url: "https://docs.decentraland.org/in-world/settings-and-performance" },
  { label: "FAQs", url: "https://docs.decentraland.org/faqs/decentraland-101" },
  { label: "Contact Support", url: "https://decentraland.org/help/" },
];

const RESOURCES: FooterLink[] = [
  { label: "Marketplace", url: "/shop" },
  { label: "Creator Hub", url: "/create" },
  { label: "Docs", url: "/docs" },
  { label: "Blog", url: "/blog" },
];

const LEGAL: FooterLink[] = [
  { label: "Privacy Policy", url: "/privacy" },
  { label: "Terms of Use", url: "/terms" },
  { label: "Content Policy", url: "/content-policy" },
  { label: "Code of Ethics", url: "/ethics" },
];

const external = (url: string) => /^https?:/.test(url);
const linkProps = (url: string) => (external(url) ? { href: url, target: "_blank", rel: "noopener noreferrer" } : { href: url });

function Svg({ w, h = w, box = "0 0 24 24", d }: { w: number; h?: number; box?: string; d: string }) {
  return (
    <svg viewBox={box} fill="currentColor" width={w} height={h} aria-hidden>
      <path d={d} />
    </svg>
  );
}

const SOCIAL: Array<{ name: string; url: string; icon: ReactNode }> = [
  {
    name: "Discord",
    url: "https://dcl.gg/discord",
    icon: <Svg w={26} h={20} box="0 0 28 22" d="M23.7 1.84A23.25 23.25 0 0 0 17.96 0c-.25.45-.54 1.05-.74 1.53a21.5 21.5 0 0 0-6.45 0C10.57 1.05 10.27.45 10.02 0A23.2 23.2 0 0 0 4.27 1.85C.62 7.34-.38 12.7.12 18a23.37 23.37 0 0 0 7.13 3.6 17.4 17.4 0 0 0 1.53-2.49 15.17 15.17 0 0 1-2.41-1.16c.2-.15.4-.3.59-.46a16.63 16.63 0 0 0 14.1 0c.19.16.39.31.58.46-.77.45-1.57.84-2.41 1.16.44.87.96 1.7 1.53 2.49a23.3 23.3 0 0 0 7.14-3.61c.58-6.15-1-11.47-4.1-16.15ZM9.35 14.74c-1.4 0-2.54-1.28-2.54-2.85s1.12-2.86 2.54-2.86 2.56 1.29 2.54 2.86c0 1.57-1.12 2.85-2.54 2.85Zm9.38 0c-1.4 0-2.54-1.28-2.54-2.85s1.12-2.86 2.54-2.86 2.56 1.29 2.54 2.86c0 1.57-1.12 2.85-2.54 2.85Z" />,
  },
  {
    name: "GitHub",
    url: "https://github.com/decentraland",
    icon: <Svg w={24} d="M12 .3a12 12 0 0 0-3.8 23.38c.6.12.82-.26.82-.58v-2.02c-3.34.73-4.04-1.61-4.04-1.61-.55-1.39-1.34-1.76-1.34-1.76-1.08-.74.08-.73.08-.73 1.2.09 1.84 1.24 1.84 1.24 1.07 1.83 2.8 1.3 3.49 1 .1-.78.42-1.3.76-1.6-2.67-.31-5.47-1.34-5.47-5.93 0-1.31.47-2.38 1.24-3.22-.13-.3-.54-1.52.12-3.18 0 0 1-.33 3.3 1.23a11.5 11.5 0 0 1 6.02 0c2.28-1.56 3.29-1.23 3.29-1.23.66 1.66.25 2.88.12 3.18a4.65 4.65 0 0 1 1.24 3.22c0 4.61-2.81 5.62-5.48 5.92.43.37.81 1.1.81 2.22v3.29c0 .32.22.7.82.58A12 12 0 0 0 12 .3Z" />,
  },
  {
    name: "X",
    url: "https://x.com/decentraland",
    icon: <Svg w={22} d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231 5.45-6.231Zm-1.161 17.52h1.833L7.084 4.126H5.117L17.083 19.77Z" />,
  },
  {
    name: "Instagram",
    url: "https://instagram.com/decentraland_foundation/",
    icon: <Svg w={24} d="M12 2.163c3.204 0 3.584.012 4.85.07 3.252.148 4.771 1.691 4.919 4.919.058 1.265.069 1.645.069 4.849 0 3.205-.012 3.584-.069 4.849-.149 3.225-1.664 4.771-4.919 4.919-1.266.058-1.644.07-4.85.07-3.204 0-3.584-.012-4.849-.07-3.26-.149-4.771-1.699-4.919-4.92-.058-1.265-.07-1.644-.07-4.849 0-3.204.013-3.583.07-4.849.149-3.227 1.664-4.771 4.919-4.919 1.266-.057 1.645-.069 4.849-.069ZM12 0C8.741 0 8.333.014 7.053.072 2.695.272.273 2.69.073 7.052.014 8.333 0 8.741 0 12c0 3.259.014 3.668.072 4.948.2 4.358 2.618 6.78 6.98 6.98C8.333 23.986 8.741 24 12 24c3.259 0 3.668-.014 4.948-.072 4.354-.2 6.782-2.618 6.979-6.98.059-1.28.073-1.689.073-4.948 0-3.259-.014-3.667-.072-4.947-.196-4.354-2.617-6.78-6.979-6.98C15.668.014 15.259 0 12 0Zm0 5.838a6.162 6.162 0 1 0 0 12.324 6.162 6.162 0 0 0 0-12.324ZM12 16a4 4 0 1 1 0-8 4 4 0 0 1 0 8Zm6.406-11.845a1.44 1.44 0 1 0 0 2.881 1.44 1.44 0 0 0 0-2.881Z" />,
  },
  {
    name: "YouTube",
    url: "https://youtube.com/@decentraland_foundation",
    icon: <Svg w={24} d="M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.546 12 3.546 12 3.546s-7.505 0-9.377.504A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.504 9.376.504 9.376.504s7.505 0 9.377-.504a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814ZM9.545 15.568V8.432L15.818 12l-6.273 3.568Z" />,
  },
  {
    name: "TikTok",
    url: "https://tiktok.com/@decentraland_fdn",
    icon: <Svg w={22} d="M12.525.02c1.31-.02 2.61-.01 3.91-.02.08 1.53.63 3.09 1.75 4.17 1.12 1.11 2.7 1.62 4.24 1.79v4.03c-1.44-.05-2.89-.35-4.2-.97-.57-.26-1.1-.59-1.62-.93-.01 2.92.01 5.84-.02 8.75-.08 1.4-.54 2.79-1.35 3.94-1.31 1.92-3.58 3.17-5.91 3.21-1.43.08-2.86-.31-4.08-1.03-2.02-1.19-3.44-3.37-3.65-5.71-.02-.5-.03-1-.01-1.49.18-1.9 1.12-3.72 2.58-4.96 1.66-1.44 3.98-2.13 6.15-1.72.02 1.48-.04 2.96-.04 4.44-.99-.32-2.15-.23-3.02.37-.63.41-1.11 1.04-1.36 1.75-.21.51-.15 1.07-.14 1.61.24 1.64 1.82 3.02 3.5 2.87 1.12-.01 2.19-.66 2.77-1.61.19-.33.4-.67.41-1.06.1-1.79.06-3.57.07-5.36.01-4.03-.01-8.05.02-12.07Z" />,
  },
  {
    name: "LinkedIn",
    url: "https://linkedin.com/company/decentralandorg",
    icon: <Svg w={24} d="M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286ZM5.337 7.433a2.062 2.062 0 1 1 0-4.123 2.062 2.062 0 0 1 0 4.123ZM6.863 20.452H3.804V9h3.059v11.452ZM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003Z" />,
  },
];

function Chevron({ open }: { open: boolean }) {
  return (
    <span className="shop-footer__chev" data-open={open || undefined}>
      <Svg w={18} d="M7.41 8.59 12 13.17l4.59-4.58L18 10l-6 6-6-6 1.41-1.41Z" />
    </span>
  );
}

const EMAIL_RE = /^[^\s@,;<>]+@[^\s@,;<>]+\.[^\s@,;<>]+$/;
type NewsState = { phase: "idle" | "sending" | "ok" } | { phase: "error"; message: string };

function Newsletter() {
  const [email, setEmail] = useState("");
  const [state, setState] = useState<NewsState>({ phase: "idle" });
  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const value = email.trim();
    if (!EMAIL_RE.test(value)) {
      setState({ phase: "error", message: "Enter a valid email address." });
      return;
    }
    setState({ phase: "sending" });
    fetch("/v1/newsletter", {
      method: "POST",
      headers: { "content-type": "application/json", accept: "application/json" },
      body: JSON.stringify({ email: value, source: "Shop" }),
    })
      .then((res) => {
        if (!res.ok) throw new Error(`newsletter returned ${res.status}`);
        setState({ phase: "ok" });
        setEmail("");
      })
      .catch(() => setState({ phase: "error", message: "Something went wrong. Please try again." }));
  };
  return (
    <form className="shop-footer__newsform" onSubmit={submit} noValidate data-testid="shop-footer-newsletter">
      <input
        type="email"
        name="email"
        autoComplete="email"
        placeholder="Enter your email"
        aria-label="Email address"
        value={email}
        onChange={(e) => {
          setEmail(e.target.value);
          if (state.phase !== "sending") setState({ phase: "idle" });
        }}
      />
      <button type="submit" disabled={state.phase === "sending"}>
        Subscribe
      </button>
      <span className="shop-footer__newsstatus" role="status" data-error={state.phase === "error" || undefined}>
        {state.phase === "ok" ? "Thanks! You're subscribed." : state.phase === "error" ? state.message : ""}
      </span>
    </form>
  );
}

function SocialRow() {
  return (
    <div className="shop-footer__social">
      {SOCIAL.map(({ name, url, icon }) => (
        <a key={name} href={url} target="_blank" rel="noopener noreferrer" aria-label={name}>
          {icon}
        </a>
      ))}
    </div>
  );
}

export default function ShopFooter() {
  const [open, setOpen] = useState<string | null>(null);
  const sections = [
    { key: "getting-started", label: "Getting Started", links: GETTING_STARTED },
    { key: "resources", label: "Resources", links: RESOURCES },
  ];
  return (
    <footer className="shop-footer" data-testid="shop-footer">
      <div className="shop-footer__main">
        <div className="shop-footer__left">
          <span className="shop-footer__wordmark">Decentraland</span>
          <div className="shop-footer__newsblock">
            <p className="shop-footer__news">Get the weekly highlights in your inbox</p>
            <Newsletter />
          </div>
          <div className="shop-footer__connect" data-variant="desktop">
            <p className="shop-footer__label">Connect</p>
            <SocialRow />
          </div>
        </div>
        <div className="shop-footer__right">
          {sections.map((s) => (
            <div className="shop-footer__col" key={s.key}>
              <p className="shop-footer__label">{s.label}</p>
              {s.links.map((l) => (
                <a key={l.label} className="shop-footer__link" {...linkProps(l.url)}>
                  {l.label}
                </a>
              ))}
            </div>
          ))}
        </div>
        <div className="shop-footer__menu">
          <p className="shop-footer__menulabel">Menu</p>
          {sections.map((s) => (
            <div key={s.key}>
              <button type="button" className="shop-footer__drop" aria-expanded={open === s.key} onClick={() => setOpen((o) => (o === s.key ? null : s.key))}>
                {s.label}
                <Chevron open={open === s.key} />
              </button>
              <div className="shop-footer__dropcontent" data-open={open === s.key || undefined}>
                {s.links.map((l) => (
                  <a key={l.label} className="shop-footer__mlink" {...linkProps(l.url)}>
                    {l.label}
                  </a>
                ))}
              </div>
            </div>
          ))}
        </div>
        <div className="shop-footer__connect" data-variant="mobile">
          <p className="shop-footer__label">Connect</p>
          <SocialRow />
        </div>
      </div>
      <div className="shop-footer__bottom">
        <div className="shop-footer__bottomleft">
          <span className="shop-footer__lang">
            <span aria-hidden>🇺🇸</span>
            English
            <span className="shop-footer__langchev" aria-hidden>
              <Svg w={18} d="M7.41 8.59 12 13.17l4.59-4.58L18 10l-6 6-6-6 1.41-1.41Z" />
            </span>
          </span>
          <nav className="shop-footer__legal" aria-label="Legal">
            {LEGAL.map((l) => (
              <a key={l.label} href={l.url}>
                {l.label}
              </a>
            ))}
          </nav>
        </div>
        <span className="shop-footer__copy">&copy; {new Date().getFullYear()} Decentraland</span>
      </div>
    </footer>
  );
}
