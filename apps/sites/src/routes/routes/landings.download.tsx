import { redirect } from "react-router";

import type { Route } from "./+types/landings.download";

/** The download page lives at /download, the path of the sites reference. */
export function loader({ request }: Route.LoaderArgs) {
  return redirect(`/download${new URL(request.url).search}`, 308);
}
