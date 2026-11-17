import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";

import LiveCastStream from "@/features/social/cast-live/LiveCastStream";

import type { Route } from "./+types/cast.s.$token";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "cast/stream";

const DEFAULT_ASSIGNMENT: PageDefaults = {
  variant: "live",
};

// The stream link the gatekeeper hands a scene admin: {CAST2_URL}/s/{streamingKey}.
export async function loader({ request, context, params }: Route.LoaderArgs) {
  const { sid, assignment, wrap } = await pageLoader(request, context, DEFAULT_ASSIGNMENT);
  return wrap({ sid, assignment, streamingKey: params.token });
}

export function meta() {
  return [{ title: "Decentraland Cast" }, { name: "robots", content: "noindex" }];
}

export default function CastStreamPage({ loaderData }: Route.ComponentProps) {
  const { sid, assignment, streamingKey } = loaderData;
  return (
    <main className="cast-stream">
      <LiveCastStream
        key={streamingKey}
        trackCtx={{ sid, story: STORY, variant: assignment.variant }}
        streamingKey={streamingKey}
      />
    </main>
  );
}
