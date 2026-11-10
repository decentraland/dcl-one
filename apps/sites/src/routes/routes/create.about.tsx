import { redirect } from "react-router";

import { requestSession, serializeSidCookie } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/create.about";

const FROM = "/create/about";
const TO = "/create";

export async function loader({ request, context }: Route.LoaderArgs) {
  const session = requestSession(request, context);
  const { sid } = session;
  // Behind the root middleware the cookie is set once on the response; only a direct call sets it here.
  const created = session.ownsCookie && session.created;

  track("creator_builder_redirect", { from: FROM, to: TO }, { sid });

  return redirect(
    TO,
    created ? { headers: { "Set-Cookie": serializeSidCookie(sid) } } : undefined,
  );
}

export default function CreateAboutRedirect() {
  return null;
}
