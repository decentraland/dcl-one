import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import { useLocation } from "react-router";

import { DownloadPage } from "@dcl/one-ui/apps/sites/download/DownloadPage";

import { sidLoader } from "@/core/lib/telemetry/page-loader";
import { useProfileIdentity } from "@/data/lib/auth/use-profile-name";
import { readWallet } from "@/data/lib/auth/wallet-cookie";
import { catalystBase } from "@/data/lib/catalyst/client";
import { loadDownloadsLabel } from "@/data/lib/catalyst/landings/download-counts.server";
import { useChromeAuth } from "@ui/web/frames/chrome-auth";

import {
  downloadActionsProps,
  storeBadgeProps,
  type ActionPlaces,
} from "@/features/download/action-props";
import {
  detectPlatform,
  viewportOffers,
  type DetectedPlatform,
} from "@/features/download/options";
import { collectCampaignParams } from "@/features/download/params";
import { useDownloadPage } from "@/features/download/useDownloadPage";
import {
  loadLocaleMessages,
  type LocaleMessages,
} from "@/features/locale/catalogs";
import { LocaleProvider, useLocale } from "@/features/locale/LocaleProvider";
import { resolveRequestLocale } from "@/features/locale/locales";

import SitesPageChrome from "../lib/sites-page-chrome";
import type { Route } from "./+types/download";

// dcl-one's own avatar renderer, loaded only for signed-in visitors.
const WearablePreview = lazy(
  () => import("@ui/wearable-preview/WearablePreview"),
);

const STORY = "download";
// Every CTA on /download reports this page as its place, as source does.
const PLACES: ActionPlaces = {
  installer: "download-page",
  installerClick: "download-page",
  platformSwitch: "download-page",
  epic: "download-page",
  store: "download-page",
};
const ADDRESS_RE = /^0x[0-9a-f]{40}$/i;

export function meta() {
  return [{ title: "Download Decentraland" }];
}

export async function loader({ request }: Route.LoaderArgs) {
  const { sid, wrap } = sidLoader(request);
  // The onboarding link names the account through ?user= (source DownloadLayout.tsx:75-89).
  const user = new URL(request.url).searchParams.get("user");
  const [messages, downloadsLabel] = await Promise.all([
    loadLocaleMessages(resolveRequestLocale(request)),
    loadDownloadsLabel(),
  ]);
  return wrap({
    sid,
    messages,
    downloadsLabel,
    platform: detectPlatform(request.headers.get("user-agent")),
    account:
      (user && ADDRESS_RE.test(user) ? user.toLowerCase() : null) ??
      readWallet(request),
  });
}

type LoaderData = {
  sid: string;
  messages: LocaleMessages;
  downloadsLabel: string;
  platform: DetectedPlatform;
  account: string | null;
};

export default function DownloadRoute({ loaderData }: Route.ComponentProps) {
  const data = loaderData as LoaderData;
  const location = useLocation();
  // The current session (cookie on the server, live after hydration), not the loader's: signing out
  // or an expired cookie brings the signed-out header back without a reload. The onboarding ?user=
  // only names an account to preview, it signs nobody in.
  const { signedIn } = useChromeAuth();
  const session = useMemo(() => ({ sid: data.sid, story: STORY }), [data.sid]);
  return (
    <LocaleProvider initial={data.messages}>
      {/* As source: signed in, the full navbar; signed out, the compact header, its logo and Sign In. */}
      <SitesPageChrome
        session={session}
        header={(signedIn) => ({ variant: signedIn ? "full" : "compact" })}
      >
        <DownloadView {...data} signedIn={signedIn} search={location.search} />
      </SitesPageChrome>
    </LocaleProvider>
  );
}

function DownloadView({
  sid,
  downloadsLabel,
  platform,
  account,
  signedIn,
  search,
}: LoaderData & { signedIn: boolean; search: string }) {
  const { t, text } = useLocale();
  const session = useMemo(() => ({ sid, story: STORY }), [sid]);
  const mobile = platform.kind === "mobile";
  const actions = useDownloadPage(search, session, mobile);
  // Both layouts render and the viewport width picks one, as source does: installers wide, a store
  // narrow.
  const offers = viewportOffers(platform);
  const profile = useProfileIdentity(account, !!account);
  const campaign = useMemo(
    () => collectCampaignParams(new URLSearchParams(search)),
    [search],
  );
  // The preview needs the browser (WebGL); render it after hydration.
  const [hydrated, setHydrated] = useState(false);
  useEffect(() => setHydrated(true), []);

  const i18n = { t, text };
  const actionProps = downloadActionsProps({
    offer: offers.wide,
    ctas: actions.ctas,
    i18n,
    places: PLACES,
    downloadsLabel,
    campaign,
  });

  return (
    <DownloadPage
      title={t("page.download.download_onboarding_title")}
      preTitle={
        account
          ? t("page.download.pre_title", {
              name: profile.name || text("page.download.your_account"),
            })
          : undefined
      }
      reserveHeaderRow={!signedIn}
      jumpIn={{
        text: t("page.download.already_downloaded"),
        label: t("page.download.jump_in"),
        onClick: () => void actions.jumpIn(),
      }}
      {...actionProps}
      store={storeBadgeProps({
        offer: { ...offers.wide, store: offers.narrowStore },
        ctas: actions.ctas,
        i18n,
        place: PLACES.store,
        campaign,
      })}
      preview={
        account && hydrated ? (
          <Suspense fallback={null}>
            <WearablePreview
              profile={account}
              base={catalystBase()}
              pauseOffscreen
            />
          </Suspense>
        ) : undefined
      }
      launcherMissing={
        actions.launcherMissing
          ? {
              titleLines: [
                t("page.download.modal.title_first_line"),
                t("page.download.modal.title_second_line"),
              ],
              cta: t("page.download.modal.cta"),
              closeLabel: text("component.modal.close"),
              onClose: actions.dismissLauncherMissing,
            }
          : null
      }
    />
  );
}
