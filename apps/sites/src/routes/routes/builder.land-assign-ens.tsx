import { redirect } from "react-router";

import { landRedirectTarget } from "@ui/shop/land/redirects";

import type { Route } from "./+types/builder.land-assign-ens";

// The Builder's per-LAND assign-ens URL: the Shop page of the LAND it names (the Creator Hub's LAND list
// when it names none), instead of the Shop home that dropped the feature.
export async function loader({ request }: Route.LoaderArgs) {
  return redirect(landRedirectTarget("assign-ens", new URL(request.url).search), 308);
}
