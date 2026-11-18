import { configForRequest } from "@/data/lib/creators/config";
import { FLAG_APPLICATIONS, readFlags, type FlagApplication } from "@/data/lib/creators/featureFlags.server";

/**
 * The flags the creators pages read (`?app=builder` or `?app=dapps`), as this server reads them from Decentraland's
 * flag service for the environment the request addresses. The pages treat a failed answer as every flag off.
 */
export async function loader({ request }: { request: Request }) {
  const url = new URL(request.url);
  const application = url.searchParams.get("app") as FlagApplication;
  if (!FLAG_APPLICATIONS.includes(application)) return Response.json({ error: "unknown application" }, { status: 400 });
  try {
    const flags = await readFlags(configForRequest(url), application);
    return Response.json({ flags }, { headers: { "cache-control": "public, max-age=60" } });
  } catch {
    return Response.json({ error: "flags unavailable" }, { status: 503, headers: { "cache-control": "no-store" } });
  }
}
