import { legacyRedirect } from "@/core/lib/telemetry/legacy-redirect";

// Discover is Places: /discover and every path under it land on /places with the subpath and the query
// intact, as decentraland.org does.
export function loader({ request }: { request: Request }) {
  const url = new URL(request.url);
  return legacyRedirect(request, "places", `/places${url.pathname.slice("/discover".length)}${url.search}`);
}
