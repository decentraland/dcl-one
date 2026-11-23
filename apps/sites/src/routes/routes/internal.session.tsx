import { sitesSegmentPublicConfig } from "@/core/lib/telemetry/public-config.server";
import { ownedSessionCookies, readVerifiedWallet, requestSession } from "@/core/lib/telemetry/session";

import type { Route } from "./+types/internal.session";

/**
 * The visitor's session id and verified wallet, plus the public Sites Segment transport the root
 * would have published, for a document whose root loader did not run (a URL no route matches renders
 * the 404 without it). The root middleware reads or mints the sid and sets its cookie on this
 * response; nothing here may be stored by a shared cache.
 */
export function loader({ request, context }: Route.LoaderArgs) {
  const session = requestSession(request, context);
  const headers = new Headers({ "Cache-Control": "no-store, private" });
  for (const [name, value] of ownedSessionCookies(request, session) ?? []) headers.append(name, value);
  return Response.json({ sid: session.sid, userId: readVerifiedWallet(request), ...sitesSegmentPublicConfig() }, { headers });
}
