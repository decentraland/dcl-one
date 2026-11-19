import { redirect } from "react-router";

import type { Route } from "./+types/builder.pool_.$id";

export async function loader(_: Route.LoaderArgs) {
  return redirect("/download/creator-hub", 308);
}
