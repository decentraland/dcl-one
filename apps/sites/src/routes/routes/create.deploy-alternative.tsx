import { redirect } from "react-router";

import type { Route } from "./+types/create.deploy-alternative";

export async function loader(_args: Route.LoaderArgs) {
  return redirect("/create/publish", 308);
}
