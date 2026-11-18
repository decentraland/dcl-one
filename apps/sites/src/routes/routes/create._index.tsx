import { lazy, Suspense, useEffect, useSyncExternalStore } from "react";
import { redirect, useLocation, useNavigate } from "react-router";
import { dehydrate, HydrationBoundary, QueryClient } from "@tanstack/react-query";

import SitesChrome from "@ui/web/frames/SitesChrome";

import { creatorHubHost } from "@ui/creatorhub/explorer-host";

import { isDesktopShell } from "@/data/lib/auth/native-shell";
import { latestPostsQuery } from "@/data/lib/create-landing/blog-posts";
import { latestPosts } from "@/data/lib/create-landing/blog-posts.server";
import { liveScenesQuery } from "@/data/lib/create-landing/live-scenes";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import CreateLanding from "@/features/create-landing/CreateLanding";

import type { Route } from "./+types/create._index";

// The Create section's tabs. Loaded apart from the landing, which every visitor downloads, and drawn in the
// browser only: their stylesheet arrives with them, so server-rendered tabs would show unstyled first. They
// overlay the hero, so nothing moves when they appear.
const SubNav = lazy(() => import("@/features/creators/components/SubNav").then((m) => ({ default: m.SubNav })));

const subscribeNever = () => () => {};

export const meta = () => [
  { title: "Create in Decentraland | Decentraland" },
  {
    name: "description",
    content: "Download the Creator Hub to start building immersive scenes and interactive experiences in Decentraland.",
  },
];

/** Where `/create?projectUrl=…` (the link dcl-scene-cli start prints) goes: the SDK workspace of the
    scene editor. The run token rides the link as a `#token=` fragment, which the browser carries
    across this redirect untouched and the editor route claims client-side; it never reaches a
    query string or this server. */
export function sdkWorkspaceHandoff(url: URL): string | null {
  const projectUrl = url.searchParams.get("projectUrl")?.trim();
  if (!projectUrl) return null;
  const target = new URLSearchParams({ source: "sdk", projectUrl, from: "home" });
  return `/create/scene-editor?${target.toString()}`;
}

/** `/create` is the Creator Hub's marketing landing on the web; the desktop shell and the Explorer's
    framed hub start at the hub home. */
export async function loader({ request, context }: Route.LoaderArgs) {
  const handoff = sdkWorkspaceHandoff(new URL(request.url));
  if (handoff) throw redirect(handoff, { headers: { "Cache-Control": "private, no-store" } });

  const { wrap } = sidLoader(request, context);
  const queryClient = new QueryClient();
  await Promise.all([
    queryClient.prefetchQuery(liveScenesQuery()),
    queryClient.prefetchQuery({ ...latestPostsQuery(), queryFn: () => latestPosts() }),
  ]);
  const dehydratedState = dehydrate(queryClient);
  queryClient.clear();
  return wrap({ dehydratedState });
}

export async function clientLoader(): Promise<null> {
  return null;
}

export default function CreateRoute({ loaderData }: Route.ComponentProps) {
  const navigate = useNavigate();
  const { search, hash } = useLocation();
  const inBrowser = useSyncExternalStore(subscribeNever, () => true, () => false);

  useEffect(() => {
    if (isDesktopShell() || creatorHubHost(window)) navigate(`/create/home${search}${hash}`, { replace: true });
  }, [navigate, search, hash]);

  return (
    <HydrationBoundary state={loaderData?.dehydratedState}>
      <SitesChrome active="create" overlayNav>
        {inBrowser ? (
          <Suspense fallback={null}>
            <SubNav overlay />
          </Suspense>
        ) : null}
        <CreateLanding />
      </SitesChrome>
    </HydrationBoundary>
  );
}
