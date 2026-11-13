import { redirect } from "react-router";

/** The invite moved to decentraland.org's path, /invite/<referrer>; the other query parameters stay. */
export function loader({ request }: { request: Request }) {
  const url = new URL(request.url);
  const referrer = url.searchParams.get("referrer")?.trim() ?? "";
  url.searchParams.delete("referrer");
  return redirect(`/invite${referrer ? `/${encodeURIComponent(referrer)}` : ""}${url.search}`, 308);
}
