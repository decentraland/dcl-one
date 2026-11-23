import { redirect } from "react-router";

import type { Route } from "./+types/builder.land";

export async function loader(_: Route.LoaderArgs) {
  return redirect("/profile/me/assets", 308);
}
