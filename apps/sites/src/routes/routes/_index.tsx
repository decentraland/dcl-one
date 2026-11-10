import "@dcl/one-ui/foundations/foundations.css";
import { useEffect, useRef } from "react";

import SitesChrome from "@ui/web/frames/SitesChrome";
import AnimatedBackground from "@ui/shop/discovery/components/animated-background/AnimatedBackground";
import { Hero } from "@dcl/one-ui/apps/sites/home/Hero";
import { WhatsOn } from "@dcl/one-ui/apps/sites/home/WhatsOn";
import { CatchTheVibe } from "@dcl/one-ui/apps/sites/home/CatchTheVibe";
import { WeeklyRituals } from "@dcl/one-ui/apps/sites/home/WeeklyRituals";
import { ComeHangOut } from "@dcl/one-ui/apps/sites/home/ComeHangOut";
import { homeActions } from "@/features/home/actions";
import { detectPlatform, type DetectedPlatform } from "@/features/home/options";
import { VIBE_CLIPS, WEEKLY_RITUALS } from "@/features/home/content";

import {
  heroEventCards,
  type HomeContent,
} from "@/data/lib/catalyst/landings/home";
import { loadDownloadsLabel } from "@/data/lib/catalyst/landings/download-counts.server";
import { loadHome } from "@/data/lib/catalyst/landings/home.server";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/_index";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";

export const handle = {
  agentMarkdown: "discover",
} satisfies AgentMarkdownHandle;

const STORY = "discover";

export function meta() {
  return [{ title: "Decentraland" }];
}

export async function loader({ request }: Route.LoaderArgs) {
  const { sid, wrap } = sidLoader(request);
  const [{ content, live }, downloads] = await Promise.all([
    loadHome(),
    loadDownloadsLabel(),
  ]);
  const payload = {
    sid,
    content,
    live,
    downloads,
    platform: detectPlatform(request.headers.get("user-agent")),
  };
  return wrap(payload);
}

type LoaderData = {
  sid: string;
  content: HomeContent;
  live: boolean;
  downloads: string;
  platform: DetectedPlatform;
};

export default function Home({ loaderData }: Route.ComponentProps) {
  const { sid, content, platform, downloads } = loaderData as LoaderData;

  const viewed = useRef(false);
  useEffect(() => {
    if (viewed.current) return;
    viewed.current = true;
    track("lp_discover_viewed", {}, { sid, story: STORY });
  }, [sid]);

  const { actions, store } = homeActions(platform, downloads);
  const labels = {
    previous: "Previous slide",
    next: "Next slide",
    goTo: (n: number) => `Go to slide ${n}`,
    position: (n: number, total: number) => `Slide ${n} of ${total}`,
  };
  return (
    <SitesChrome active="play" overlayNav>
      <main>
        <Hero
          title="Close the Feed. Come Hang Out."
          imageAlt="Friends exploring Decentraland"
          actions={actions}
          mobile={{
            title: "Hang Out From Anywhere",
            subtitle: "Decentraland now on mobile.",
            store,
          }}
        />
        <WhatsOn
          title="Jump Into What's Happening"
          labels={{ live: "LIVE", by: "by" }}
          viewAll={{ label: "View all", href: "/whats-on" }}
          cards={(heroEventCards(content) ?? [])
            .filter((card) => card.live)
            .map((card) => ({
              id: card.id,
              title: card.title,
              image: card.image,
              href: `/events?id=${encodeURIComponent(card.id)}`,
            }))}
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
            creator: { name: clip.name, href: `/profile/${clip.address}` },
          }))}
        />
        <WeeklyRituals
          title="Your Weekly Rituals"
          labels={labels}
          cards={WEEKLY_RITUALS.map((ritual) => ({
            id: ritual.id,
            title: ritual.title,
            href: `/events?id=${encodeURIComponent(ritual.eventId)}`,
            image: ritual.image,
            mobileImage: ritual.mobileImage,
          }))}
        />
        <ComeHangOut
          title="Come Hang Out"
          background={<AnimatedBackground />}
          actions={platform.kind === "mobile" ? undefined : actions}
          store={platform.kind === "mobile" ? store : undefined}
        />
      </main>
    </SitesChrome>
  );
}
