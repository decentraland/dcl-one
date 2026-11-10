import type { ReactNode } from "react";

import {
  SitesFooter,
  type FooterColumn,
  type FooterLink,
} from "@dcl/one-ui/apps/sites/chrome/SitesFooter";
import { SitesPageLayout } from "@dcl/one-ui/apps/sites/chrome/SitesPageLayout";
import { docsUrl } from "@ui/data/docs";
import { siteHost } from "@ui/data/site";
import { useChromeAuth } from "@ui/web/frames/chrome-auth";

import type { TrackContext } from "@/core/lib/telemetry/track";

import { useLocale } from "@/features/locale/LocaleProvider";
import { LOCALES, toLocale, type Locale } from "@/features/locale/locales";
import {
  trackFooterLinkClick,
  trackFooterSocialClick,
} from "@/features/components/chrome/chrome-telemetry";

import SitesNavbarConnected, { type SitesHeaderVariant } from "./sites-navbar";

// Each language in its own name, as source's footer lists them.
const LOCALE_LABELS: Record<Locale, string> = {
  en: "English",
  es: "Español",
  fr: "Français",
  zh: "中文",
  ko: "한국어",
  ja: "日本語",
};

// Source links these to docs.decentraland.org and decentraland.org/help; dcl-one serves the docs.
const HELP_DOC = "player/faqs/contact-and-support";

async function subscribe(email: string): Promise<void> {
  const response = await fetch("/v1/newsletter", {
    method: "POST",
    headers: { "content-type": "application/json", accept: "application/json" },
    body: JSON.stringify({ email, source: siteHost() }),
  });
  if (!response.ok) throw new Error(`newsletter returned ${response.status}`);
}

/** Which Sites header a page shows: a navbar variant, or none. */
export type SitesHeader =
  | { variant: "full"; overlay?: boolean }
  | { variant: "landing"; onJumpIn?: () => void }
  | { variant: "compact" }
  | "none";

/**
 * The source-matching chrome of the migrated Sites pages: the one Sites header and source's footer,
 * wired to this page's LocaleProvider. Must render inside that provider. It is the only place that
 * renders the connected navbar: a page declares which header it shows, as data, either one spec or
 * one per auth state, and never builds a header itself.
 */
export default function SitesPageChrome({
  header,
  session,
  children,
}: {
  header: SitesHeader | ((signedIn: boolean) => SitesHeader);
  /** The page's telemetry session, for the header's and the footer's link clicks. */
  session: TrackContext;
  children: ReactNode;
}) {
  const { t, text, locale, setLocale } = useLocale();
  // The shared chrome auth: seeded from the session cookie, so the server render picks the same header.
  const { signedIn } = useChromeAuth();
  const spec = typeof header === "function" ? header(signedIn) : header;
  // Source reports a footer link by its shown label, in the page's language.
  const shown = new Map<FooterLink, string>();
  const link = (key: string, href: string, external?: boolean): FooterLink => {
    const entry = { label: t(key), href, ...(external ? { external } : {}) };
    shown.set(entry, text(key));
    return entry;
  };

  const columns: FooterColumn[] = [
    {
      id: "getting-started",
      title: t("component.landing.footer.getting_started.title"),
      links: [
        link(
          "component.landing.footer.getting_started.what_is",
          docsUrl("player/introduction/about-decentraland"),
        ),
        link("component.landing.footer.getting_started.download", "/download"),
        link(
          "component.landing.footer.getting_started.system_requirements",
          docsUrl("player/decentraland-in-world/settings-and-performance"),
        ),
        link(
          "component.landing.footer.getting_started.faqs",
          docsUrl("player/faqs/decentraland-101"),
        ),
        link(
          "component.landing.footer.getting_started.contact_support",
          docsUrl(HELP_DOC),
        ),
      ],
    },
    {
      id: "resources",
      title: t("component.landing.footer.resources.title"),
      links: [
        link("component.landing.footer.resources.marketplace", "/shop"),
        link("component.landing.footer.resources.creator_hub", "/create"),
        link("component.landing.footer.resources.docs", docsUrl()),
        link("component.landing.footer.resources.blog", "/blog"),
        link(
          "component.landing.footer.resources.vote",
          "https://dao.decentraland.org",
          true,
        ),
      ],
    },
  ];

  const footer = (
    <SitesFooter
      wordmarkLabel={text("component.landing.footer.wordmark_alt")}
      newsletter={{
        title: t("component.landing.footer.newsletter.title"),
        emailLabel: text("component.landing.footer.newsletter.email_label"),
        placeholder: text("component.landing.footer.newsletter.placeholder"),
        submitLabel: t("component.landing.footer.newsletter.subscribe"),
        sendingLabel: t("component.landing.footer.newsletter.sending"),
        successText: t("component.landing.footer.newsletter.success"),
        invalidText: t("component.landing.footer.newsletter.invalid"),
        errorText: t("component.landing.footer.newsletter.error"),
        onSubscribe: subscribe,
      }}
      connectLabel={t("component.landing.footer.connect")}
      menuLabel={t("component.landing.footer.menu")}
      columns={columns}
      language={{
        listLabel: text("component.landing.footer.language.list"),
        triggerLabel: (language) =>
          text("component.landing.footer.language.trigger", { language }),
        locales: LOCALES.map((code) => ({ code, label: LOCALE_LABELS[code] })),
        current: locale,
        onChange: (code) => {
          const next = toLocale(code);
          if (next) void setLocale(next).catch(() => undefined);
        },
      }}
      legal={[
        {
          label: t("component.landing.footer.legal.privacy"),
          href: "/privacy",
        },
        { label: t("component.landing.footer.legal.terms"), href: "/terms" },
        {
          label: t("component.landing.footer.legal.content"),
          href: "/content",
        },
        { label: t("component.landing.footer.legal.ethics"), href: "/ethics" },
      ]}
      copyright={t("component.landing.footer.copyright", {
        year: new Date().getFullYear(),
      })}
      onLinkClick={(followed) =>
        trackFooterLinkClick(shown.get(followed) ?? "", session)
      }
      onSocialClick={(platform) => trackFooterSocialClick(platform, session)}
    />
  );

  return (
    <SitesPageLayout
      header={spec === "none" ? undefined : navbar(spec, session)}
      footer={footer}
      skipLabel={text("component.landing.footer.skip_to_content")}
    >
      {children}
    </SitesPageLayout>
  );
}

function navbar(
  spec: Exclude<SitesHeader, "none">,
  session: TrackContext,
): ReactNode {
  const variant: SitesHeaderVariant = spec.variant;
  return (
    <SitesNavbarConnected
      variant={variant}
      session={session}
      overlay={spec.variant === "full" ? spec.overlay : undefined}
      onJumpIn={spec.variant === "landing" ? spec.onJumpIn : undefined}
    />
  );
}
