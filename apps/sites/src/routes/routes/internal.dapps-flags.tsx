import { dappsFlags } from "@/data/lib/feature-flags.server";

/** decentraland.org's `dapps` rollout flags as this server reads them, for a page with no server render (the native shop). */
export async function loader() {
  return Response.json(await dappsFlags(), { headers: { "cache-control": "public, max-age=60" } });
}
