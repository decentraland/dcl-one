import { useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { useNavigate } from "react-router";
import { DownloadSuccess } from "@dcl/one-ui/apps/sites/download/DownloadSuccess";
import { DownloadSuccessFooter } from "@dcl/one-ui/apps/sites/download/DownloadSuccessFooter";
import { parseCookies } from "@/core/lib/telemetry/session";
import { installerUrl } from "@/data/lib/download/installers";
import { installerSelection } from "@/features/download/installers";
import {
  detectDownloadBrowser,
  stepImages,
  type DownloadBrowser,
} from "@/features/download/steps";
import { successCopy } from "@/features/download/success-copy";
import { sitesFooterCopy } from "@dcl/one-ui/apps/sites/chrome/footer-copy";
import {
  SITES_LANGUAGES,
  SitesLocaleContext,
  saveSitesLocale,
  sitesLanguage,
} from "@ui/web/frames/sites-locale";
import { publicBase } from "../lib/social-meta";
import { staticPageMeta } from "../lib/static-page-meta";
import type { Route } from "./+types/download_success";

function linkText(message: string, link: ReactNode) {
  const [before, after = ""] = message.split("{link}");
  return (
    <>
      {before}
      {link}
      {after}
    </>
  );
}

// The card follows decentraland.org; the page itself stays out of the index.
export function meta({ matches }: Route.MetaArgs) {
  return staticPageMeta("/download_success", publicBase(matches), { noindex: true });
}

export function loader({ request }: Route.LoaderArgs) {
  const params = new URL(request.url).searchParams;
  const locale = sitesLanguage(
    params.get("lang") ?? parseCookies(request.headers.get("cookie")).lang,
  ).code;
  const selection = installerSelection(params);
  // The installers come from the host the deployment configures; without one there is nothing to start.
  const url = installerUrl(selection.os, selection.arch);
  if (!url) throw new Response(null, { status: 404, statusText: "Not Found" });
  return {
    ...selection,
    url,
    locale,
    year: new Date().getUTCFullYear(),
  };
}

export default function DownloadSuccessRoute({
  loaderData,
}: Route.ComponentProps) {
  const { os, url: href, locale, year } = loaderData;
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);
  const copy = successCopy[locale];
  const success = copy.success;
  const footer = sitesFooterCopy[locale];
  const navigate = useNavigate();
  const preference = useContext(SitesLocaleContext);
  const started = useRef<string | null>(null);
  const [browser, setBrowser] = useState<DownloadBrowser>("chrome");
  useEffect(() => {
    setBrowser(
      detectDownloadBrowser(navigator.userAgent, "brave" in navigator),
    );
  }, []);
  useEffect(() => {
    // Locale changes keep this attempt; a different artifact or a new mount starts another.
    const visit = href;
    if (started.current === visit) return;
    started.current = visit;
    const link = document.createElement("a");
    link.href = href;
    link.hidden = true;
    document.body.append(link);
    link.click();
    link.remove();
  }, [href]);
  const steps = success.steps[os === "windows" ? "windows" : "macOS"];
  return (
    <>
      <DownloadSuccess
        title={success.started.title}
        subtitle={success.started.subtitle}
        stepsLabel={success.started.subtitle}
        stepLabel={(n) => `${success.step} ${n}`}
        steps={stepImages(os, browser).map((image, index) => ({
          ...image,
          ...steps[`step${index + 1}` as keyof typeof steps],
        }))}
        footerLines={[
          {
            text: (link) => linkText(success.footer_retry, link),
            link: { label: success.footer_retry_link, href },
          },
          {
            text: (link) => linkText(success.footer_help, link),
            link: {
              label: success.footer_help_link,
              external: true,
              href: "/help",
            },
          },
        ]}
      />
      <DownloadSuccessFooter
        wordmarkLabel="Decentraland"
        newsletter={{
          title: footer.newsletter.title,
          src: "https://embeds.beehiiv.com/ff89783d-748b-4ba3-8700-4759f6f62831?slim=true",
        }}
        connectLabel={footer.connect}
        menuLabel={footer.menu}
        columns={[
          {
            id: "start",
            title: footer.getting_started.title,
            links: [
              {
                label: footer.getting_started.what_is,
                href: "https://docs.decentraland.org/introduction/about-decentraland",
              },
              { label: footer.getting_started.download, href: "/download" },
              {
                label: footer.getting_started.system_requirements,
                href: "https://docs.decentraland.org/in-world/settings-and-performance",
              },
              {
                label: footer.getting_started.faqs,
                href: "https://docs.decentraland.org/faqs/decentraland-101",
              },
              { label: footer.getting_started.contact_support, href: "/help" },
            ],
          },
          {
            id: "resources",
            title: footer.resources.title,
            links: [
              { label: footer.resources.marketplace, href: "/shop" },
              { label: footer.resources.creator_hub, href: "/create" },
              { label: footer.resources.docs, href: "/docs" },
              { label: footer.resources.blog, href: "/blog" },
              {
                label: footer.resources.vote,
                href: "https://decentraland.org/dao",
              },
            ],
          },
        ]}
        language={{
          listLabel: "Languages",
          triggerLabel: (label) => `Change language: ${label}`,
          locales: SITES_LANGUAGES,
          current: locale,
          onChange: (code) => {
            const nextLocale = sitesLanguage(code).code;
            saveSitesLocale(nextLocale);
            preference?.onSelect(nextLocale);
            const url = new URL(window.location.href);
            url.searchParams.set("lang", nextLocale);
            void navigate(`${url.pathname}${url.search}`);
          },
        }}
        legal={[
          { label: "Privacy Policy", href: "/privacy" },
          { label: "Terms of Use", href: "/terms" },
          { label: "Content Policy", href: "/content" },
          { label: "Code of Ethics", href: "/ethics" },
        ]}
        copyright={`© ${year} Decentraland`}
      />
    </>
  );
}
