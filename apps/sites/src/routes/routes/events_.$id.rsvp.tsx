import { redirect } from "react-router";

import { legacyEventsHref } from "@/features/events/view";

/**
 * Going to an event is the detail's "Remind me", as on decentraland.org: an old RSVP link opens that detail,
 * keeping its query (campaign tags) but not the old page's `step`, which the Events page has no use for.
 */
export function loader({ request, params }: { request: Request; params: { id?: string } }) {
  const url = new URL(request.url);
  url.searchParams.delete("step");
  return redirect(legacyEventsHref(url, params.id?.trim() || undefined));
}
