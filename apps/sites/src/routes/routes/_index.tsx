import "@dcl/one-ui/foundations/foundations.css";
import { useEffect, useRef } from "react";

import SitesChrome from "@ui/web/frames/SitesChrome";
import AnimatedBackground from "@ui/shop/discovery/components/animated-background/AnimatedBackground";
import { Hero } from "@dcl/one-ui/apps/sites/home/Hero";
import { WhatsOn } from "@dcl/one-ui/apps/sites/home/WhatsOn";
import { CatchTheVibe } from "@dcl/one-ui/apps/sites/home/CatchTheVibe";
import { WeeklyRituals } from "@dcl/one-ui/apps/sites/home/WeeklyRituals";
import { ComeHangOut } from "@dcl/one-ui/apps/sites/home/ComeHangOut";
import { homeEventHref } from "@/features/home/links";
import { downloadActions } from "@/features/download/actions";
import { detectPlatform } from "@/features/download/options";
import {
  homeAgentContent,
  VIBE_CLIPS,
  WEEKLY_RITUALS,
} from "@/features/home/content";

import { heroEventCards } from "@/data/lib/catalyst/landings/home";
import { loadDownloadsLabel } from "@/data/lib/catalyst/landings/download-counts.server";
import { loadHomeEvents } from "@/data/lib/catalyst/landings/home.server";
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
  const [{ content: loadedContent }, downloads] = await Promise.all([
    loadHomeEvents(),
    loadDownloadsLabel(),
  ]);
  const content = homeAgentContent(loadedContent);
  const payload = {
    sid,
    content,
    downloads,
    platform: detectPlatform(request.headers.get("user-agent")),
  };
  return wrap(payload);
}

export default function Home({ loaderData }: Route.ComponentProps) {
  const { sid, content, platform, downloads } = loaderData;

  const viewed = useRef(false);
  useEffect(() => {
    if (viewed.current) return;
    viewed.current = true;
    track("lp_discover_viewed", {}, { sid, story: STORY });
  }, [sid]);

  const { actions, store } = downloadActions(platform, downloads);
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
          viewAll={{ label: "View all", href: "/events" }}
          cards={(heroEventCards(content) ?? []).map((card) => ({
            id: card.id,
            title: card.title,
            image: card.image,
            href: homeEventHref(card.id),
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
            href: homeEventHref(ritual.eventId),
            image: ritual.image,
            mobileImage: ritual.mobileImage,
          }))}
        />
        <ComeHangOut
          title="Come Hang Out"
          background={<AnimatedBackground />}
          actions={actions}
          store={store}
        />
      </main>
    </SitesChrome>
  );
}
