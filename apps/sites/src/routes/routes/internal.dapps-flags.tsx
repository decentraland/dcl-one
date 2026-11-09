import { dappsFlagsAnswer } from "@/data/lib/feature-flags.server";

/**
 * decentraland.org's `dapps` rollout flags as this server reads them, for a page with no server render (a client
 * navigation into the Shop, the native shop). The bare flags map by default, which is what a page loaded before the
 * variants reads; `?variants=1` answers `{ flags, variants }` with the variants' payloads (an address list, for one).
 */
export async function loader({ request }: { request: Request }) {
  const answer = await dappsFlagsAnswer();
  const withVariants = new URL(request.url).searchParams.get("variants") === "1";
  return Response.json(withVariants ? answer : answer.flags, { headers: { "cache-control": "public, max-age=60" } });
}
