import { legacyPlaceRedirect } from "@/data/lib/catalyst/places/legacy-route.server";

export function loader({ request, params }: { request: Request; params: { position?: string } }) {
  return legacyPlaceRedirect(request, "place", params.position ?? "");
}
