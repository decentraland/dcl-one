import { redirect } from "react-router";

import { landRedirectTarget } from "@/core/lib/shop/land-redirects";

import type { Route } from "./+types/builder.land_.$id";

export async function loader({ params }: Route.LoaderArgs) {
  return redirect(landRedirectTarget("manage", `?id=${encodeURIComponent(params.id)}`), 308);
}
