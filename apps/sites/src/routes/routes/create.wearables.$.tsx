import { redirect } from "react-router";

import { retiredCreateDestination } from "@/features/lib/creator/retired-create-redirect";

import type { Route } from "./+types/create.wearables.$";

export function loader({ request }: Route.LoaderArgs) {
  return redirect(retiredCreateDestination(new URL(request.url)), 308);
}
