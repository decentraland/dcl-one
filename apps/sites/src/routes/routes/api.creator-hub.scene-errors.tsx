import { verifyAuthChainRequest } from "@/data/lib/catalyst/creator-hub/scene-drafts.server";
import { canReadSceneErrors, loadSceneErrors } from "@/data/lib/catalyst/creator-hub/scene-errors.server";

export async function loader({ request }: { request: Request }) {
  const headers = { "Cache-Control": "private, no-store" };
  const auth = await verifyAuthChainRequest(request);
  if (!auth.ok) return Response.json({ error: auth.error }, { status: auth.status, headers });
  const url = new URL(request.url);
  const sceneType = url.searchParams.get("type") ?? "";
  const sceneId = url.searchParams.get("scene") ?? "";
  if (!sceneId || sceneId.length > 128 || !["genesis", "world"].includes(sceneType)) {
    return Response.json({ error: "Invalid scene" }, { status: 400, headers });
  }
  try {
    if (!await canReadSceneErrors(auth.wallet, sceneType, sceneId)) {
      return Response.json({ error: "Scene access denied" }, { status: 403, headers });
    }
    return Response.json({ errors: await loadSceneErrors(sceneType, sceneId) }, { headers });
  } catch {
    return Response.json({ error: "Scene diagnostics unavailable" }, { status: 503, headers });
  }
}
