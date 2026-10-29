// from: shop@94584a5c:app/src/lib/emoteBase64.ts
// A published emote as the preview's base64 definition: the entity's metadata with each
// representation's file names resolved to content URLs, so the renderer can download them itself.
// Fail-soft: null means the preview plays no emote.

import { peerUrlFor } from "./urn";

type ActiveEntity = {
  content?: { file: string; hash: string }[];
  metadata?: EmoteMetadata;
};

type EmoteMetadata = {
  emoteDataADR74?: {
    representations?: { contents?: string[] }[];
    loop?: boolean;
    [key: string]: unknown;
  };
  [key: string]: unknown;
};

export function isEmoteUrn(emote: string): emote is `urn:decentraland:${string}` {
  return emote.startsWith("urn:");
}

export type EmotePlayback = { base64: string; loop: boolean };

export function toEmoteBase64(entity: ActiveEntity, peerUrl: string): EmotePlayback | null {
  const data = entity.metadata?.emoteDataADR74;
  if (!data?.representations?.length) return null;
  const hashes = new Map((entity.content ?? []).map((c) => [c.file, c.hash]));
  const representations = data.representations.map((rep) => ({
    ...rep,
    contents: (rep.contents ?? []).filter((key) => hashes.has(key)).map((key) => ({ key, url: `${peerUrl}/content/contents/${hashes.get(key)}` })),
  }));
  if (representations.every((rep) => rep.contents.length === 0)) return null;
  return { base64: encode({ ...entity.metadata, emoteDataADR74: { ...data, representations } }), loop: !!data.loop };
}

function encode(definition: unknown): string {
  return btoa(JSON.stringify(definition).replace(/[^\x20-\x7f]/g, ""));
}

export async function fetchEmoteBase64(urn: string, peerUrl: string = peerUrlFor([urn]), signal?: AbortSignal): Promise<EmotePlayback | null> {
  try {
    const res = await fetch(`${peerUrl}/content/entities/active`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ pointers: [urn] }),
      signal,
    });
    if (!res.ok) {
      await res.body?.cancel();
      return null;
    }
    const [entity] = (await res.json()) as ActiveEntity[];
    return entity ? toEmoteBase64(entity, peerUrl) : null;
  } catch {
    return null;
  }
}

type RepresentationEntity = {
  content?: { file: string; hash: string }[];
  metadata?: { emoteDataADR74?: { representations?: { bodyShapes?: string[]; mainFile?: string; contents?: string[] }[] } };
};

/**
 * ui3's preview (wearable-preview) plays an emote from its GLB URL rather than from a base64 definition,
 * so this resolves the representation for `bodyShape` (else the first) to its content URL.
 */
export function emoteClipUrl(entity: RepresentationEntity, peerUrl: string, bodyShape?: string): string | null {
  const reps = entity.metadata?.emoteDataADR74?.representations ?? [];
  const rep = (bodyShape ? reps.find((r) => r.bodyShapes?.some((b) => b.toLowerCase() === bodyShape.toLowerCase())) : undefined) ?? reps[0];
  const file = rep?.mainFile;
  if (!file) return null;
  const hash = (entity.content ?? []).find((c) => c.file.toLowerCase() === file.toLowerCase())?.hash;
  return hash ? `${peerUrl}/content/contents/${hash}` : null;
}

export async function fetchEmoteClipUrl(urn: string, bodyShape?: string, signal?: AbortSignal): Promise<string | null> {
  const peerUrl = peerUrlFor([urn]);
  try {
    const res = await fetch(`${peerUrl}/content/entities/active`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ pointers: [urn] }),
      signal,
    });
    if (!res.ok) return null;
    const [entity] = (await res.json()) as RepresentationEntity[];
    return entity ? emoteClipUrl(entity, peerUrl, bodyShape) : null;
  } catch {
    return null;
  }
}
