import { redirect } from "react-router";

import type { Route } from "./+types/landings.report-abuse";

/** The report form lives at /report, the path of the sites reference. */
export function loader({ request }: Route.LoaderArgs) {
  return redirect(`/report${new URL(request.url).search}`, 308);
}
