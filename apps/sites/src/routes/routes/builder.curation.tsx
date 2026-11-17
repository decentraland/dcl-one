import { builderRedirect } from "@/features/lib/creator/builder-redirect";

import type { Route } from "./+types/builder.curation";

export async function loader({ request, context }: Route.LoaderArgs) {
  return builderRedirect("curation", request, context);
}
