import type { GetOptions } from "../client";
import { loadManageWorlds, worldNameForName } from "./manage-worlds.server";
import type { OwnedNameVM } from "./names-list";

/** Every NAME the wallet owns, each paired with the World it entitles. */
export async function loadNamesList(
  address: string,
  signal?: AbortSignal,
  opts: Pick<GetOptions, "fetchImpl"> = {},
): Promise<OwnedNameVM[]> {
  const { names, worlds } = await loadManageWorlds(address, signal, opts);
  if (names === null) throw new Error("NAMEs service unreachable");
  const byWorld = new Map(worlds.map((w) => [w.name.toLowerCase(), w]));
  return names
    .map((n) => {
      const worldName = worldNameForName(n.name);
      const world = byWorld.get(worldName) ?? null;
      return {
        name: n.name,
        worldName,
        contractAddress: n.contractAddress,
        tokenId: n.tokenId,
        deployedScenes: world?.deployedScenes ?? 0,
        title: world?.title ?? null,
        lastDeployedAt: world?.lastDeployedAt ?? null,
        thumbnail: world?.thumbnail ?? null,
      };
    })
    .sort((a, b) => a.name.localeCompare(b.name));
}
