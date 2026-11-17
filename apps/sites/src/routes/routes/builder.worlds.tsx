import { redirect } from "react-router";

import { ownedSessionCookies, requestSession } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/builder.worlds";

const STORY = "creator-integration-redirect-worlds";
const FROM = "/builder/worlds";
const TO = "/create/worlds";

export async function loader({ request, context }: Route.LoaderArgs) {
  const session = requestSession(request, context);
  const { sid } = session;
  // Behind the root middleware the cookie is set once on the response; only a direct call sets it here.
  const cookies = ownedSessionCookies(request, session);

  const query = new URL(request.url).searchParams.toString();
  const location = query ? `${TO}?${query}` : TO;

  track(
    "creator_builder_redirect",
    { from: FROM, to: TO, query },
    { sid, story: STORY },
  );

  return redirect(
    location,
    cookies ? { headers: cookies } : undefined,
  );
}
