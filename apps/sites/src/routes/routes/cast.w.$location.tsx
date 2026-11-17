import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";

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
  return wrap({ sid, assignment, location: params.location.trim() });
}

export function meta() {
  return [{ title: "Decentraland Cast" }, { name: "robots", content: "noindex" }];
}

export default function CastWatchPage({ loaderData }: Route.ComponentProps) {
  const { sid, assignment, location } = loaderData;
  return (
    <main className="cast-watch-route">
      <LiveCastWatch
        key={location}
        trackCtx={{ sid, story: STORY, variant: assignment.variant }}
        location={location}
      />
    </main>
  );
}
