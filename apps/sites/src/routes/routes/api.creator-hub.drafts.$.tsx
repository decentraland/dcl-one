// The drafts sites kept on its own disk, for their one-time move to the
// node's builder projects (scene-drafts-client.ts importLegacyDrafts): the
// signer lists and reads their own, then deletes each once the node holds
// it. Saves go to the node; the delete, the one write left here, waits on
// the node's creator writes (CREATE_WRITES_ENABLED and its restore stamps).
import { isSafeDraftId } from "@/data/lib/catalyst/creator-hub/scene-drafts";
import {
  DraftStoreUnavailable,
  deleteDraft,
  getDraft,
  listDrafts,
  nodeCreateWrites,
  verifyAuthChainRequest,
} from "@/data/lib/catalyst/creator-hub/scene-drafts.server";

type RouteArgs = {
  request: Request;
  params: Record<string, string | undefined>;
};

function json(
  status: number,
  body: unknown,
  extraHeaders: Record<string, string> = {},
): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "content-type": "application/json",
      "cache-control": "no-store",
      ...extraHeaders,
    },
  });
}

function draftId(params: Record<string, string | undefined>): string {
  return (params["*"] ?? "").trim();
}

export async function loader({ request, params }: RouteArgs) {
  const auth = await verifyAuthChainRequest(request);
  if (!auth.ok) return json(auth.status, { error: auth.error });

  const id = draftId(params);
  if (id && !isSafeDraftId(id)) return json(400, { error: "invalid draft id" });

  try {
    if (!id) {
      return json(200, { drafts: await listDrafts(auth.wallet) });
    }
    const draft = await getDraft(auth.wallet, id);
    if (!draft) return json(404, { error: "draft not found" });
    return json(200, draft);
  } catch (err) {
    return storeFailure(err);
  }
}

function storeFailure(err: unknown): Response {
  if (err instanceof DraftStoreUnavailable) {
    console.error("[creator-hub drafts]", err.message);
    return json(503, { error: err.message }, { "retry-after": "30" });
  }
  throw err;
}

export async function action({ request, params }: RouteArgs) {
  if (request.method.toUpperCase() !== "DELETE") {
    return json(
      405,
      { error: "scene drafts are saved to the node's builder projects (/builder-api/v1/projects)" },
      { Allow: "GET, DELETE" },
    );
  }

  const auth = await verifyAuthChainRequest(request);
  if (!auth.ok) return json(auth.status, { error: auth.error });

  const id = draftId(params);
  if (!id) return json(400, { error: "draft id is required" });
  if (!isSafeDraftId(id)) return json(400, { error: "invalid draft id" });

  const writes = await nodeCreateWrites();
  if (!writes.open) {
    return json(
      503,
      { error: `drafts are deleted only while the node takes creator writes: ${writes.reason}` },
      { "retry-after": "3600" },
    );
  }

  try {
    await deleteDraft(auth.wallet, id);
    return new Response(null, { status: 204, headers: { "cache-control": "no-store" } });
  } catch (err) {
    return storeFailure(err);
  }
}
