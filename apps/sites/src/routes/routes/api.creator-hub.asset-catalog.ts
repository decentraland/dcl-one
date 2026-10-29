import { loadAssetCatalog } from "@/data/lib/catalyst/creator-hub/asset-catalog.server";
import { withDeadline } from "@/data/lib/request-deadline";

// Public builder assets are shared across projects. Keep them out of navigation
// HTML and let subsequent editor visits reuse the browser's cached response.
export async function loader({ request }: { request: Request }) {
  const catalog = await withDeadline(signal => loadAssetCatalog({ signal }), 3000, request.signal)
    .catch(() => {
      request.signal.throwIfAborted();
      return null;
    });
  return catalog === null
    ? Response.json({ error: "Asset catalog unavailable" }, { status: 503, headers: { "Cache-Control": "no-store" } })
    : Response.json(catalog, { headers: { "Cache-Control": "public, max-age=300, stale-while-revalidate=3600" } });
}
