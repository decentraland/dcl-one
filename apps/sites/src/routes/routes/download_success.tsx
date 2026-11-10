import { useEffect, useMemo, useState } from "react";
import { useLocation } from "react-router";

import { DownloadSuccess } from "@dcl/one-ui/apps/sites/download/DownloadSuccess";
import { docsUrl } from "@ui/data/docs";

import { sidLoader } from "@/core/lib/telemetry/page-loader";
import { downloadGatewayUrl } from "@/data/lib/catalyst/bases";
import { isModifiedClick } from "@/features/download/action-props";
import { cdnInstallerUrl } from "@/features/download/links";
import {
  detectDownloadBrowser,
  stepImages,
  type DownloadBrowser,
} from "@/features/download/steps";
import { useDownloadSuccess } from "@/features/download/useDownloadSuccess";
import {
  loadLocaleMessages,
  type LocaleMessages,
} from "@/features/locale/catalogs";
import { LocaleProvider, useLocale } from "@/features/locale/LocaleProvider";
import { resolveRequestLocale } from "@/features/locale/locales";

import SitesPageChrome from "../lib/sites-page-chrome";
import type { Route } from "./+types/download_success";

const STORY = "download";
// Source Sites links "need help" to its help page; dcl-one serves the equivalent docs page.
const HELP_DOC = "player/faqs/contact-and-support";

export function meta() {
  return [{ title: "Decentraland" }, { name: "robots", content: "noindex" }];
}

export async function loader({ request }: Route.LoaderArgs) {
  const { sid, wrap } = sidLoader(request);
  const messages = await loadLocaleMessages(resolveRequestLocale(request));
  return wrap({ sid, gateway: downloadGatewayUrl() ?? null, messages });
}

type LoaderData = {
  sid: string;
  gateway: string | null;
  messages: LocaleMessages;
};

export default function DownloadSuccessRoute({
  loaderData,
}: Route.ComponentProps) {
  const { sid, gateway, messages } = loaderData as LoaderData;
  const location = useLocation();
  const session = useMemo(() => ({ sid, story: STORY }), [sid]);
  return (
    <LocaleProvider initial={messages}>
      {/* Keyed by navigation: each navigation, back/forward included, is one download. */}
      {/* As source: no navbar here; the page's own logo and title head it, then the footer. */}
      <SitesPageChrome session={session} header="none">
        <DownloadSuccessPage
          key={location.key}
          sid={sid}
          gateway={gateway ?? undefined}
          search={location.search}
        />
      </SitesPageChrome>
    </LocaleProvider>
  );
}

function DownloadSuccessPage({
  sid,
  gateway,
  search,
}: {
  sid: string;
  gateway?: string;
  search: string;
}) {
  const { t, text } = useLocale();
  const session = useMemo(() => ({ sid, story: STORY }), [sid]);
  const { os, arch, state, redownload } = useDownloadSuccess(
    search,
    session,
    gateway,
  );
  // Only picks the step 1 screenshot; read after hydration so the server render matches.
  const [browser, setBrowser] = useState<DownloadBrowser>("chrome");
  useEffect(() => {
    setBrowser(
      detectDownloadBrowser(navigator.userAgent, "brave" in navigator),
    );
  }, []);
  const images = stepImages(os, browser);
  const steps = images.map((image, index) => {
    const prefix = `page.download.success.steps.${os === "windows" ? "windows" : "macOS"}.step${index + 1}`;
    return {
      ...image,
      imageAlt: text(`page.download.success.steps.image_alt.step${index + 1}`),
      title: t(`${prefix}.title`),
      text: t(`${prefix}.text`),
    };
  });

  return (
    <DownloadSuccess
      title={t("page.download.success.started.title")}
      subtitle={t("page.download.success.started.subtitle")}
      stepsLabel={text("page.download.success.started.subtitle")}
      stepLabel={(step) => `${text("page.download.success.step")} ${step}`}
      steps={steps}
      downloading={
        state.phase === "downloading"
          ? {
              label: text("page.download.downloading"),
              progress: state.progress,
            }
          : null
      }
      footerLines={[
        {
          text: (link) => t("page.download.success.footer_retry", { link }),
          // The static installer is the no-JS fallback; with JS a plain click re-runs the download,
          // and a modified click (new tab or window) is left to the browser.
          link: {
            label: t("page.download.success.footer_retry_link"),
            href: cdnInstallerUrl(os, arch),
            onClick: (event) => {
              if (isModifiedClick(event)) return;
              event.preventDefault();
              redownload();
            },
          },
        },
        {
          text: (link) => t("page.download.success.footer_help", { link }),
          link: {
            label: t("page.download.success.footer_help_docs_link"),
            href: docsUrl(HELP_DOC),
          },
        },
      ]}
    />
  );
}
