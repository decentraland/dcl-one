import { redirect } from "react-router";
import { dehydrate, HydrationBoundary, QueryClient } from "@tanstack/react-query";

import SitesChrome from "@ui/web/frames/SitesChrome";

import { latestPostsQuery } from "@/data/lib/create-landing/blog-posts";
import { latestPosts } from "@/data/lib/create-landing/blog-posts.server";
import { liveScenesQuery } from "@/data/lib/create-landing/live-scenes";
import { heroDownloads, loadLatestRelease } from "@/data/lib/download/creator-hub.server";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import CreateLanding from "@/features/create-landing/CreateLanding";
import { CREATOR_HUB_DOWNLOAD } from "@/features/create-landing/HeroDownload";
import { SubNav } from "@/features/creators/components/SubNav";

import { absoluteUrl, publicBase, socialMeta } from "../lib/social-meta";
import type { Route } from "./+types/create._index";

// decentraland.org serves /create from creators-website, whose shell carries this card.
const CREATE_SOCIAL_IMAGE =
  "https://images.ctfassets.net/ea2ybdmmn1kv/3fZB29mbYNJdp7sv1y2SJ2/108e55d6d3e23cd11e7d75563d81caf5/hero.webp?fm=jpg&q=85";

export function meta({ matches }: Route.MetaArgs) {
  return socialMeta({
    title: "Create in Decentraland | Wearables, Emotes, Scenes & Worlds",
    description:
      "Everything you need to create in Decentraland: build Wearables, Emotes, Scenes and Worlds, manage your collections and join the creator community.",
    image: CREATE_SOCIAL_IMAGE,
    url: absoluteUrl(publicBase(matches), "/create"),
  });
}

// The landing never waits long for the release: without it the hero links to the download page, which does.
const RELEASE_WAIT_MS = 300;

async function latestReleaseInTime() {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      loadLatestRelease().catch(() => null),
      new Promise<null>((resolve) => {
        timer = setTimeout(() => resolve(null), RELEASE_WAIT_MS);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

export async function loader({ request, context }: Route.LoaderArgs) {
  // `dcl-scene-cli start` still links here with `?projectUrl=`; scenes are edited in the desktop app.
  if (new URL(request.url).searchParams.has("projectUrl")) {
    throw redirect(CREATOR_HUB_DOWNLOAD, { headers: { "Cache-Control": "private, no-store" } });
  }
  const { sid, wrap } = sidLoader(request, context);
  const queryClient = new QueryClient();
  const [release] = await Promise.all([
    latestReleaseInTime(),
    queryClient.prefetchQuery(liveScenesQuery()),
    queryClient.prefetchQuery({ ...latestPostsQuery(), queryFn: () => latestPosts() }),
  ]);
  const dehydratedState = dehydrate(queryClient);
  queryClient.clear();
  const downloads = release ? heroDownloads(release, request.headers.get("user-agent")) : null;
  return wrap({ dehydratedState, downloads, sid });
}

export async function clientLoader(): Promise<null> {
  return null;
}

export default function CreateRoute({ loaderData }: Route.ComponentProps) {
  return (
    <HydrationBoundary state={loaderData?.dehydratedState}>
      <SitesChrome active="create" overlayNav>
        <SubNav landing />
        <CreateLanding downloads={loaderData?.downloads} sid={loaderData?.sid} />
      </SitesChrome>
    </HydrationBoundary>
  );
}
