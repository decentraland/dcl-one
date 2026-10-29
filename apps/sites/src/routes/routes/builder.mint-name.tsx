import { redirect } from "react-router";

import type { Route } from "./+types/builder.mint-name";

// The Builder's "mint a NAME" entry is the Shop's NAME registration, query kept.
export async function loader({ request }: Route.LoaderArgs) {
  return redirect(`/shop/names${new URL(request.url).search}`, 308);
}
