import { redirect } from "react-router";

import { legacyEventsHref } from "@/features/events/view";

import type { Route } from "./+types/whats-on_.$id";

/** An old event detail link opens the same event in the Events page's detail. */
export function loader({ request, params }: Route.LoaderArgs) {
  return redirect(legacyEventsHref(new URL(request.url), params.id));
}
