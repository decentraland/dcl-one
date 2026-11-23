import { legacyRedirect } from "@/core/lib/telemetry/legacy-redirect";
import { legacyWhatsOnHref } from "@/features/events/view";

import type { Route } from "./+types/whats-on_.$";

/** Moderation and edit links keep their target under /events; any other deeper path lands on the page. */
export function loader({ request, params }: Route.LoaderArgs) {
  return legacyRedirect(request, "events", legacyWhatsOnHref(new URL(request.url), params["*"] ?? ""));
}
