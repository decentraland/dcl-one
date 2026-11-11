import { builderRedirect } from "@/features/lib/creator/builder-redirect";

import type { Route } from "./+types/builder.collection_.$id";

export async function loader({ request, params, context }: Route.LoaderArgs) {
  return builderRedirect("collection-detail", request, context, params);
}
