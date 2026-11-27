import { Outlet } from "react-router";

import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";

import { isWorldLocation } from "@/data/lib/catalyst/landings/cast-watcher";
import { loadWorldScenes, type WorldScene } from "@/data/lib/catalyst/places/world-scenes";

import LiveCastWatch from "@/features/social/cast-live/LiveCastWatch";

import type { Route } from "./+types/cast.w.$location";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "cast/watch";

const DEFAULT_ASSIGNMENT: PageDefaults = {
  variant: "live",
};

// The watcher link the gatekeeper hands out: {CAST2_URL}/w/{location}, a parcel or a world.
export async function loader({ request, context, params }: Route.LoaderArgs) {
  const { sid, assignment, wrap } = await pageLoader(request, context, DEFAULT_ASSIGNMENT);
  const location = params.location.trim();
  let scenes: WorldScene[] | null = null;
  if (!params.parcel && isWorldLocation(location)) {
    const deadline = AbortSignal.timeout(3000);
    try {
      scenes = await loadWorldScenes(location, {
        signal: AbortSignal.any([request.signal, deadline]),
      });
    } catch (error) {
      if (request.signal.aborted || !deadline.aborted) throw error;
    }
  }
  return wrap({ sid, assignment, location, scenes });
}

export function meta() {
  return [{ title: "Decentraland Cast" }, { name: "robots", content: "noindex" }];
}

export default function CastWatchPage({ loaderData, params }: Route.ComponentProps) {
  if (params.parcel) return <Outlet />;
  const { sid, assignment, location, scenes } = loaderData;
  return (
    <main className="cast-watch-route">
      <LiveCastWatch
        key={location}
        trackCtx={{ sid, story: STORY, variant: assignment.variant }}
        location={location}
        scenes={scenes}
      />
    </main>
  );
}
