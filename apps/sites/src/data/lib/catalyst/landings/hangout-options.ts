import { z } from "zod";
import type { AuthIdentity } from "../../auth/types";
import { getJSON, signedGetJSON } from "../client";
import { loadEventProfile } from "../places/event-management";
import { fetchSchedules } from "./schedules";

export type CommunityOption = { id: string; name: string };
export type FeaturedAsset = { urn: string; name: string; thumbnail: string | null; kind: "item" | "collection" };
export const validFeaturedUrn = (value: string): boolean => /^urn:decentraland:(matic|ethereum|amoy|sepolia):collections-v2:0x[0-9a-fA-F]{40}(?::\d+)?$/.test(value) && value.length <= 160;
const Communities = z.object({ data: z.object({ results: z.array(z.object({ id: z.string(), name: z.string(), role: z.string().nullable().optional() })), total: z.number().optional() }) });

export async function loadManageableCommunities(identity: AuthIdentity, signal?: AbortSignal): Promise<CommunityOption[]> {
  const found = new Map<string, CommunityOption>();
  let offset = 0;
  for (;;) {
    const query = new URLSearchParams({ onlyMemberOf: "true", limit: "100", offset: String(offset) });
    query.append("roles", "owner"); query.append("roles", "moderator");
    const response = Communities.parse(await signedGetJSON(`/social-api/v1/communities?${query}`, { identity, signal })).data;
    const previous = found.size;
    for (const community of response.results) {
      if (community.role === "owner" || community.role === "moderator") found.set(community.id, { id: community.id, name: community.name });
    }
    offset += response.results.length;
    if (response.results.length < 100 || (response.total !== undefined && offset >= response.total)) break;
    if (found.size === previous) throw new Error("The community list did not advance. Please retry.");
  }
  return [...found.values()];
}

export async function loadHangoutOptions(identity: AuthIdentity, signal?: AbortSignal) {
  const [profile, communities, schedules, worlds] = await Promise.allSettled([
    loadEventProfile(identity, signal), loadManageableCommunities(identity, signal), fetchSchedules({ signal }),
    getJSON("/events/api/world_names", { signal }).then((raw) => z.object({ data: z.array(z.string()) }).parse(raw).data),
  ]);
  const error = (result: PromiseSettledResult<unknown>) => result.status === "rejected" ? result.reason instanceof Error ? result.reason.message : "Could not load this list." : null;
  return {
    profile: profile.status === "fulfilled" ? profile.value : null, profileError: error(profile),
    communities: communities.status === "fulfilled" ? communities.value : [], communitiesError: error(communities),
    schedules: schedules.status === "fulfilled" ? schedules.value.data : [], schedulesError: error(schedules),
    worlds: worlds.status === "fulfilled" ? worlds.value : [], worldsError: error(worlds),
  };
}
export type HangoutOptions = Awaited<ReturnType<typeof loadHangoutOptions>>;

const Assets = z.object({ data: z.array(z.object({ urn: z.string().nullable(), name: z.string().nullable().optional(), thumbnail: z.string().nullable().optional() })) });

export async function searchFeaturedAssets(search: string, signal?: AbortSignal): Promise<FeaturedAsset[]> {
  const query = search.trim();
  if (query.length < 3) return [];
  const urn = validFeaturedUrn(query);
  const collection = urn && !/:\d+$/.test(query);
  const requests = (urn ? [collection ? "collection" : "item"] : ["item", "collection"]).map(async (kind) => {
    const result = Assets.parse(await getJSON(`/marketplace-api/v1/${kind === "item" ? "items" : "collections"}`, {
      signal, query: { ...(urn ? { urn: query } : { search: query }), first: kind === "item" ? 12 : 6 },
    }));
    return result.data.filter((asset) => asset.urn !== null && validFeaturedUrn(asset.urn)).map((asset) => ({ urn: asset.urn ?? "", name: asset.name || "Untitled asset", thumbnail: asset.thumbnail ?? null, kind: kind === "item" ? "item" as const : "collection" as const }));
  });
  const results = await Promise.all(requests);
  const assets = [...new Map(results.flat().map((asset) => [asset.urn, asset])).values()];
  // A well-formed URN can identify an unlisted native collection; preserve the explicit selection after a successful lookup.
  return assets.length === 0 && urn ? [{ urn: query, name: query, thumbnail: null, kind: collection ? "collection" : "item" }] : assets;
}
