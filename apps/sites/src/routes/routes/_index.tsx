import { useEffect, useMemo, useRef, useState } from "react";
import { useLocation } from "react-router";

import { DownloadDialog } from "@dcl/one-ui/apps/sites/download/DownloadDialog";
import { DownloadQRDialog } from "@dcl/one-ui/apps/sites/download/DownloadQRDialog";
import { CatchTheVibe } from "@dcl/one-ui/apps/sites/home/CatchTheVibe";
import { ComeHangOut } from "@dcl/one-ui/apps/sites/home/ComeHangOut";
import { Hero } from "@dcl/one-ui/apps/sites/home/Hero";
import { WeeklyRituals } from "@dcl/one-ui/apps/sites/home/WeeklyRituals";
import { WhatsOn } from "@dcl/one-ui/apps/sites/home/WhatsOn";

import AnimatedBackground from "@ui/shop/discovery/components/animated-background/AnimatedBackground";
import { useChromeAuth } from "@ui/web/frames/chrome-auth";

import { sidLoader } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { useProfileIdentity } from "@/data/lib/auth/use-profile-name";
import { loadHome } from "@/data/lib/catalyst/landings/home.server";
import type { WhatsOnCard } from "@/data/lib/catalyst/landings/events.server";
import {
  downloadActionsProps,
  storeBadgeProps,
  type ActionPlaces,
} from "@/features/download/action-props";
import { launchDesktopApp } from "@/features/download/launch";
import { trackSectionClick } from "@/features/download/telemetry";
import {
  detectPlatform,
  downloadOffer,
  viewportOffers,
  type DetectedPlatform,
} from "@/features/download/options";
import { useDownloadCtas } from "@/features/download/useDownloadCtas";
import { VIBE_CLIPS, WEEKLY_RITUALS } from "@/features/home/content";
import {
  loadLocaleMessages,
  type LocaleMessages,
} from "@/features/locale/catalogs";
import { LocaleProvider, useLocale } from "@/features/locale/LocaleProvider";
import { resolveRequestLocale } from "@/features/locale/locales";

import SitesPageChrome from "../lib/sites-page-chrome";
import type { Route } from "./+types/_index";
import { eventDetailHref } from "@/core/lib/router/event-href";

export const handle = {
  agentMarkdown: "discover",
} satisfies AgentMarkdownHandle;

const STORY = "discover";
// Click places as source Sites sends them (its CTA data-place attributes): the installer Click and
// store badges name the section, while /download_success carries the DownloadPlace.
const HERO_PLACES: ActionPlaces = {
  installer: "landing-hero",
  installerClick: "Landing Hero",
  platformSwitch: "landing-hero-platform-switch",
  epic: "landing-hero-epic",
  store: "Landing Hero",
};
const HANG_OUT_PLACES: ActionPlaces = {
  installer: "come-hang-out",
  installerClick: "come-hang-out",
  platformSwitch: "come-hang-out-platform-switch",
  epic: "come-hang-out",
  store: "Landing Come Hang Out",
};
const DIALOG_PLACES: ActionPlaces = {
  installer: "download-modal",
  installerClick: "download-modal",
  platformSwitch: "download-modal",
  epic: "download-modal",
  store: "download-modal",
};

export function meta() {
  return [{ title: "Decentraland" }];
}

export async function loader({ request }: Route.LoaderArgs) {
  const { sid, wrap } = sidLoader(request);
  const [home, messages] = await Promise.all([
    loadHome(),
    loadLocaleMessages(resolveRequestLocale(request)),
  ]);
  return wrap({
    sid,
    messages,
    whatsOn: home.whatsOn,
    downloadsLabel: home.downloadsLabel,
    platform: detectPlatform(request.headers.get("user-agent")),
  });
}

type LoaderData = {
  sid: string;
  messages: LocaleMessages;
  whatsOn: WhatsOnCard[];
  downloadsLabel: string;
  platform: DetectedPlatform;
};

