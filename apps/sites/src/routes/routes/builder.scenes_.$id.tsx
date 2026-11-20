import { redirect } from "react-router";

import type { Route } from "./+types/builder.scenes_.$id";

export async function loader(_: Route.LoaderArgs) {
  return redirect("/create", 308);
}
