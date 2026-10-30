import { redirect } from "react-router";

import type { Route } from "./+types/builder.land";

export async function loader(_: Route.LoaderArgs) {
  return redirect("/create/land", 308);
}
