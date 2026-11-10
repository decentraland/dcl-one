import { redirect } from "react-router";

import { requestSession, serializeSidCookie } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/create.claim-name";

const FROM = "/create/claim-name";

export async function loader({ request, context }: Route.LoaderArgs) {
  const session = requestSession(request, context);
  const { sid } = session;
  // Behind the root middleware the cookie is set once on the response; only a direct call sets it here.
  const created = session.ownsCookie && session.created;

  const target = `/shop/names${new URL(request.url).search}`;

  track("creator_claim_name_redirect", { from: FROM, to: target }, { sid });

  return redirect(
    target,
    created ? { headers: { "Set-Cookie": serializeSidCookie(sid) } } : undefined,
  );
}

export default function CreatorHubClaimName() {
  return null;
}
