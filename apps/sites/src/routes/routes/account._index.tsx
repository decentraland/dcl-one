import { redirect } from "react-router";

import type { Route } from "./+types/account._index";

/** The settings open on notifications, the first section listed. */
export function loader({ request }: Route.LoaderArgs) {
  return redirect(`/account/notifications${new URL(request.url).search}`);
}
