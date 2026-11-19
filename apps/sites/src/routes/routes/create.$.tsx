import { redirect } from "react-router";

import { CREATOR_HUB_DOWNLOAD, isRetiredHubPage } from "@/features/lib/creator/retired-hub";

import type { Route } from "./+types/create.$";

/** Old links to the retired web Creator Hub pages go to the desktop app's download page. */
export function loader({ params }: Route.LoaderArgs) {
  const [head] = (params["*"] ?? "").split("/");
  if (isRetiredHubPage(head)) return redirect(CREATOR_HUB_DOWNLOAD, 308);
  throw new Response(null, { status: 404, statusText: "Not Found" });
}

export default function CreateRetired() {
  return null;
}
