import { redirect } from "react-router";

import type { Route } from "./+types/builder.names";

export async function loader(_: Route.LoaderArgs) {
  return redirect("/profile/me/assets", 308);
}
