import { useQuery } from "@tanstack/react-query";
import { catalystBase, getJSON, sendJSON, type RequestOpts } from "./catalyst/client";
import { worldRealmBase } from "./catalyst/sceneOwner";
import { previewLaunch, toPreviewManifest, type PreviewSpawn } from "./previewMode";

/** A scene the realm serves, as drawn on the realm map. */
export type RealmScene = { id: string; title: string; base: [number, number]; parcels: [number, number][]; spawns: PreviewSpawn[] };
export type RealmBounds = { minX: number; minY: number; maxX: number; maxY: number };

/** Where a realm map applies: a scene preview (its server), or a world (its content base). Genesis
    City keeps the atlas: it has a map service of its own. */
export function realmMapBase(search: string, sceneRealm: string | null | undefined): string | null {
  const launch = previewLaunch(search);
  if (launch.preview) return launch.origin;
  if (!sceneRealm) return null;
  let launchRealm: string | null = null;
  try {
    launchRealm = new URLSearchParams(search).get("realm");
  } catch {
    launchRealm = null;
  }
  const fromUrl = worldRealmBase(sceneRealm, launchRealm);
  if (fromUrl) return fromUrl;
  if (/^[\w.-]+\.eth$/i.test(sceneRealm.trim())) return `${catalystBase()}/world/${encodeURIComponent(sceneRealm.trim())}`;
  return null;
}

const isRecord = (v: unknown): v is Record<string, unknown> => !!v && typeof v === "object" && !Array.isArray(v);
const field = (v: unknown, key: string): unknown => (isRecord(v) ? v[key] : undefined);
const strings = (v: unknown): string[] => (Array.isArray(v) ? v.filter((s): s is string => typeof s === "string") : []);

export function parsePointer(p: string): [number, number] | null {
  const m = /^\s*(-?\d+)\s*,\s*(-?\d+)\s*$/.exec(p);
  return m ? [Number(m[1]), Number(m[2])] : null;
}

export function toRealmScene(entity: unknown): RealmScene | null {
  if (!isRecord(entity)) return null;
  const parcels = strings(entity.pointers).map(parsePointer).filter((p): p is [number, number] => !!p);
  if (!parcels.length) return null;
  const id = typeof entity.id === "string" ? entity.id : parcels.map((p) => p.join(",")).join(";");
  const fallbackBase: [number, number] = parcels.reduce((acc, p) => [Math.min(acc[0], p[0]), Math.min(acc[1], p[1])], [Infinity, Infinity]) as [number, number];
  const metadata = isRecord(entity.metadata) ? entity.metadata : {};
  const manifest = toPreviewManifest({ ...metadata, scene: { base: `${fallbackBase[0]},${fallbackBase[1]}`, ...(isRecord(metadata.scene) ? metadata.scene : {}) } });
  return {
    id,
    title: manifest?.title ?? "Untitled scene",
    base: manifest?.base ?? fallbackBase,
    parcels,
    spawns: manifest?.spawns ?? [],
  };
}

export function realmBounds(scenes: RealmScene[]): RealmBounds | null {
  let b: RealmBounds | null = null;
  for (const s of scenes) for (const [x, y] of s.parcels) {
    b = b ? { minX: Math.min(b.minX, x), minY: Math.min(b.minY, y), maxX: Math.max(b.maxX, x), maxY: Math.max(b.maxY, y) } : { minX: x, minY: y, maxX: x, maxY: y };
  }
  return b;
}

export function sceneAt(scenes: RealmScene[], x: number, y: number): RealmScene | null {
  return scenes.find((s) => s.parcels.some((p) => p[0] === x && p[1] === y)) ?? null;
}

const POINTER_CHUNK = 200;

/** Every scene the realm serves: local scene parcels resolved through the content API the way a
    client does, plus any scenesUrn entities a world pins. Deduplicated by entity id. */
export async function fetchRealmScenes(base: string, opts: RequestOpts = {}): Promise<RealmScene[]> {
  const about = await getJSON("/about", { ...opts, base });
  const config = field(about, "configurations");
  const publicUrl = field(field(about, "content"), "publicUrl");
  const contentBase = typeof publicUrl === "string" && /^https?:\/\//.test(publicUrl) ? publicUrl.replace(/\/$/, "") : `${base}/content`;
  const pointers = strings(field(config, "localSceneParcels")).filter((p) => parsePointer(p));
  const entities: unknown[] = [];
  for (let i = 0; i < pointers.length; i += POINTER_CHUNK) {
    const chunk = pointers.slice(i, i + POINTER_CHUNK);
    const batch = await sendJSON<unknown>("/entities/active", { ...opts, base: contentBase, method: "POST", body: { pointers: chunk } });
    if (Array.isArray(batch)) entities.push(...batch);
  }
  for (const raw of strings(field(config, "scenesUrn"))) {
    if (!raw.startsWith("urn:decentraland:entity:")) continue;
    let id = "";
    let contents = `${contentBase}/contents`;
    try {
      const urn = new URL(raw);
      id = urn.pathname.split(":").at(-1) ?? "";
      const baseUrl = urn.searchParams.get("baseUrl");
      if (baseUrl && /^https?:\/\//.test(baseUrl)) contents = baseUrl.replace(/\/$/, "");
    } catch {
      continue;
    }
    if (!id) continue;
    const entity = await getJSON<unknown>(`/${encodeURIComponent(id)}`, { ...opts, base: contents }).catch((error: unknown) => {
      if (opts.signal?.aborted) throw error;
      return null;
    });
    if (isRecord(entity)) entities.push({ ...entity, id });
  }
  const seen = new Set<string>();
  const scenes: RealmScene[] = [];
  for (const e of entities) {
    const scene = toRealmScene(e);
    if (!scene || seen.has(scene.id)) continue;
    seen.add(scene.id);
    scenes.push(scene);
  }
  return scenes;
}

export const realmMapKeys = { scenes: (base: string | null) => ["realm-scenes", base] as const };

export function useRealmScenes(base: string | null) {
  return useQuery({
    queryKey: realmMapKeys.scenes(base),
    queryFn: ({ signal }) => fetchRealmScenes(base ?? "", { signal }),
    enabled: !!base,
    staleTime: 30_000,
    retry: false,
  });
}
