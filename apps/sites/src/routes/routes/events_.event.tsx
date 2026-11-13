import { redirect } from "react-router";

/** The reference's legacy event link, /events/event?id=<id>: the event's page, or What's On without an id. */
export function loader({ request }: { request: Request }) {
  const id = new URL(request.url).searchParams.get("id");
  return redirect(id ? `/whats-on/${encodeURIComponent(id)}` : "/whats-on");
}
