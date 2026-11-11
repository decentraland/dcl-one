import { redirect } from "react-router";

import { legacyEventsHref } from "@/features/events/view";

/** Any deeper What's On path has no Events counterpart: it lands on the page with its query. */
export function loader({ request }: { request: Request }) {
  return redirect(legacyEventsHref(new URL(request.url)));
}
