import { redirect } from "react-router";

import { retiredHubTarget } from "@/features/lib/creator/retired-hub";

import type { Route } from "./+types/create.$";

/** Old links to the retired web Creator Hub pages go to the page that now does their job. */
export function loader({ params }: Route.LoaderArgs) {
  const [head] = (params["*"] ?? "").split("/");
  const target = retiredHubTarget(head);
  if (target) return redirect(target, 308);
  throw new Response(null, { status: 404, statusText: "Not Found" });
}

export default function CreateRetired() {
  return null;
}
