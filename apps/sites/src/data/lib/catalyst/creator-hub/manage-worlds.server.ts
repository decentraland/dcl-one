import { z } from "zod";

import { CatalystError, getJSON, publicWorldsBase, worldsBase } from "../client";
import type { GetOptions } from "../client";
import { withDeadline } from "../../request-deadline";
import { WcsWorldRowSchema, toManagedWorld } from "../wcs";
import { fetchWorldsRealm, personalWorldName } from "./deploy-world";
import {
  parseManagedWorlds,
  parseNames,
  normalizeAddress,
  type ManagedWorld,
  type DclName,
} from "./manage-worlds";

type ManageWorldsData = {
  address: string;
  worlds: ManagedWorld[];
  /** null: the NAMEs service did not answer (the worlds list did). */
  names: DclName[] | null;
};

const WorldAboutSchema = z.object({
  configurations: z
    .object({
      scenesUrn: z.array(z.string()),
    })
    .nullish()
    .transform((v) => v ?? null),
});

const LIST_DEADLINE_MS = 5000;

const WorldsListSchema = z.object({ worlds: z.array(z.unknown()) });

function parseWorldsList(raw: unknown, base: string): ManagedWorld[] {
  const out: ManagedWorld[] = [];
  for (const row of WorldsListSchema.parse(raw).worlds) {
    const parsed = WcsWorldRowSchema.safeParse(row);
    if (parsed.success) out.push(toManagedWorld(parsed.data, base));
  }
  return out;
}

export function worldNameForName(name: string): string {
  const bare = name
    .trim()
    .toLowerCase()
    .replace(/\.(dcl\.eth|eth)$/i, "");
  return `${bare}.dcl.eth`;
}

export async function fetchLiveNames(
  address: string,
  opts: GetOptions = {},
): Promise<DclName[]> {
  const raw = await getJSON<{ elements?: unknown[] }>(
    `/lambdas/users/${encodeURIComponent(normalizeAddress(address))}/names`,
    { ...opts, query: { pageSize: 100 } },
  );
  return parseNames(raw?.elements ?? []);
}

async function resolveWorldScenes(
  worldName: string,
  opts: GetOptions = {},
): Promise<number> {
  const path = `/world/${encodeURIComponent(worldName)}/about`;
  let about: unknown;
  try {
    about = await getJSON<unknown>(path, opts);
  } catch (err) {
    if (err instanceof CatalystError && err.status === 404) return 0;
    throw err;
  }
  const parsed = WorldAboutSchema.safeParse(about);
  if (!parsed.success || parsed.data.configurations === null) {
    throw new CatalystError("world about carried no scene list", path);
  }
  return parsed.data.configurations.scenesUrn.length;
}

function worldNamesFor(names: DclName[], address: string, personalWorlds: boolean): string[] {
  const out = names.map((n) => worldNameForName(n.name));
  const personal = personalWorlds ? personalWorldName(address) : null;
  if (personal && !out.includes(personal)) out.push(personal);
  return out;
}

async function realmOffersPersonalWorlds(opts: GetOptions): Promise<boolean> {
  try {
    return (await fetchWorldsRealm(opts)).personalWorlds;
  } catch {
    return false;
  }
}

/** Every world the wallet may deploy to, as the worlds registry knows them: owned worlds,
    the personal world, and worlds whose owners added the wallet as a deployer. `null` when
    the registry could not be asked. */
async function fetchAuthorizedWorlds(
  wallet: string,
  opts: GetOptions,
): Promise<ManagedWorld[] | null> {
  try {
    const raw = await withDeadline(
      (signal) =>
        getJSON<unknown>("/worlds", {
          ...opts,
          signal,
          base: worldsBase(),
          query: { authorized_deployer: wallet, limit: 1000 },
        }),
      LIST_DEADLINE_MS,
      opts.signal,
    );
    return parseWorldsList(raw, publicWorldsBase());
  } catch {
    opts.signal?.throwIfAborted();
    return null;
  }
}

function roleFor(world: ManagedWorld, wallet: string, ownedNames: Set<string>): ManagedWorld["role"] {
  const name = world.name.toLowerCase();
  if (normalizeAddress(world.owner) === wallet) return "owner";
  if (ownedNames.has(name) || name === personalWorldName(wallet)) return "owner";
  return "collaborator";
}

/** Worlds that exist only as a NAME (never deployed, unknown to the registry). */
async function unlistedNameWorlds(
  worldNames: string[],
  listed: Map<string, ManagedWorld>,
  wallet: string,
  askAbout: boolean,
  opts: GetOptions,
): Promise<ManagedWorld[]> {
  const missing = worldNames.filter((name) => !listed.has(name));
  const resolved = await Promise.all(
    missing.map(async (name) => ({
      name,
      owner: wallet,
      title: null,
      deployedScenes: askAbout ? await resolveWorldScenes(name, opts) : 0,
      role: "owner" as const,
    })),
  );
  return parseManagedWorlds(resolved);
}

export async function loadManageWorlds(
  address: string,
  signal?: AbortSignal,
  opts: Pick<GetOptions, "fetchImpl"> = {},
): Promise<ManageWorldsData> {
  const wallet = normalizeAddress(address);
  if (!wallet) {
    return {
      address,
      worlds: [],
      names: [],
    };
  }

  const get: GetOptions = { signal, fetchImpl: opts.fetchImpl };
  let namesError: Error | null = null;
  const [names, personalWorlds, listed] = await Promise.all([
    fetchLiveNames(address, get).catch((err: unknown) => {
      signal?.throwIfAborted();
      namesError = err instanceof Error ? err : new Error("NAMEs service unreachable");
      return null;
    }),
    realmOffersPersonalWorlds(get),
    fetchAuthorizedWorlds(wallet, get),
  ]);
  if (namesError !== null && listed === null) throw namesError;

  const ownedNames = new Set((names ?? []).map((n) => worldNameForName(n.name)));
  const byName = new Map<string, ManagedWorld>();
  for (const world of listed ?? []) {
    const key = world.name.toLowerCase();
    byName.set(key, { ...world, name: key, role: roleFor(world, wallet, ownedNames) });
  }
  const fromNames = await unlistedNameWorlds(
    worldNamesFor(names ?? [], wallet, personalWorlds),
    byName,
    wallet,
    listed === null,
    get,
  );
  for (const world of fromNames) byName.set(world.name, world);

  return {
    address,
    worlds: [...byName.values()],
    names,
  };
}
