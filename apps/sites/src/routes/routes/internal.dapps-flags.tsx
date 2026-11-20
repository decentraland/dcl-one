import { dappsFlagsAnswer, knownDappsFlags } from "@/data/lib/feature-flags.server";

/**
 * decentraland.org's `dapps` rollout flags as this server reads them, for a page with no server render (a client
 * navigation into the Shop, the native shop). The bare flags map by default, which is what a page loaded before the
 * variants reads; `?variants=1` answers `{ flags, variants }` with the variants' payloads (an address list, for one).
 */
export async function loader({ request }: { request: Request }) {
  const query = new URL(request.url).searchParams;
  if (query.get("known") === "1") {
    const flags = await knownDappsFlags();
    return flags === null
      ? Response.json({}, { status: 503, headers: { "cache-control": "no-store" } })
      : Response.json(flags, { headers: { "cache-control": "public, max-age=60" } });
  }
  const answer = await dappsFlagsAnswer();
  const withVariants = query.get("variants") === "1";
  return Response.json(withVariants ? answer : answer.flags, { headers: { "cache-control": "public, max-age=60" } });
}
