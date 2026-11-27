import { data } from "react-router";

/**
 * `/communities` and everything under it (`/communities/new` included): decentraland.org answers
 * them 404, and so does this site. The route keeps the prefix with Sites, so the gateway sends these
 * paths here and the visitor gets the site's not-found page, not the API's.
 */
export function loader() {
  throw data(null, { status: 404, statusText: "Not Found" });
}

export function meta() {
  return [{ title: "Page not found | Decentraland" }];
}

/** Never rendered: the loader always answers 404, which the root boundary renders. */
export default function CommunitiesNotFound() {
  return null;
}
