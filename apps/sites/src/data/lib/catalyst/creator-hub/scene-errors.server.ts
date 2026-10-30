import { Pool } from "pg";
import { z } from "zod";

import { catalystBase, getJSON, worldsBase } from "../client";
import { fetchParcelPermissions, grantsDeploy, parseParcel } from "./land-rights";

const WorldPermissions = z.object({ owner: z.string().nullish() });
const WorldScenes = z.object({ scenes: z.array(z.object({ entityId: z.string() })) });
const ActiveScenes = z.array(z.object({
  id: z.string().optional(),
  entityId: z.string().optional(),
  type: z.string(),
}));
const EventRow = z.object({
  received_at: z.date(),
  scene_hash: z.string(),
  error_kind: z.string(),
  error_message: z.string(),
});

let pool: Pool | null = null;

function telemetryPool(): Pool {
  const connectionString = process.env.TELEMETRY_DATABASE_URL;
  if (!connectionString) throw new Error("Telemetry database is unavailable");
  if (!pool) pool = new Pool({ connectionString, max: 2 });
  return pool;
}

export async function canReadSceneErrors(
  wallet: string,
  sceneType: string,
  sceneId: string,
): Promise<boolean> {
  if (!/^0x[0-9a-f]{40}$/i.test(wallet)) return false;
  if (sceneType === "genesis") {
    if (!parseParcel(sceneId)) return false;
    const flags = await fetchParcelPermissions(wallet, sceneId);
    return flags !== null && grantsDeploy(flags);
  }
  if (sceneType === "world") {
    if (!/^[a-z0-9.-]{1,128}$/i.test(sceneId)) return false;
    const raw = await getJSON<unknown>(
      `/world/${encodeURIComponent(sceneId)}/permissions`,
      { base: worldsBase() },
    );
    const parsed = WorldPermissions.safeParse(raw);
    return parsed.success && parsed.data.owner?.toLowerCase() === wallet.toLowerCase();
  }
  return false;
}

async function currentSceneHashes(sceneType: string, sceneId: string): Promise<string[]> {
  if (sceneType === "world") {
    const raw = await getJSON<unknown>(
      `/world/${encodeURIComponent(sceneId)}/scenes`,
      { base: worldsBase() },
    );
    return WorldScenes.parse(raw).scenes.map((scene) => scene.entityId).slice(0, 500);
  }
  const response = await fetch(`${catalystBase()}/content/entities/active`, {
    method: "POST",
    headers: { "content-type": "application/json", accept: "application/json" },
    body: JSON.stringify({ pointers: [sceneId] }),
  });
  if (!response.ok) throw new Error("Active scene lookup failed");
  return ActiveScenes.parse(await response.json())
    .filter((scene) => scene.type === "scene")
    .map((scene) => scene.id ?? scene.entityId)
    .filter((hash): hash is string => Boolean(hash));
}

export async function loadSceneErrors(sceneType: string, sceneId: string) {
  const hashes = await currentSceneHashes(sceneType, sceneId);
  if (hashes.length === 0) return [];
  const database = telemetryPool();
  const result = await database.query(
    `SELECT received_at,
            coalesce(body->'properties'->>'scene_hash', '') AS scene_hash,
            coalesce(body->'properties'->>'error_kind', '') AS error_kind,
            left(coalesce(body->'properties'->>'error_message', ''), 2048) AS error_message
       FROM telemetry.events
      WHERE source = 'segment' AND body->>'event' = 'Scene Error'
        AND body->'properties'->>'scene_hash' = ANY($3::text[])
        AND ($1 = 'genesis' AND body->'properties'->>'scene_base' = $2
          OR $1 = 'world' AND lower(body->'properties'->>'scene_world') = lower($2))
      ORDER BY received_at DESC LIMIT 100`,
    [sceneType, sceneId, hashes],
  );
  return result.rows.map((row: unknown) => {
    const parsed = EventRow.parse(row);
    return { ...parsed, received_at: parsed.received_at.toISOString() };
  });
}
