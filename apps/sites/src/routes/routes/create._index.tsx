import { dehydrate, HydrationBoundary, QueryClient } from "@tanstack/react-query";

import SitesChrome from "@ui/web/frames/SitesChrome";

import { latestPostsQuery } from "@/data/lib/create-landing/blog-posts";
import { latestPosts } from "@/data/lib/create-landing/blog-posts.server";
import { liveScenesQuery } from "@/data/lib/create-landing/live-scenes";
import { heroDownloads, loadLatestRelease } from "@/data/lib/download/creator-hub.server";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import CreateLanding from "@/features/create-landing/CreateLanding";

import type { Route } from "./+types/create._index";

export const meta = () => [
  { title: "Create in Decentraland | Decentraland" },
  {
    name: "description",
    content: "Download the Creator Hub to start building immersive scenes and interactive experiences in Decentraland.",
  },
];

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
        <CreateLanding downloads={loaderData?.downloads} sid={loaderData?.sid} />
      </SitesChrome>
    </HydrationBoundary>
  );
}
