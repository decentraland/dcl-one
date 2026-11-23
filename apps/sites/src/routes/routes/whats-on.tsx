import { legacyRedirect } from "@/core/lib/telemetry/legacy-redirect";
import { legacyEventsHref } from "@/features/events/view";

/** What's On is now Events: the old path keeps its query, so shared links and bookmarks resolve. */
export function loader({ request }: { request: Request }) {
  return legacyRedirect(request, "events", legacyEventsHref(new URL(request.url)));
}
