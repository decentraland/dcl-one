import { data } from "react-router";

/** `/social/*` beyond a community page: the source's section catch-all, a 404. */
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
