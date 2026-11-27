import { data } from "react-router";

/** `/social` and `/social/*` beyond a community page: decentraland.org answers them 404, and so does this site. */
export function loader() {
  throw data(null, { status: 404, statusText: "Not Found" });
}

export function meta() {
  return [{ title: "Page not found | Decentraland" }];
}

/** Never rendered: the loader always answers 404, which the root boundary renders. */
export default function SocialNotFound() {
  return null;
}
