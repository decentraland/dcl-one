import { redirect } from "react-router";

import { landRedirectTarget } from "@ui/shop/land/redirects";

import type { Route } from "./+types/builder.land-transfer";

// The Builder's per-LAND transfer URL: the Shop page of the LAND it names (the Creator Hub's LAND list
// when it names none), instead of the Shop home that dropped the feature.
export async function loader({ request }: Route.LoaderArgs) {
  return redirect(landRedirectTarget("transfer", new URL(request.url).search), 308);
}
