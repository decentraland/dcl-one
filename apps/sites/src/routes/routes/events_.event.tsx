import { legacyRedirect } from "@/core/lib/telemetry/legacy-redirect";
import { legacyEventsHref } from "@/features/events/view";

/** The standalone events site carried its target in the query (`?id=`); the page reads the same one. */
export function loader({ request }: { request: Request }) {
  return legacyRedirect(request, "events", legacyEventsHref(new URL(request.url)));
}
