import { signedFetch } from "../../auth/signer";
import type { AuthIdentity } from "../../auth/types";
import { CatalystError, catalystBase } from "../client";
import type { CommunityDraft } from "./create-community";

type CommunityCommitOptions = {
  identity: AuthIdentity;
  base?: string;
  signal?: AbortSignal;
};

async function readError(res: Response): Promise<string> {
  try {
    const body = (await res.json()) as { message?: string; error?: string };
    return body?.message ?? body?.error ?? res.statusText;
  } catch {
    return res.statusText;
  }
}

type CreatedCommunity = { id: string; name: string };

export async function createCommunity(
  draft: CommunityDraft,
  thumbnail: { bytes: Uint8Array; contentType: string } | null,
  opts: CommunityCommitOptions,
): Promise<CreatedCommunity> {
  const fd = new FormData();
  fd.append("name", draft.name.trim());
  fd.append("description", draft.description.trim());
  fd.append("privacy", draft.privacy);
  fd.append("visibility", draft.visibility);
  if (draft.placeIds.length > 0) {
    fd.append("placeIds", JSON.stringify(draft.placeIds));
  }
  if (thumbnail) {
    fd.append(
      "thumbnail",
      new Blob([thumbnail.bytes as unknown as BlobPart], {
        type: thumbnail.contentType,
      }),
      "thumbnail.png",
    );
  }

  const url = `${catalystBase(opts.base)}/v1/communities`;
  let res: Response;
  try {
    res = await signedFetch(opts.identity, url, {
      method: "POST",
      body: fd,
      signal: opts.signal,
    });
  } catch (err) {
    throw new CatalystError(
      `Create community failed: ${(err as Error)?.message ?? "network error"}`,
      url,
    );
  }
  if (!res.ok) {
    throw new CatalystError(
      `Create community failed (${res.status}): ${await readError(res)}`,
      url,
    );
  }
  const body = (await res.json()) as { data?: unknown } & Record<string, unknown>;
  const data = (body?.data ?? body) as { id?: unknown; name?: unknown };
  if (!data?.id) {
    throw new CatalystError("Create community: response had no community id", url);
  }
  return { id: String(data.id), name: String(data.name ?? draft.name) };
}
