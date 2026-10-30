import { legacyPlaceRedirect } from "@/data/lib/catalyst/places/legacy-route.server";

export function loader({ request, params }: { request: Request; params: { name?: string } }) {
  return legacyPlaceRedirect(request, "world", params.name ?? "");
}
