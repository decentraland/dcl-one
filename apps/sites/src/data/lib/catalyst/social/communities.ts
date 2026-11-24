import { z } from "zod";

import { getJSON } from "../client";
import type { GetOptions } from "../client";
import { dataOf } from "../envelope";
import { readCopy, type CopyStamp } from "@web-core/freshness/copyAsOf";
import { CommunityMemberWireSchema } from "../generated-schemas/communities";

const nullableStr = z.string().nullish().transform((v) => v ?? null);

export const CommunityMemberSchema = CommunityMemberWireSchema;
export type CommunityMember = z.infer<typeof CommunityMemberSchema>;

const CommunityEventSchema = z.object({
  id: z.string(),
  name: z.string(),
  image: nullableStr,
  creatorName: nullableStr,
  timeLabel: nullableStr,
});
type CommunityEvent = z.infer<typeof CommunityEventSchema>;

export const CommunitySchema = z.object({
  id: z.string(),
  name: z.string(),
  description: nullableStr,
  ownerAddress: z.string(),
  ownerName: nullableStr,
  thumbnailUrl: nullableStr,
  privacy: z.enum(["public", "private"]),
  visibility: z.enum(["all", "unlisted"]).nullish().transform((v) => v ?? null),
  membersCount: z.number().nullish().transform((v) => v ?? null),
  role: z.string().nullish().transform((v) => v ?? null),
});
export type Community = z.infer<typeof CommunitySchema>;

export type CommunityDetail = {
  community: Community;
  members: CommunityMember[];
  events: CommunityEvent[];
  source: "live" | "fixture";
  /** When the community is a mirrored copy: its mirror's stamp. */
  copy?: CopyStamp | null;
};

function projectNode(node: unknown, source: "live" | "fixture"): CommunityDetail | null {
  const obj = (node ?? {}) as Record<string, unknown>;

  const community = CommunitySchema.safeParse(obj);
  if (!community.success) {
    console.warn("[communities] community failed validation:", community.error.message);
    return null;
  }

  const members = z
    .array(CommunityMemberSchema)
    .safeParse(Array.isArray(obj.members) ? obj.members : []);
  const events = z
    .array(CommunityEventSchema)
    .safeParse(Array.isArray(obj.events) ? obj.events : []);

  return {
    community: community.data,
    members: members.success ? members.data : [],
    events: events.success ? events.data : [],
    source,
  };
}

const DataEnvelope = dataOf(z.unknown());
export function unwrapData(env: unknown): unknown {
  const parsed = DataEnvelope.safeParse(env);
  return parsed.success ? parsed.data.data : env;
}

const ResultsShape = z.object({ results: z.array(z.unknown()) });
export function unwrapResults(env: unknown): unknown[] | null {
  const parsed = ResultsShape.safeParse(unwrapData(env));
  return parsed.success ? parsed.data.results : null;
}

const RecordShape = z.record(z.string(), z.unknown());

export async function loadCommunity(
  id: string,
  opts: GetOptions = {},
): Promise<CommunityDetail | null> {
  try {
    const [cRaw, mRaw] = await Promise.all([
      getJSON<unknown>(`/v1/communities/${encodeURIComponent(id)}`, opts),
      getJSON<unknown>(
        `/v1/communities/${encodeURIComponent(id)}/members`,
        opts,
      ).catch(() => null),
    ]);

    const community = RecordShape.safeParse(unwrapData(cRaw));
    const members = (mRaw ? unwrapResults(mRaw) : null) ?? [];

    const node = {
      ...(community.success ? community.data : {}),
      members,
      events: [] as unknown[],
    };
    const detail = projectNode(node, "live");
    return detail && { ...detail, copy: readCopy(cRaw) };
  } catch {
    return null;
  }
}

/** decentraland.org's Communities tab read one page of this many communities. */
const DIRECTORY_LIMIT = 200;

/** A row of the communities list: only what a card shows is required. */
const DirectoryCommunitySchema = CommunitySchema.pick({
  id: true,
  name: true,
  description: true,
  ownerName: true,
  thumbnailUrl: true,
  membersCount: true,
}).extend({ privacy: CommunitySchema.shape.privacy.catch("public") });
export type DirectoryCommunity = z.infer<typeof DirectoryCommunitySchema>;

/**
 * The communities list of `/places/communities` (decentraland/sites `DiscoverCommunitiesPage`): one page of up to
 * 200, matching `search` when given, most members first. A row that does not read as a community is left out.
 */
export async function fetchCommunityDirectory(
  search: string,
  opts: Pick<GetOptions, "signal"> = {},
): Promise<DirectoryCommunity[]> {
  const term = search.trim();
  const raw = await getJSON<unknown>("/v1/communities", {
    signal: opts.signal,
    query: { limit: DIRECTORY_LIMIT, offset: 0, ...(term ? { search: term } : {}) },
  });
  const rows = unwrapResults(raw);
  if (!rows) throw new Error("the response did not have the communities shape");
  const communities = rows.flatMap((row) => {
    const parsed = DirectoryCommunitySchema.safeParse(row);
    return parsed.success ? [parsed.data] : [];
  });
  // A stable sort keeps the service's order among equal counts, as the reference's did.
  return communities.sort((a, b) => (b.membersCount ?? 0) - (a.membersCount ?? 0));
}
