import { flagsAnswer, isFlagApplication, isUnread } from "@/data/lib/feature-flags.server";

/**
 * decentraland.org's rollout flags of one application (`?app=dapps`, the default, or `?app=builder`) as this server
 * reads them, for a page with no server render (a client navigation into the Shop or the creators pages, the native
 * shop). The bare flags map by default, which is what a page loaded before the variants reads; `?variants=1` answers
 * `{ flags, variants }` with the variants' payloads (an address list, for one).
 */
export async function loader({ request }: { request: Request }) {
  const url = new URL(request.url);
  const application = url.searchParams.get("app") ?? "dapps";
  if (!isFlagApplication(application)) {
    return Response.json({ error: "unknown application" }, { status: 400, headers: { "cache-control": "no-store" } });
  }
  const answer = await flagsAnswer(application);
  // Not a read: a page asking again soon must not get "all off" from a cache, and treats the failure as off anyway.
  if (isUnread(answer)) {
    return Response.json({ error: "flags unavailable" }, { status: 503, headers: { "cache-control": "no-store" } });
  }
  const withVariants = url.searchParams.get("variants") === "1";
  return Response.json(withVariants ? answer : answer.flags, { headers: { "cache-control": "public, max-age=60" } });
}
