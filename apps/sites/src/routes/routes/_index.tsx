import { useContext, useEffect, useRef } from "react";

import SitesLayout from "@/features/components/chrome/SitesLayout";
import { SitesLocaleContext, sitesLanguage, type SitesLocaleHandle } from "@dcl/one-ui/apps/sites/chrome/sites-locale";
import { homeCopy } from "@/features/home/locale";
import { sitesFooterCopy } from "@dcl/one-ui/apps/sites/chrome/footer-copy";
import { parseCookies } from "@/core/lib/telemetry/session";
import { AnimatedBackground } from "@dcl/one-ui/apps/sites/shared/AnimatedBackground/AnimatedBackground";
import { Hero } from "@dcl/one-ui/apps/sites/home/Hero";
import { WhatsOn } from "@dcl/one-ui/apps/sites/home/WhatsOn";
import { CatchTheVibe } from "@dcl/one-ui/apps/sites/home/CatchTheVibe";
import { WeeklyRituals } from "@dcl/one-ui/apps/sites/home/WeeklyRituals";
import { ComeHangOut } from "@dcl/one-ui/apps/sites/home/ComeHangOut";
import { homeEventHref } from "@/features/home/links";
import { homeDownloadActions } from "@/features/home/actions";
import { googlePlayHref } from "@/features/download/google-play";
import { withDownloadClicks } from "@/features/home/download-clicks";
import { trackDownloadClick } from "@/features/download/download-click";
import { homeClicks } from "@/features/home/clicks";
import { trackLegacyClick, useLegacyTrackScope } from "@/core/lib/telemetry/legacy-click";
import { whatsOnCards } from "@/features/home/cards.server";
import { detectPlatform } from "@/features/download/options";
import {
  homeAgentContent,
  VIBE_CLIPS,
  WEEKLY_RITUALS,
} from "@/features/home/content";

import { loadDownloadCounts } from "@/data/lib/download/counts.server";
import { explorerInstallersUrl } from "@/data/lib/catalyst/bases";
import { loadHomeLive } from "@/data/lib/catalyst/landings/home.server";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import { useAnalyticsUserId } from "@/core/lib/telemetry/page-view";
import { track } from "@/core/lib/telemetry/track";

import { homeMeta, publicBase } from "../lib/social-meta";
import type { Route } from "./+types/_index";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";

export const handle = {
  agentMarkdown: "discover",
  sitesLocalized: true,
} satisfies AgentMarkdownHandle & SitesLocaleHandle;

const STORY = "discover";
/** The reference's count while its download counter loads or fails. */
const DOWNLOADS_FALLBACK = "+400K";

export function meta({ matches }: Route.MetaArgs) {
  return homeMeta(publicBase(matches));
}

export async function loader({ request, context }: Route.LoaderArgs) {
  const { sid, wrap } = sidLoader(request, context);
  const [{ content: loadedContent, cards, profiles }, downloads] =
    await Promise.all([loadHomeLive(), loadDownloadCounts()]);
  const locale = sitesLanguage(
    parseCookies(request.headers.get("cookie")).lang,
  ).code;
  const content = homeAgentContent(loadedContent, locale);
  const payload = {
    sid,
    locale,
    content,
    downloads: downloads ?? DOWNLOADS_FALLBACK,
    platform: detectPlatform(request.headers.get("user-agent")),
    googlePlay: googlePlayHref(new URL(request.url).search),
    installers: explorerInstallersUrl() !== null,
    cards: whatsOnCards(cards, profiles),
    faces: Object.fromEntries(
      VIBE_CLIPS.map((clip) => [
        clip.address,
        profiles[clip.address]?.face ?? null,
      ]),
    ),
  };
  return wrap(payload);
}

export default function Home({ loaderData }: Route.ComponentProps) {
  const { sid, platform, installers, downloads, cards, faces, googlePlay } = loaderData;

  const locale = useContext(SitesLocaleContext)?.locale ?? loaderData.locale;
  const copy = homeCopy[locale];
  const userId = useAnalyticsUserId();
  const scope = useLegacyTrackScope();
  const viewed = useRef(false);
  useEffect(() => {
    if (viewed.current) return;
    viewed.current = true;
    track("lp_discover_viewed", {}, { sid, story: STORY });
  }, [sid]);

  const { hero, hangOut, store, alternateStore, hangOutStore } = withDownloadClicks(
    homeDownloadActions(platform, downloads, installers, copy.download, googlePlay),
    (click) => trackDownloadClick(click, { sid, userId, scope }),
  );
  const clicks = homeClicks((click) => trackLegacyClick(click, { sid, userId, scope }));
  const labels = {
    previous: copy.slides.previous,
    next: copy.slides.next,
    goTo: (n: number) => `${copy.slides.goTo} ${n}`,
    position: (n: number, total: number) =>
      copy.slides.position
        .replace("{current}", String(n))
        .replace("{total}", String(total)),
  };
  return (
    <SitesLayout
      active="play"
      overlayNav
      headerSurface="overlay"
      footerCopy={sitesFooterCopy[locale]}
    >
      <main>
        <Hero
          title={copy.hero.title}
          imageAlt={copy.hero.imageAlt}
          actions={hero}
          mobile={{
            title: copy.hero.mobileTitle,
            subtitle: copy.hero.subtitle,
            store,
            alternateStore,
          }}
        />
        <WhatsOn
          title={copy.whatsOn.title}
          labels={copy.whatsOn}
          carouselLabels={labels}
          viewAll={{ label: copy.whatsOn.view_all, href: "/events", onClick: clicks.viewAll }}
          background={<AnimatedBackground />}
          cards={cards}
          onCardClick={clicks.whatsOnCard}
          onJumpInClick={clicks.whatsOnJumpIn}
        />
        <CatchTheVibe
          title={copy.vibe}
          labels={copy.media}
          carouselLabels={labels}
          items={VIBE_CLIPS.map((clip) => ({
            id: clip.id,
            image: clip.image,
            video: clip.video,
            creator: {
              name: clip.name,
              href: `/profile/${clip.address}`,
              avatarUrl: faces[clip.address] ?? undefined,
            },
          }))}
          hangOut={{ label: copy.hangOutNow, href: "/play/", onClick: clicks.hangOut }}
        />
        <WeeklyRituals
          title={copy.rituals.title}
          labels={labels}
          onCardClick={clicks.ritual}
          cards={WEEKLY_RITUALS.map((ritual) => ({
            id: ritual.id,
            title: copy.rituals[ritual.id],
            href: homeEventHref(ritual.eventId),
            image: ritual.image,
            mobileImage: ritual.mobileImage,
          }))}
        />
        <ComeHangOut
          title={copy.hangOut}
          background={<AnimatedBackground />}
          actions={hangOut}
          store={hangOutStore}
        />
      </main>
    </SitesLayout>
  );
}
