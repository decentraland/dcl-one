import { builderRedirect } from "@/features/lib/creator/builder-redirect";

import type { Route } from "./+types/builder.item-editor";

export async function loader({ request, context }: Route.LoaderArgs) {
  return builderRedirect("item-editor", request, context);
}
