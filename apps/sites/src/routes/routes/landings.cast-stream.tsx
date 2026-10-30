import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";

import CastConsole from "@/features/social/cast-stream/CastConsole";

import type { Route } from "./+types/landings.cast-stream";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "landings/cast-stream";

const DEFAULT_ASSIGNMENT: PageDefaults = {
  variant: "console",
};

const DEFAULT_TOKEN = "demo-stream-key";

export async function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const step = url.searchParams.get("step")?.trim() || null;
  const tokenParam = url.searchParams.get("token");
  const token = tokenParam === null ? DEFAULT_TOKEN : tokenParam;
  const identity = url.searchParams.get("name")?.trim() || undefined;

  const { sid, assignment, wrap } = await pageLoader(
    request,
    
    DEFAULT_ASSIGNMENT,
  );

  const payload = { sid, step, token, identity: identity ?? null, assignment };
  return wrap(payload);
}

export default function LandingsCastStream({ loaderData }: Route.ComponentProps) {
  const { sid, step, token, identity, assignment } = loaderData;

  return (
    <main className="cast-stream">
      <CastConsole
        trackCtx={{
          sid,
          story: STORY,
          variant: assignment.variant,
        }}
        token={token}
        identity={identity ?? undefined}
        initialStep={step ?? undefined}
      />
    </main>
  );
}
