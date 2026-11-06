import { getJSON, worldsBase, type GetOptions } from "../client";

type WorldScene = {
  entityId: string;
  parcels: string[];
  baseParcel: string;
};

const PAGE_SIZE = 100;

function record(v: unknown): Record<string, unknown> {
  return v !== null && typeof v === "object" ? (v as Record<string, unknown>) : {};
}

function toWorldScene(s: unknown): WorldScene {
  const o = record(s);
  const parcels = Array.isArray(o.parcels)
    ? o.parcels.filter((p): p is string => typeof p === "string")
    : [];
  // The base parcel lives in the entity's scene metadata (`WorldSceneEntry.entity`).
  const base = record(record(record(o.entity).metadata).scene).base;
  return {
    entityId: typeof o.entityId === "string" ? o.entityId : "",
    baseParcel: typeof base === "string" ? base : "",
    parcels,
  };
}

export async function loadWorldScenes(
  worldName: string,
  opts: GetOptions = {},
): Promise<WorldScene[] | null> {
  try {
    const all: unknown[] = [];
    // The endpoint pages at 100; read pages until `total` is reached.
    for (;;) {
      const raw = await getJSON<{ scenes?: unknown; total?: unknown }>(
        `/world/${encodeURIComponent(worldName)}/scenes`,
        {
          ...opts,
          base: opts.base ?? worldsBase(),
          query: { ...opts.query, limit: PAGE_SIZE, offset: all.length },
        },
      );
      const page = Array.isArray(raw?.scenes) ? raw.scenes : [];
      all.push(...page);
      const total = typeof raw?.total === "number" ? raw.total : all.length;
      if (page.length === 0 || all.length >= total) break;
    }
    return all.map(toWorldScene).filter((s) => s.entityId !== "" && s.baseParcel !== "");
  } catch {
    return null;
  }
}
