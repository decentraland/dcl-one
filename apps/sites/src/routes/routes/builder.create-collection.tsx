import { redirect } from "react-router";

import { buildRedirect } from "@/data/lib/catalyst/creator-hub/integration-redirect-collections";
import { requestSession, serializeSidCookie } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/builder.create-collection";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "creator-hub/integration-redirect-collections";
const FROM = "/builder/create-collection";

export async function loader({ request, context }: Route.LoaderArgs) {
  const session = requestSession(request, context);
  const { sid } = session;
  // Behind the root middleware the cookie is set once on the response; only a direct call sets it here.
  const created = session.ownsCookie && session.created;

  const { location, to, query } = buildRedirect(request.url);

  track(
    "creator_builder_redirect",
    { from: FROM, to, query },
    { sid, story: STORY },
  );

  return redirect(
    location,
    created ? { headers: { "Set-Cookie": serializeSidCookie(sid) } } : undefined,
  );
}
