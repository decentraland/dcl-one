import { redirect } from "react-router";

/** The reference's events page: /events?id=<id> is an event's link, and opens it. */
export function loader({ request }: { request: Request }) {
  const { searchParams, search } = new URL(request.url);
  const id = searchParams.get("id");
  return redirect(id ? `/whats-on/${encodeURIComponent(id)}` : `/whats-on${search}`);
}
