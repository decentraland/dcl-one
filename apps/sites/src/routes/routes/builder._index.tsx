import { redirect } from "react-router";

import { ownedSessionCookies, requestSession } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/builder._index";

const STORY = "creator-integration-redirect-home";
const FROM = "/builder";
const TO = "/create";

export async function loader({ request, context }: Route.LoaderArgs) {
  const session = requestSession(request, context);
  const { sid } = session;

  const query = new URL(request.url).searchParams.toString();
  const location = query ? `${TO}?${query}` : TO;

  track(
    "creator_builder_redirect",
    { from: FROM, to: TO, query },
    { sid, story: STORY },
  );

  // Behind the root middleware the cookie is set once on the response; on its own this sets it.
  const cookies = ownedSessionCookies(request, session);
  return redirect(location, cookies ? { headers: cookies } : undefined);
}
