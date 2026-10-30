import type { AuthIdentity } from "../../auth/types";
import { creatorsDataUrl } from "../bases";
import { signedGetJSON } from "../client";
import type { CreatorScenesStats } from "@ui/creatorhub/lib/scene-analytics";
import { parseCreatorScenesStats } from "./scene-analytics.gen";

export { parseCreatorScenesStats } from "./scene-analytics.gen";

export const SCENE_STATS_PATH = "/creators/me/scenes/stats";

export function creatorsDataBase(override?: string): string {
  return creatorsDataUrl(override);
}

type FetchSceneStatsOptions = {
  base?: string;
  signal?: AbortSignal;
};

export async function fetchCreatorScenesStats(
  identity: AuthIdentity,
  opts: FetchSceneStatsOptions = {},
): Promise<CreatorScenesStats> {
  const raw = await signedGetJSON<unknown>(SCENE_STATS_PATH, {
    identity,
    base: creatorsDataBase(opts.base),
    signal: opts.signal,
  });
  return parseCreatorScenesStats(raw);
}
