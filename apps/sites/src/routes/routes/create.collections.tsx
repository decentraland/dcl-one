import { redirect } from "react-router";

import { ownedSessionCookies, requestSession } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/create.collections";

const FROM = "/create/collections";
const TO = "/create/wearables";

export async function loader({ request, context }: Route.LoaderArgs) {
  const session = requestSession(request, context);
  const { sid } = session;
  // Behind the root middleware the cookie is set once on the response; only a direct call sets it here.
  const cookies = ownedSessionCookies(request, session);

  track("creator_builder_redirect", { from: FROM, to: TO }, { sid });

  const to = `${TO}${new URL(request.url).search}`;
  return redirect(
    to,
    cookies ? { headers: cookies } : undefined,
  );
}

export default function CreateCollectionsRedirect() {
  return null;
}
