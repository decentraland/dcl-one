import { useEffect, useRef } from "react";

import SitesChrome from "@ui/web/frames/SitesChrome";
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
import { loadHomeLive } from "@/data/lib/catalyst/landings/home.server";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/_index";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";

export const handle = {
  agentMarkdown: "discover",
} satisfies AgentMarkdownHandle;

const STORY = "discover";
/** The reference's count while its download counter loads or fails (sites Hero.tsx). */
const DOWNLOADS_FALLBACK = "+400K";

export function meta() {
  return [{ title: "Decentraland" }];
}

export async function loader({ request, context }: Route.LoaderArgs) {
  const { sid, wrap } = sidLoader(request, context);
  const [{ content: loadedContent, cards, profiles }, downloads] =
    await Promise.all([loadHomeLive(), loadDownloadCounts()]);
  const content = homeAgentContent(loadedContent);
  const payload = {
    sid,
    content,
    downloads: downloads ?? DOWNLOADS_FALLBACK,
    platform: detectPlatform(request.headers.get("user-agent")),
    cards: whatsOnCards(cards, profiles),
    faces: Object.fromEntries(
      VIBE_CLIPS.map((clip) => [clip.address, profiles[clip.address]?.face ?? null]),
    ),
  };
  return wrap(payload);
}

export default function Home({ loaderData }: Route.ComponentProps) {
  const { sid, platform, downloads, cards, faces } = loaderData;

  const viewed = useRef(false);
  useEffect(() => {
    if (viewed.current) return;
    viewed.current = true;
    track("lp_discover_viewed", {}, { sid, story: STORY });
  }, [sid]);

  const { hero, hangOut, store } = homeDownloadActions(platform, downloads);
  const labels = {
    previous: "Previous slide",
    next: "Next slide",
    goTo: (n: number) => `Go to slide ${n}`,
    position: (n: number, total: number) => `Slide ${n} of ${total}`,
  };
  return (
    <SitesChrome active="play" overlayNav headerSurface="overlay">
      <main>
        <Hero
          title="Close the Feed. Come Hang Out."
          imageAlt="Friends exploring Decentraland"
          actions={hero}
          mobile={{
            title: "Hang Out From Anywhere",
            subtitle: "Decentraland now on mobile.",
            store,
          }}
        />
        <WhatsOn
          title="Jump Into What's Happening"
          labels={{ live: "LIVE", by: "by", jumpIn: "Jump in" }}
          carouselLabels={labels}
          viewAll={{ label: "View All", href: "/whats-on" }}
          background={<AnimatedBackground />}
          cards={cards}
        />
        <CatchTheVibe
          title="Catch the Vibe"
          labels={{
            play: "Play",
            pause: "Pause",
            mute: "Mute",
            unmute: "Unmute",
          }}
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
          hangOut={{ label: "Hang out now", href: "/play/" }}
        />
        <WeeklyRituals
          title="Your Weekly Rituals"
          labels={labels}
          cards={WEEKLY_RITUALS.map((ritual) => ({
            id: ritual.id,
            title: ritual.title,
            href: homeEventHref(ritual.eventId),
            image: ritual.image,
            mobileImage: ritual.mobileImage,
          }))}
        />
        <ComeHangOut
          title="Come Hang Out"
          background={<AnimatedBackground />}
          actions={hangOut}
          store={store}
        />
      </main>
    </SitesChrome>
  );
}
