import { redirect } from "react-router";

import type { Route } from "./+types/report.players";

/** The explorer's report link (`/report/players?player_address=&reported_address=`) opens the form with its query. */
export function loader({ request }: Route.LoaderArgs) {
  return redirect(`/report${new URL(request.url).search}`);
}
