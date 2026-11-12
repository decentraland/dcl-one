import { redirect } from "react-router";

import type { Route } from "./+types/jump.event";

/** `/jump/event` is the singular URL older share links still carry. */
export function loader({ request }: Route.LoaderArgs) {
  return redirect(`/jump/events${new URL(request.url).search}`, 308);
}
