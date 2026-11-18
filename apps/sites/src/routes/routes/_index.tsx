import { useEffect, useRef, useState } from "react";

import SitesChrome from "@ui/web/frames/SitesChrome";
import { sitesLanguage } from "@ui/web/frames/sites-locale";
import { homeCopy } from "@/features/home/locale";
import { successCopy } from "@/features/download/success-copy";
import { parseCookies } from "@/core/lib/telemetry/session";
import AnimatedBackground from "@ui/shop/discovery/components/animated-background/AnimatedBackground";
import { Hero } from "@dcl/one-ui/apps/sites/home/Hero";
import { WhatsOn } from "@dcl/one-ui/apps/sites/home/WhatsOn";
import { CatchTheVibe } from "@dcl/one-ui/apps/sites/home/CatchTheVibe";
import { WeeklyRituals } from "@dcl/one-ui/apps/sites/home/WeeklyRituals";
import { ComeHangOut } from "@dcl/one-ui/apps/sites/home/ComeHangOut";
import { homeEventHref } from "@/features/home/links";
import { homeDownloadActions } from "@/features/home/actions";
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
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/_index";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";

export const handle = {
  agentMarkdown: "discover",
} satisfies AgentMarkdownHandle;

const STORY = "discover";
/** The reference's count while its download counter loads or fails. */
const DOWNLOADS_FALLBACK = "+400K";

export function meta() {
  return [{ title: "Decentraland" }];
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
  const { sid, platform, installers, downloads, cards, faces } = loaderData;

  const [locale, setLocale] = useState(loaderData.locale);
  const copy = homeCopy[locale];
  const viewed = useRef(false);
  useEffect(() => {
    if (viewed.current) return;
    viewed.current = true;
    track("lp_discover_viewed", {}, { sid, story: STORY });
  }, [sid]);

  const { hero, hangOut, store, alternateStore } = homeDownloadActions(
    platform,
    downloads,
    installers,
    copy.download,
  );
  const labels = {
    previous: copy.slides.previous,
    next: copy.slides.next,
    goTo: (n: number) => `${copy.slides.goTo} ${n}`,
    position: (n: number, total: number) => `${n} / ${total}`,
  };
  return (
    <SitesChrome
      active="play"
      overlayNav
      headerSurface="overlay"
      language={{
        locale,
        onSelect: setLocale,
        footer: successCopy[locale].footer,
      }}
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
          viewAll={{ label: copy.whatsOn.view_all, href: "/events" }}
          background={<AnimatedBackground />}
          cards={cards}
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
          hangOut={{ label: copy.hangOutNow, href: "/play/" }}
        />
        <WeeklyRituals
          title={copy.rituals.title}
          labels={labels}
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
          store={store}
        />
      </main>
    </SitesChrome>
  );
}
