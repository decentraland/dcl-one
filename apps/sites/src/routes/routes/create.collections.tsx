import { redirect } from "react-router";

import { requestSession, serializeSidCookie } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/create.collections";

const FROM = "/create/collections";
const TO = "/create/wearables";

export async function loader({ request, context }: Route.LoaderArgs) {
  const session = requestSession(request, context);
  const { sid } = session;
  // Behind the root middleware the cookie is set once on the response; only a direct call sets it here.
  const created = session.ownsCookie && session.created;

  track("creator_builder_redirect", { from: FROM, to: TO }, { sid });

  const to = `${TO}${new URL(request.url).search}`;
  return redirect(
    to,
    created ? { headers: { "Set-Cookie": serializeSidCookie(sid) } } : undefined,
  );
}

export default function CreateCollectionsRedirect() {
  return null;
}
