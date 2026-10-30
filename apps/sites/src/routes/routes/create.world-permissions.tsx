import { redirect } from "react-router";

import type { Route } from "./+types/create.world-permissions";

/** `/create/world-permissions` is the retired name of `/create/admissions`. */
export async function loader({ request }: Route.LoaderArgs) {
  return redirect("/create/admissions" + new URL(request.url).search, 308);
}

export default function CreateGateMoved() {
  return null;
}
