// from: shop@94584a5c:app/src/lib/builder.ts (fetchItemVideoUrl, fetchItemContents, contentUrl);
//       marketplace@f3956826:webapp/src/lib/asset.ts (getSmartWearableRequiredPermissions, getSmartWearableVideoShowcase),
//       webapp/src/modules/vendor/decentraland/builder/api.ts (fetchItemContent)
// Licence of the Marketplace parts: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.
// A smart wearable's showcase video and required permissions. The video is in the Builder's per-item
// contents map (the Builder boundary, `/builder-api/v1/items/{collection}/{item}/contents`); the
// permissions are in the wearable's scene.json on the content server. Both are presentation, so both
// fail soft: no answer is no button and no permission list.

import { catalystBase } from "../../data/catalyst/client";
import { peerUrlFor } from "./lib/urn";

const VIDEO_FILE = "video.mp4";
const SCENE_FILE = "scene.json";

function builderV1(): string {
  return `${catalystBase()}/builder-api/v1`;
}

export function builderContentUrl(hash: string): string {
  return `${builderV1()}/storage/contents/${hash}`;
}

/** The Builder's `file name -> hash` map for a published item; empty when unavailable. */
export async function fetchItemContents(contractAddress: string, itemId: string, signal?: AbortSignal): Promise<Record<string, string>> {
  try {
    const res = await fetch(`${builderV1()}/items/${contractAddress.toLowerCase()}/${itemId}/contents`, { signal });
    if (!res.ok) return {};
    const json = (await res.json()) as { data?: Record<string, string> } | Record<string, string>;
    const map = ((json as { data?: Record<string, string> }).data ?? json) as Record<string, unknown>;
    return Object.fromEntries(Object.entries(map).filter((e): e is [string, string] => typeof e[1] === "string"));
  } catch {
    return {};
  }
}

/** The showcase video's URL, or null when the creator uploaded none (or the Builder cannot say). */
export async function fetchItemVideoUrl(contractAddress: string, itemId: string, signal?: AbortSignal): Promise<string | null> {
  const contents = await fetchItemContents(contractAddress, itemId, signal);
  const key = Object.keys(contents).find((name) => name.endsWith(VIDEO_FILE));
  return key ? builderContentUrl(contents[key]!) : null;
}

type ActiveEntity = { content?: { file: string; hash: string }[] };

/** The permissions a smart wearable's scene asks for (scene.json `requiredPermissions`); [] when none or unknown. */
export async function fetchSmartWearableRequiredPermissions(urn: string, signal?: AbortSignal): Promise<string[]> {
  const peer = peerUrlFor([urn]);
  try {
    const res = await fetch(`${peer}/content/entities/active`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ pointers: [urn] }),
      signal,
    });
    if (!res.ok) return [];
    const [entity] = (await res.json()) as ActiveEntity[];
    const scene = entity?.content?.find((c) => c.file.endsWith(SCENE_FILE));
    if (!scene) return [];
    const sceneRes = await fetch(`${peer}/content/contents/${scene.hash}`, { signal });
    if (!sceneRes.ok) return [];
    const json = (await sceneRes.json()) as { requiredPermissions?: unknown };
    return Array.isArray(json.requiredPermissions) ? json.requiredPermissions.filter((p): p is string => typeof p === "string") : [];
  } catch {
    return [];
  }
}
