import { redirect } from "react-router";

// Discover is Places: /discover and every path under it land on /places with the subpath and the query
// intact, as decentraland.org does (sites: src/pages/RenamedSectionRedirect.tsx).
export function loader({ request }: { request: Request }) {
  const url = new URL(request.url);
  return redirect(`/places${url.pathname.slice("/discover".length)}${url.search}`);
}