export default function Home({ loaderData }: Route.ComponentProps) {
  const data = loaderData as LoaderData;
  const location = useLocation();

  const viewed = useRef(false);
  useEffect(() => {
    if (viewed.current) return;
    viewed.current = true;
    track("lp_discover_viewed", {}, { sid: data.sid, story: STORY });
  }, [data.sid]);

  return (
    <LocaleProvider initial={data.messages}>
      <HomeView {...data} search={location.search} />
    </LocaleProvider>
  );
}

function HomeView({
  sid,
  whatsOn,
  downloadsLabel,
  platform,
  search,
}: LoaderData & { search: string }) {
  const { t, text } = useLocale();
  // The current session: the cookie's on the server, the live one after hydration, so signing out
  // or an expired cookie shows the signed-out header without a reload.
  const { signedIn } = useChromeAuth();
  const i18n = { t, text };
  const session = useMemo(() => ({ sid, story: STORY }), [sid]);
  const ctas = useDownloadCtas(search, session);
  const offer = downloadOffer(platform);
  const campaign = ctas.request.campaign;
  const [qr, setQr] = useState<"ios" | "android" | null>(null);
  const alan = useProfileIdentity(VIBE_CLIPS[0].address, true);
  const roustan = useProfileIdentity(VIBE_CLIPS[1].address, true);
  const avatars = [alan.avatarUrl, roustan.avatarUrl];
  const downloads = t("page.download.total_downloads", {
    downloads: downloadsLabel,
  });
  const actionsFor = (places: ActionPlaces) =>
    downloadActionsProps({
      offer,
      ctas,
      i18n,
      places,
      downloadsLabel,
      campaign,
      openQr: setQr,
    });
  const storeFor = (places: ActionPlaces) =>
    storeBadgeProps({ offer, ctas, i18n, place: places.store, campaign });
  const sectionClick = (
    place: string,
    detail: { section?: string; card?: string },
  ) => trackSectionClick({ place, ...detail, campaign }, session);
  const [installFirst, setInstallFirst] = useState(false);
  // As source useHangOutAction: signed in, try the installed app; otherwise, or when none answers,
  // offer the download.
  const openHangOut = async () => {
    if (!signedIn) return setInstallFirst(true);
    const launched = await launchDesktopApp({
      ...ctas.request.flags,
      mobile: platform.kind === "mobile",
    }).catch(() => false);
    if (!launched) setInstallFirst(true);
  };
  const hangOut = () => {
    sectionClick("Landing Catch The Vibe", {});
    void openHangOut();
  };
  const carouselLabels = {
    previous: text("page.home.carousel.previous"),
    next: text("page.home.carousel.next"),
    goTo: (slide: number) => text("page.home.carousel.go_to", { slide }),
    position: (slide: number, total: number) =>
      text("page.home.carousel.position", { slide, total }),
  };
  // The hero picks its layout by viewport width, as source does: installers wide, a store narrow.
  const heroOffers = viewportOffers(platform);
  const heroActions = downloadActionsProps({
    offer: heroOffers.wide,
    ctas,
    i18n,
    places: HERO_PLACES,
    downloadsLabel,
    campaign,
    openQr: setQr,
  });
  const heroStore = storeBadgeProps({
    offer: { ...heroOffers.wide, store: heroOffers.narrowStore },
    ctas,
    i18n,
    place: HERO_PLACES.store,
    campaign,
  });

  const page = (
    <main>
      <Hero
        title={t("page.home.hero.title")}
        imageAlt={text("page.home.hero.image_alt")}
        actions={heroActions}
        mobile={
          heroStore
            ? {
                title: t("page.home.hero.mobile_android_title"),
                subtitle: t("page.home.hero.mobile_android_subtitle"),
                store: heroStore,
              }
            : undefined
        }
      >
        {qr ? (
          <DownloadQRDialog
            os={qr}
            title={text("component.download_qr.title", {
              os: text(
                `page.download.store.${qr === "ios" ? "ios" : "google_play"}`,
              ),
            })}
            qrLabel={text("component.download_qr.code_label", {
              os: text(
                `page.download.store.${qr === "ios" ? "ios" : "google_play"}`,
              ),
            })}
            downloads={downloads}
            closeLabel={text("component.modal.close")}
            onClose={() => setQr(null)}
          />
        ) : null}
      </Hero>
      <WhatsOn
        title={text("page.home.whats_on.title")}
        labels={{
          live: text("page.home.whats_on.live"),
          by: text("page.home.whats_on.by"),
        }}
        viewAll={{
          label: text("page.home.whats_on.view_all"),
          href: "/events",
          onClick: () =>
            sectionClick("Landing Explore", { section: "view_all" }),
        }}
        onCardClick={(card) =>
          sectionClick("Landing Explore", { card: card.title })
        }
        cards={whatsOn.map((card) => ({
          id: card.id,
          title: card.title,
          image: card.image,
          href: card.href,
          users: card.users,
          usersLabel: text("page.home.whats_on.users", { count: card.users }),
          ...(card.creatorName || card.creatorAddress
            ? {
                creator: {
                  name:
                    card.creatorName ??
                    `${card.creatorAddress!.slice(0, 6)}…${card.creatorAddress!.slice(-4)}`,
                  ...(card.creatorAddress
                    ? { href: `/profile/${card.creatorAddress}` }
                    : {}),
                  ...(card.creatorAvatarUrl
                    ? { avatarUrl: card.creatorAvatarUrl }
                    : {}),
                  ...(card.isGenesisPlaza ? { foundation: true } : {}),
                },
              }
            : {}),
        }))}
      />
      <CatchTheVibe
        title={text("page.home.catch_the_vibe.title")}
        labels={{
          play: text("page.home.catch_the_vibe.play"),
          pause: text("page.home.catch_the_vibe.pause"),
          mute: text("page.home.catch_the_vibe.mute"),
          unmute: text("page.home.catch_the_vibe.unmute"),
        }}
        carouselLabels={carouselLabels}
        hangOut={{ label: text("page.home.hang_out_now"), onClick: hangOut }}
        items={VIBE_CLIPS.map((clip, index) => ({
          id: clip.id,
          image: clip.image,
          video: clip.video,
          creator: {
            name: clip.name,
            href: `/profile/${clip.address}`,
            ...(avatars[index] ? { avatarUrl: avatars[index] } : {}),
          },
        }))}
      />
      <WeeklyRituals
        title={text("page.home.weekly_rituals.title")}
        labels={carouselLabels}
        onCardClick={(card) =>
          sectionClick("Landing Weekly Rituals", { card: card.id })
        }
        cards={WEEKLY_RITUALS.map((ritual) => ({
          id: ritual.id,
          title: text(ritual.titleKey),
          href: eventDetailHref(ritual.eventId),
          image: ritual.image,
          mobileImage: ritual.mobileImage,
        }))}
      />
      <ComeHangOut
        background={<AnimatedBackground />}
        title={text("page.home.come_hang_out.title")}
        actions={actionsFor(HANG_OUT_PLACES)}
        store={storeFor(HANG_OUT_PLACES)}
      />
      {installFirst ? (
        <DownloadDialog
          title={text("component.download_dialog.title")}
          actions={downloadActionsProps({
            offer,
            ctas,
            i18n,
            places: DIALOG_PLACES,
            downloadsLabel,
            campaign,
          })}
          store={storeFor(DIALOG_PLACES)}
          closeLabel={text("component.modal.close")}
          onClose={() => setInstallFirst(false)}
        />
      ) : null}
    </main>
  );

  return (
    <SitesPageChrome
      session={session}
      // As source: signed in, the full navbar, transparent over the hero; signed out, the landing header.
      header={(signedIn) =>
        signedIn
          ? { variant: "full", overlay: true }
          : { variant: "landing", onJumpIn: () => void openHangOut() }
      }
    >
      {page}
    </SitesPageChrome>
  );
}
