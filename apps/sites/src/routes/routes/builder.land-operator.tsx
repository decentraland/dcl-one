import { redirect } from "react-router";

import { landRedirectTarget } from "@/core/lib/shop/land-redirects";

import type { Route } from "./+types/builder.land-operator";

export async function loader({ request }: Route.LoaderArgs) {
  return redirect(landRedirectTarget("operator", new URL(request.url).search), 308);
}