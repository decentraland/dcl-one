import { redirect } from "react-router";

import { requestSession, serializeSidCookie } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/builder.scenes";

const STORY = "creator-integration-redirect-scenes";
const FROM = "/builder/scenes";
const TO = "/create/scenes";

export async function loader({ request, context }: Route.LoaderArgs) {
  const session = requestSession(request, context);
  const { sid } = session;
  // Behind the root middleware the cookie is set once on the response; only a direct call sets it here.
  const created = session.ownsCookie && session.created;

  const params = new URL(request.url).searchParams;
  const address = params.get("address");
  if (address !== null) {
    params.delete("address");
    params.set("creator", address);
  }
  const query = params.toString();
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
