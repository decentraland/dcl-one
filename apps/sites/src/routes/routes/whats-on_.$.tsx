import { redirect } from "react-router";

import { legacyWhatsOnHref } from "@/features/events/view";

import type { Route } from "./+types/whats-on_.$";

/** Moderation and edit links keep their target under /events; any other deeper path lands on the page. */
export function loader({ request, params }: Route.LoaderArgs) {
  return redirect(legacyWhatsOnHref(new URL(request.url), params["*"] ?? ""));
}
