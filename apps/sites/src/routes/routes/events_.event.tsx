import { redirect } from "react-router";

import { legacyEventsHref } from "@/features/events/view";

/** The standalone events site carried its target in the query (`?id=`); the page reads the same one. */
export function loader({ request }: { request: Request }) {
  return redirect(legacyEventsHref(new URL(request.url)));
}
