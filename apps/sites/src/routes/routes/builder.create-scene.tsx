import { redirect } from "react-router";

import { requestSession, serializeSidCookie } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/builder.create-scene";

const STORY = "creator-integration-redirect-create-scene";
const FROM = "/builder/create-scene";
const TO = "/create/create-project";

export async function loader({ request, context }: Route.LoaderArgs) {
  const session = requestSession(request, context);
  const { sid } = session;
  // Behind the root middleware the cookie is set once on the response; only a direct call sets it here.
  const created = session.ownsCookie && session.created;

  const query = new URL(request.url).searchParams.toString();
  const location = query ? `${TO}?${query}` : TO;

  track(
    "creator_builder_redirect",
    { from: FROM, to: TO, query },
    { sid, story: STORY },
  );

  return redirect(
    location,
    created ? { headers: { "Set-Cookie": serializeSidCookie(sid) } } : undefined,
  );
}
