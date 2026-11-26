import { builderRedirect } from "@/features/lib/creator/builder-redirect";

import type { Route } from "./+types/builder.publish-collection";

export async function loader({ request, context }: Route.LoaderArgs) {
  return builderRedirect("publish-collection", request, context);
}
